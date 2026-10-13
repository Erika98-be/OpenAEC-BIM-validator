# Verificatie — 0.3.0-rc.1

Uitgevoerd op Linux Mint 22.3, 13 oktober 2026:

- `cargo test --all-features --locked`: 53 tests geslaagd.
- `cargo clippy --all-features --all-targets --locked -- -D warnings`: geslaagd.
- `cargo fmt --all --check` en `git diff --check`: geslaagd.
- Native releasebinary gebouwd; echte IFC geïmporteerd via headless inspectie: 255 objecten, 49.140 driehoeken.
- CLI-query op `IfcWallStandardCase`: verwacht object uit fixture gevonden.
- Dezelfde IFC tegen zichzelf vergeleken: nul wijzigingen.
- Werkelijke GLB/SVG-export: GLB-header, chunklengtes en bufferranges gecontroleerd; SVG opnieuw als XML ingelezen.
- Vier UI-screenshots gemaakt met de native app: viewer, data, issues en scans.
- `actionlint` 1.7.12: native check- en releaseworkflows geslaagd.
- Python packagingscript syntactisch gecontroleerd; macOS-signingscript met `bash -n` gecontroleerd.

API/MCP-aanvulling:

- Officiële Rust-MCP-client: onderhandeling, discovery, query en toolfouten geslaagd.
- Streamable HTTP: legacy initialisatie (2025-06-18), sessie, tools/list en tools/call geslaagd.
- REST: bearer-authenticatie, Host/Origin-afwijzing, bodylimiet, argumentfouten en werkelijke GLB-download gecontroleerd.
- Regressie opgelost voor het afwijzen van uploads en connection reuse. De volledige HTTP-lifecycle-test slaagt daarna twintig keer achter elkaar.
- Alle tien tooloperaties getest op de gedeelde uitvoeringslaag; paginering, modelrevisies, verlopen exports, headless/UI-verschil, wachtrijlimiet, workerlimiet en auditlimiet gecontroleerd.
- `python3 scripts/test-native-protocols.py target/debug/openaec-bim-validator`: vijf black-box subprocess-tests geslaagd (ook op de releasebinary); stdio-EOF en HTTP-Ctrl+C sluiten de executable correct af.
- Native UI gestart met echt IFC-model: HTTP-query en externe selectie uitgevoerd; de screenshotcontrole bevestigt `applied` en één geselecteerd object. Screenshots: docs/native-mcp.png (1440×900) en docs/native-mcp-compact.png (1024×768), met passende paneelbreedte en tablabels.
- Serverherstart krijgt een nieuwe revisie-ID; oude selectieopdrachten worden geweigerd.
- De tests zijn opgenomen in de native check- en platformreleaseworkflows.

Nog niet uitgevoerd:

- Windows- en macOS-compilatie/runtimecontrole (GitHub-run vereist).
- Authenticode, Developer ID signing, notarization en GitHub-attestation (credentials/secrets vereist).
- End-to-end Revit, gedeelde projectserver en AI-provider: die functies zijn nog niet geïmplementeerd.
- Volledige functiedekking van ClashControl en IfcLite: zie FEATURE-PARITY.md.

Een workflow-lint is geen platformbuild. Een checksum is geen vertrouwde codehandtekening. Het lokaal gemaakte Linux-pakket is ongetekend; publiceer het niet als ondertekend CI-artifact.
