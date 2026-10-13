use super::*;
fn models() -> Vec<Model> {
    vec![Model::load(
        std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../test/fixtures/sample.ifc"
        )),
        |_| {},
    )
    .unwrap()]
}
fn service(ui: bool) -> Service {
    Service::new(models(), ui)
}
#[test]
fn schemas_and_openapi_cover_every_operation() {
    let definitions = tools();
    assert_eq!(definitions.len(), 10);
    let spec = http::openapi();
    for tool in definitions {
        let _: rmcp::model::Tool = serde_json::from_value(tool.clone()).unwrap();
        let name = tool["name"].as_str().unwrap();
        assert!(spec["paths"][format!("/api/v1/tools/{name}")].is_object());
        assert!(!tool["inputSchema"].to_string().contains("$ref"));
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    }
}
#[test]
fn model_listing_query_pagination_and_properties() {
    let s = service(false);
    assert_eq!(
        s.execute("models_list", json!({})).unwrap()["models"][0]["objects"],
        5
    );
    let q = s
        .execute(
            "objects_query",
            json!({"class":"IfcWallStandardCase","limit":1}),
        )
        .unwrap();
    assert_eq!(q["total"], 1);
    assert_eq!(q["objects"][0]["element"], 42);
    let page = s
        .execute("objects_query", json!({"offset":4,"limit":1}))
        .unwrap();
    assert_eq!(page["total"], 5);
    assert_eq!(page["objects"].as_array().unwrap().len(), 1);
    let o = s
        .execute("object_get", json!({"model":0,"element":42}))
        .unwrap();
    assert_eq!(o["object"]["name"], "W-001");
    assert_eq!(o["object"]["attributes"]["Description"], "Test Wall");
}
#[test]
fn schema_errors_are_explicit() {
    let s = service(false);
    for (name, args, status) in [
        ("models_list", json!({"ignored":true}), 400),
        ("objects_query", json!({"limit":1001}), 400),
        ("objects_query", json!({"limit":0}), 400),
        (
            "objects_query",
            json!({"operator":"greater","value":"NaN"}),
            400,
        ),
        ("objects_query", json!({"operator":"unknown"}), 400),
        ("objects_query", json!({"model":99}), 404),
        ("object_get", json!({"model":0,"element":99999}), 404),
        ("object_get", json!({"model":-1,"element":42}), 400),
        ("object_get", json!({"model":0}), 400),
        ("clash_check", json!({"tolerance":-1}), 400),
        ("models_diff", json!({"before":0,"after":0}), 400),
        ("model_export", json!({"format":"svg"}), 400),
        ("artifact_read", json!({"id":"missing"}), 404),
        ("not_a_tool", json!({}), 404),
    ] {
        assert_eq!(s.execute(name, args).unwrap_err().status, status, "{name}");
    }
}
#[test]
fn validation_and_clash_use_native_geometry() {
    let s = service(false);
    assert!(s.execute("validate", json!({})).unwrap()["findings"].is_array());
    assert!(s.execute("validate", json!({"ids_xml":"<broken"})).is_err());
    let findings = s.execute("clash_check", json!({})).unwrap();
    assert_eq!(findings["findings"], json!([]));
    let empty = s
        .execute("clash_check", json!({"class_b":"IfcDoesNotExist"}))
        .unwrap();
    assert_eq!(empty["findings"], json!([]));
}
#[test]
fn model_comparison_and_export_artifacts_roundtrip() {
    let mut ms = models();
    ms.push(ms[0].clone());
    let s = Service::new(ms, false);
    assert_eq!(
        s.execute("models_diff", json!({"before":0,"after":1}))
            .unwrap()["differences"],
        json!([])
    );
    for args in [
        json!({"format":"glb"}),
        json!({"format":"svg","height":1.2}),
        json!({"format":"csv","column":"Name"}),
    ] {
        let v = s.execute("model_export", args).unwrap();
        let id = v["id"].as_str().unwrap();
        let a = s.artifact(id).unwrap();
        assert_eq!(a.bytes.len() as u64, v["bytes"]);
        let encoded = s.execute("artifact_read", json!({"id":id})).unwrap();
        use base64::Engine;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded["base64"].as_str().unwrap())
            .unwrap();
        assert_eq!(decoded, *a.bytes);
    }
}
#[test]
fn model_sync_invalidates_stale_selection_and_artifacts() {
    let s = service(true);
    let revision = s.snapshot().revision;
    let a = s.execute("model_export", json!({"format":"csv"})).unwrap();
    let q = s
        .execute(
            "view_select",
            json!({"revision":revision,"objects":[{"model":0,"element":42}],"focus":true}),
        )
        .unwrap();
    assert_eq!(q["status"], "queued");
    assert_eq!(s.execute("view_state", json!({})).unwrap()["pending"], 1);
    s.sync(&models());
    assert_eq!(s.execute("view_state", json!({})).unwrap()["pending"], 0);
    assert!(s.artifact(a["id"].as_str().unwrap()).is_err());
    assert_eq!(
        s.execute("view_select", json!({"revision":revision,"objects":[]}))
            .unwrap_err()
            .status,
        409
    );
}
#[test]
fn headless_mode_does_not_claim_to_control_a_viewer() {
    let s = service(false);
    let revision = s.snapshot().revision;
    assert_eq!(s.execute("view_state", json!({})).unwrap()["ui"], false);
    assert_eq!(
        s.execute("view_select", json!({"revision":revision,"objects":[]}))
            .unwrap_err()
            .status,
        409
    );
}
#[test]
fn desktop_queue_has_a_limit_and_preserves_request_fields() {
    let s = service(true);
    let revision = s.snapshot().revision;
    for _ in 0..32 {
        s.execute(
            "view_select",
            json!({"revision":revision,"objects":[{"model":0,"element":42}],"focus":true}),
        )
        .unwrap();
    }
    assert_eq!(
        s.execute("view_select", json!({"revision":revision,"objects":[]}))
            .unwrap_err()
            .status,
        429
    );
    let q = s.state.lock().unwrap().pending.pop_front().unwrap();
    assert!(!q.id.is_empty());
    assert_eq!(q.keys, vec![(0, 42)]);
    assert_eq!(q.revision, revision);
    assert!(q.focus);
}
#[tokio::test]
async fn worker_limit_and_audit_are_bounded() {
    let s = service(false);
    let permits = s.workers.clone().acquire_many_owned(2).await.unwrap();
    assert_eq!(
        s.call("models_list".into(), json!({}))
            .await
            .unwrap_err()
            .status,
        429
    );
    drop(permits);
    for _ in 0..103 {
        s.call("models_list".into(), json!({})).await.unwrap();
    }
    assert_eq!(s.state.lock().unwrap().audit.len(), 100);
}
#[tokio::test]
async fn mcp_official_client_negotiates_lists_and_calls() {
    use rmcp::{model::CallToolRequestParams, ServiceExt};
    let (client_io, server_io) = tokio::io::duplex(128 * 1024);
    let server = tokio::spawn(async move {
        mcp::Mcp(service(false))
            .serve(server_io)
            .await
            .unwrap()
            .waiting()
            .await
            .unwrap()
    });
    let client = ().serve(client_io).await.unwrap();
    let tools = client.list_all_tools().await.unwrap();
    assert_eq!(tools.len(), 10);
    let reply = client
        .call_tool(
            CallToolRequestParams::new("objects_query")
                .with_arguments(json!({"limit":1}).as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    let v = serde_json::to_value(reply).unwrap();
    assert_eq!(v["isError"], false);
    assert_eq!(
        v["structuredContent"]["objects"].as_array().unwrap().len(),
        1
    );
    let error = client
        .call_tool(
            CallToolRequestParams::new("object_get").with_arguments(
                json!({"model":0,"element":9999})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    assert_eq!(serde_json::to_value(error).unwrap()["isError"], true);
    client.cancel().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap();
}
const TOKEN: &str = "test-token-only-not-a-production-secret";
#[tokio::test]
async fn http_auth_limits_errors_exports_and_shutdown() {
    let mut server = http::Server::start(0, TOKEN.into(), service(false)).unwrap();
    let url = format!("http://{}", server.address);
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap();
    assert_eq!(
        client
            .get(format!("{url}/api/v1/models"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        client
            .get(format!("{url}/api/v1/models"))
            .bearer_auth(TOKEN)
            .header("origin", "https://external.example")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        client
            .get(format!("{url}/api/v1/models"))
            .bearer_auth(TOKEN)
            .header("host", "external.example")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        client
            .get(format!("{url}/api/v1/models"))
            .bearer_auth(TOKEN)
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()["models"][0]["objects"],
        5
    );
    assert_eq!(
        client
            .post(format!("{url}/api/v1/tools/object_get"))
            .bearer_auth(TOKEN)
            .json(&json!({"model":0}))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        client
            .post(format!("{url}/api/v1/tools/not_real"))
            .bearer_auth(TOKEN)
            .json(&json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        client
            .post(format!("{url}/api/v1/tools/models_list"))
            .bearer_auth(TOKEN)
            .header("content-type", "application/json")
            .body("{invalid")
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    let rejected = client
        .post(format!("{url}/api/v1/tools/models_list"))
        .bearer_auth(TOKEN)
        .header("content-type", "application/json")
        .body("x".repeat(1024 * 1024 + 1))
        .send()
        .await
        .unwrap();
    assert_eq!(rejected.status(), 413);
    assert_eq!(rejected.headers()["connection"], "close");
    drop(rejected);
    let export = client
        .post(format!("{url}/api/v1/tools/model_export"))
        .bearer_auth(TOKEN)
        .json(&json!({"format":"glb"}))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let bytes = client
        .get(format!("{}{}", url, export["download"].as_str().unwrap()))
        .bearer_auth(TOKEN)
        .send()
        .await
        .unwrap();
    assert_eq!(bytes.headers()["content-type"], "model/gltf-binary");
    assert_eq!(&bytes.bytes().await.unwrap()[..4], b"glTF");
    assert!(http::Server::start(server.address.port(), TOKEN.into(), service(false)).is_err());
    let address = server.address;
    server.stop.cancel();
    tokio::task::spawn_blocking(move || server.join().unwrap())
        .await
        .unwrap();
    assert!(std::net::TcpStream::connect(address).is_err());
}
fn wire_json(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|_| {
        text.lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .filter_map(|data| serde_json::from_str::<Value>(data).ok())
            .find(|value| value.get("id").is_some())
            .unwrap_or_else(|| panic!("No JSON-RPC response: {text}"))
    })
}
#[tokio::test]
async fn mcp_http_legacy_handshake_session_and_tool_call() {
    let mut server = http::Server::start(0, TOKEN.into(), service(false)).unwrap();
    let url = format!("http://{}/mcp", server.address);
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap();
    let request = |value: &Value| {
        client
            .post(&url)
            .bearer_auth(TOKEN)
            .header("accept", "application/json, text/event-stream")
            .json(value)
    };
    let init=request(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"openaec-integration-test","version":"1"}}})).send().await.unwrap();
    assert!(init.status().is_success());
    let session = init
        .headers()
        .get("mcp-session-id")
        .map(|h| h.to_str().unwrap().to_owned());
    let body = wire_json(&init.text().await.unwrap());
    assert_eq!(body["result"]["protocolVersion"], "2025-06-18");
    let session_request = |value: &Value| {
        let r = request(value).header("mcp-protocol-version", "2025-06-18");
        if let Some(s) = &session {
            r.header("mcp-session-id", s)
        } else {
            r
        }
    };
    assert!(
        session_request(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .send()
            .await
            .unwrap()
            .status()
            .is_success()
    );
    let list = wire_json(
        &session_request(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
    );
    assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 10);
    let call=wire_json(&session_request(&json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"models_list","arguments":{}}})).send().await.unwrap().text().await.unwrap());
    assert_eq!(
        call["result"]["structuredContent"]["models"][0]["objects"],
        5
    );
    server.stop.cancel();
    tokio::task::spawn_blocking(move || server.join().unwrap())
        .await
        .unwrap();
}

#[test]
fn restarting_server_does_not_reuse_selection_revision() {
    let first = service(true);
    let old = first.snapshot().revision;
    let restarted = service(true);
    let current = restarted.snapshot().revision;
    assert_ne!(old, current);
    assert!(current < (1_u64 << 53));
    assert_eq!(
        restarted
            .execute(
                "view_select",
                json!({"revision":old,"objects":[{"model":0,"element":42}]})
            )
            .unwrap_err()
            .status,
        409
    );
    assert_eq!(
        restarted
            .execute(
                "view_select",
                json!({"revision":current,"objects":[{"model":0,"element":42}]})
            )
            .unwrap()["status"],
        "queued"
    );
}
