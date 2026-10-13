#!/usr/bin/env python3
"""Build-time packaging only; the shipped application has no Python dependency."""
import argparse
import hashlib
import json
import pathlib
import plistlib
import shutil
import subprocess
import tarfile
import tomllib
import zipfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
p = argparse.ArgumentParser()
p.add_argument('--target', required=True)
p.add_argument('--stage-only', action='store_true')
p.add_argument('--archive-only', action='store_true')
a = p.parse_args()
version = tomllib.loads((ROOT / 'native/Cargo.toml').read_text())['package']['version']
name = f'openaec-bim-validator-{version}-{a.target}'
output = ROOT / 'dist'
stage = output / name
windows = 'windows' in a.target
mac = 'apple' in a.target
exe = 'openaec-bim-validator' + ('.exe' if windows else '')
if not a.archive_only:
    if stage.exists():
        raise SystemExit(f'Staging directory already exists: {stage}')
    stage.mkdir(parents=True)
    binary = ROOT / 'target' / a.target / 'release' / exe
    if not binary.exists():
        host = subprocess.check_output(['rustc', '-vV'], text=True).split('host: ')[1].splitlines()[0]
        if host == a.target:
            binary = ROOT / 'target/release' / exe
    destination = stage / exe
    if mac:
        contents = stage / 'OpenAEC BIM Validator.app/Contents'
        (contents / 'MacOS').mkdir(parents=True)
        (contents / 'Resources').mkdir()
        destination = contents / 'MacOS' / exe
        with (contents / 'Info.plist').open('wb') as f:
            plistlib.dump({'CFBundleName': 'OpenAEC BIM Validator', 'CFBundleDisplayName': 'OpenAEC BIM Validator',
                          'CFBundleIdentifier': 'org.openaec.bim-validator', 'CFBundleExecutable': exe,
                          'CFBundleVersion': version.split('-')[0], 'CFBundleShortVersionString': version.split('-')[0],
                          'CFBundlePackageType': 'APPL', 'NSHighResolutionCapable': True,
                          'LSMinimumSystemVersion': '12.0', 'CFBundleIconFile': 'AppIcon'}, f)
        iconset = output / 'AppIcon.iconset'
        iconset.mkdir(exist_ok=True)
        for size in [16, 32, 128, 256, 512]:
            for scale in [1, 2]:
                suffix = '@2x' if scale == 2 else ''
                subprocess.run(['sips', '-z', str(size * scale), str(size * scale), str(ROOT / 'native/assets/openaec-symbol.png'), '--out', str(iconset / f'icon_{size}x{size}{suffix}.png')], check=True, stdout=subprocess.DEVNULL)
        subprocess.run(['iconutil', '-c', 'icns', str(iconset), '-o', str(contents / 'Resources/AppIcon.icns')], check=True)
    shutil.copy2(binary, destination)
    for file in ['README.md', 'docs/FEATURE-PARITY.md', 'docs/RELEASE.md', 'release/NOTES.md']:
        shutil.copy2(ROOT / file, stage / pathlib.Path(file).name)
    shutil.copytree(ROOT / 'native/assets', stage / 'licenses-and-assets')
    shutil.copytree(ROOT / 'docs', stage / 'docs')
    shutil.copytree(ROOT / 'release', stage / 'release')
    metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked', '--format-version', '1'], cwd=ROOT))
    inventory = [{'name': pkg['name'], 'version': pkg['version'], 'license': pkg['license'], 'source': pkg['source'], 'repository': pkg.get('repository')} for pkg in metadata['packages']]
    (stage / 'DEPENDENCIES.json').write_text(json.dumps(inventory, indent=2) + '\n')
    # Include the exact application source/lockfile and build instructions for relinking.
    with tarfile.open(stage / 'source.tar.gz', 'w:gz') as archive:
        for file in ['Cargo.toml', 'Cargo.lock', 'native', 'ids-bestanden', 'scripts', 'docs/RELEASE.md', 'test/fixtures', 'README.md']:
            archive.add(ROOT / file, arcname=file)
    (stage / 'BUILD.txt').write_text(f'Version: {version}\nTarget: {a.target}\nCommit: ' + subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip() + '\n')
if a.stage_only:
    print(stage)
    raise SystemExit()
if not stage.is_dir():
    raise SystemExit('No staged package; refusing to create empty archive')
archive = output / (name + ('.zip' if windows or mac else '.tar.gz'))
if mac:
    subprocess.run(['ditto', '-c', '-k', '--sequesterRsrc', '--keepParent', str(stage), str(archive)], check=True)
elif windows:
    with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED) as z:
        for file in sorted(stage.rglob('*')):
            if file.is_file():
                z.write(file, file.relative_to(output))
else:
    with tarfile.open(archive, 'w:gz') as t:
        t.add(stage, arcname=name)
hash_value = hashlib.file_digest(archive.open('rb'), 'sha256').hexdigest()
(archive.with_suffix(archive.suffix + '.sha256')).write_text(f'{hash_value}  {archive.name}\n')
print(archive)
