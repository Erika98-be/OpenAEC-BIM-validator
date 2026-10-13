#!/usr/bin/env python3
"""Black-box tests of the shipped executable. Python is only a test dependency."""
import base64
import json
import os
import pathlib
import queue
import re
import signal
import subprocess
import sys
import threading
import unittest
import urllib.error
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent
BINARY = str(pathlib.Path(sys.argv[1]).resolve()) if len(sys.argv) > 1 else str(ROOT / 'target/debug/openaec-bim-validator')
FIXTURE = str(ROOT / 'test/fixtures/sample.ifc')
TOKEN = 'local-protocol-test-token-not-for-production'


def lines(stream, output):
    try:
        for line in stream:
            output.put(line)
    finally:
        output.put(None)


class McpProcess:
    def __init__(self):
        self.process = subprocess.Popen([BINARY, '--mcp', FIXTURE], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding='utf-8', bufsize=1)
        self.output, self.errors = queue.Queue(), queue.Queue()
        threading.Thread(target=lines, args=(self.process.stdout, self.output), daemon=True).start()
        threading.Thread(target=lines, args=(self.process.stderr, self.errors), daemon=True).start()
        self.next_id = 1
        reply = self.request('initialize', {'protocolVersion': '2025-06-18', 'capabilities': {}, 'clientInfo': {'name': 'native-blackbox', 'version': '1'}})
        if reply['result']['protocolVersion'] != '2025-06-18':
            raise AssertionError(reply)
        self.notify('notifications/initialized')

    def notify(self, method):
        self.process.stdin.write(json.dumps({'jsonrpc': '2.0', 'method': method}) + '\n')
        self.process.stdin.flush()

    def request(self, method, params=None):
        request = {'jsonrpc': '2.0', 'id': self.next_id, 'method': method}
        self.next_id += 1
        if params is not None:
            request['params'] = params
        self.process.stdin.write(json.dumps(request) + '\n')
        self.process.stdin.flush()
        while True:
            line = self.output.get(timeout=20)
            if line is None:
                raise AssertionError('MCP closed stdout before returning a result')
            message = json.loads(line)  # Any non-protocol stdout fails the test.
            if message.get('id') == request['id']:
                return message

    def tool(self, name, args):
        return self.request('tools/call', {'name': name, 'arguments': args})

    def close(self):
        self.process.stdin.close()
        try:
            code = self.process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait()
            raise AssertionError('MCP did not exit on stdin EOF')
        self.process.stdout.close()
        self.process.stderr.close()
        if code != 0:
            raise AssertionError(f'MCP exit code {code}')


class StdioTests(unittest.TestCase):
    def setUp(self):
        self.mcp = McpProcess()
        self.addCleanup(self.mcp.close)

    def test_discovery_and_unknown_method(self):
        tools = self.mcp.request('tools/list')['result']['tools']
        self.assertEqual(len(tools), 10)
        self.assertEqual(self.mcp.request('unimplemented/method')['error']['code'], -32601)

    def test_model_query_and_object_properties(self):
        data = self.mcp.tool('models_list', {})['result']['structuredContent']
        self.assertEqual(data['models'][0]['objects'], 5)
        query = self.mcp.tool('objects_query', {'class': 'IfcWallStandardCase'})['result']['structuredContent']
        self.assertEqual(query['total'], 1)
        obj = self.mcp.tool('object_get', {'model': 0, 'element': 42})['result']['structuredContent']['object']
        self.assertEqual(obj['name'], 'W-001')

    def test_exports_and_validation(self):
        export = self.mcp.tool('model_export', {'format': 'glb'})['result']['structuredContent']
        payload = self.mcp.tool('artifact_read', {'id': export['id']})['result']['structuredContent']
        self.assertEqual(base64.b64decode(payload['base64'])[:4], b'glTF')
        self.assertIsInstance(self.mcp.tool('validate', {})['result']['structuredContent']['findings'], list)
        self.assertEqual(self.mcp.tool('clash_check', {})['result']['structuredContent']['findings'], [])

    def test_errors_and_no_fake_ui(self):
        error = self.mcp.tool('object_get', {'model': 0, 'element': 9999})['result']
        self.assertTrue(error['isError'])
        self.assertEqual(error['structuredContent']['status'], 404)
        self.assertFalse(self.mcp.tool('view_state', {})['result']['structuredContent']['ui'])
        self.assertTrue(self.mcp.tool('view_select', {'revision': 1, 'objects': []})['result']['isError'])


class HttpProcessTests(unittest.TestCase):
    def test_cli_http_start_auth_query_and_shutdown(self):
        process = subprocess.Popen([BINARY, '--api', '0', FIXTURE], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding='utf-8', env={**os.environ, 'OPENAEC_API_TOKEN': TOKEN})
        output = queue.Queue()
        threading.Thread(target=lines, args=(process.stderr, output), daemon=True).start()
        try:
            while True:
                line = output.get(timeout=20)
                self.assertIsNotNone(line, 'HTTP CLI exited before startup')
                match = re.search(r'http://127\.0\.0\.1:\d+', line)
                if match:
                    url = match.group(0)
                    break
            opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
            with self.assertRaises(urllib.error.HTTPError) as failure:
                opener.open(url + '/api/v1/models', timeout=10)
            self.assertEqual(failure.exception.code, 401)
            request = urllib.request.Request(url + '/api/v1/tools/objects_query', data=b'{"class":"IfcWallStandardCase"}', headers={'Authorization': 'Bearer ' + TOKEN, 'Content-Type': 'application/json'})
            with opener.open(request, timeout=10) as response:
                self.assertEqual(json.load(response)['total'], 1)
            if os.name == 'posix':
                process.send_signal(signal.SIGINT)
                self.assertEqual(process.wait(timeout=10), 0)
        finally:
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=10)
            process.stdout.close()
            process.stderr.close()


if __name__ == '__main__':
    unittest.main(argv=[sys.argv[0]], verbosity=2)
