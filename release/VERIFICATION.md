# Verificatie — 0.3.0-rc.1

Uitgevoerd op Linux Mint 22.3, 13 oktober 2026:

- `cargo test --all-features --locked`: 40 tests geslaagd.
- `cargo clippy --all-features --all-targets --locked -- -D warnings`: geslaagd.
- `cargo fmt --all --check` en `git diff --check`: geslaagd.
- Native releasebinary gebouwd; echte IFC geïmporteerd via headless inspectie: 255 objecten, 49.140 driehoeken.
- CLI-query op `IfcWallStandardCase`: verwacht object uit fixture gevonden.
- Dezelfde IFC tegen zichzelf vergeleken: nul wijzigingen.
- Werkelijke GLB/SVG-export: GLB-header, chunklengtes en bufferranges gecontroleerd; SVG opnieuw als XML ingelezen.
- Vier UI-screenshots gemaakt met de native app: viewer, data, issues en scans.
- `actionlint` 1.7.12: native check- en releaseworkflows geslaagd.
- Python packagingscript syntactisch gecontroleerd; macOS-signingscript met `bash -n` gecontroleerd.

Nog niet uitgevoerd:

- Windows- en macOS-compilatie/runtimecontrole (GitHub-run vereist).
- Authenticode, Developer ID signing, notarization en GitHub-attestation (credentials/secrets vereist).
- End-to-end Revit, servercollaboratie en AI: die functies zijn nog niet geïmplementeerd.
- Volledige functiedekking van ClashControl en IfcLite: zie FEATURE-PARITY.md.

Een workflow-lint is geen platformbuild. Een checksum is geen vertrouwde codehandtekening. Het lokaal gemaakte Linux-pakket is ongetekend; publiceer het niet als ondertekend CI-artifact.
