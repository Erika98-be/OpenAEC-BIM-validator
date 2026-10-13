# Lokale API en MCP-server

De native Rust-binary biedt een REST/JSON API, OpenAPI 3.1, MCP over Streamable HTTP en MCP over stdio. Alle transports gebruiken dezelfde functies in `native/src/service/`. MCP gebruikt de officiële `rmcp` Rust-SDK (3.5.1). Er is geen Python-/Node-runtime nodig.

## Vanuit de desktop

1. Open je IFC-modellen via Bestand.
2. Kies **API / MCP → Verbinding** en start de server. Standaardpoort: `19781`; `0` kiest een vrije poort.
3. Kopieer het token en het API-adres, of kopieer de MCP-configuratie voor je client. Het precieze configuratieformaat verschilt per MCP-client; nodig zijn de URL en de Authorization-header.
4. **Tools testen** toont alle tools, hun argumentschema en een JSON-editor. Uitvoeren verstuurt een echte HTTP-aanroep naar de lokale API. Het antwoord en de HTTP-status verschijnen in het paneel.
5. **Activiteit** toont de laatste 100 uitgevoerde toolaanroepen, status en duur. Tokens en argumentinhoud worden niet gelogd.
6. Stoppen sluit de luisterende server en MCP-sessies. Herstarten gebruikt de actuele geladen modellen.

De 3D-viewer blijft zichtbaar. Een `view_select`-aanroep plaatst een opdracht in de UI-wachtrij. `view_state` bevestigt daarna `applied` en toont de selectie. Met `focus:true` zoomt de camera naar de selectie. Het laden/sluiten van modellen verhoogt de revisie en wist oude selectieopdrachten en exportartifacts. Een verouderde revisie geeft HTTP 409 / MCP tool error.

Het token ontstaat lokaal bij het starten van de app. Serverstatus en token worden niet in projecten of voorkeuren opgeslagen. Er is geen automatisch gestarte netwerkdienst.

![Native API/MCP-paneel](native-mcp.png)

## Headless starten

```sh
# Kies zelf een lang willekeurig token; niet in Git opslaan.
export OPENAEC_API_TOKEN="$(openssl rand -hex 32)"
openaec-bim-validator --api 19781 model.ifc constructie.ifc
```

Gebruik Ctrl+C om te stoppen. Het adres wordt naar stderr geschreven, het token niet. De server luistert uitsluitend op `127.0.0.1`. Alle HTTP-routes vereisen `Authorization: Bearer <token>`. Host-validatie is actief; browser-Origin-requests worden geweigerd. Deze versie is bedoeld voor native lokale clients, zonder CORS, publieke hosting of multi-userrollen.

```sh
curl -H "Authorization: Bearer $OPENAEC_API_TOKEN" \
  http://127.0.0.1:19781/api/v1/models

curl -H "Authorization: Bearer $OPENAEC_API_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"class":"IfcWall","limit":50}' \
  http://127.0.0.1:19781/api/v1/tools/objects_query
```

Voor headless MCP start een client de binary als child process:

```json
{
  "mcpServers": {
    "openaec": {
      "command": "/absolute/path/openaec-bim-validator",
      "args": ["--mcp", "/absolute/path/model.ifc"]
    }
  }
}
```

Stdio gebruikt stdin/stdout voor MCP en stderr voor diagnostiek. Sluiten van stdin beëindigt de server. HTTP-authenticatie is niet van toepassing op stdio. In headless mode rapporteert `view_state` `ui:false`; `view_select` geeft een fout, want er is geen viewer gekoppeld.

Voor de draaiende desktop- of headless HTTP-server:

```json
{
  "mcpServers": {
    "openaec": {
      "url": "http://127.0.0.1:19781/mcp",
      "headers": {"Authorization": "Bearer <token-uit-de-app>"}
    }
  }
}
```

De HTTP MCP-transport is Streamable HTTP; een eigen JSON-RPC-client moet MCP-initialisatie, protocolheaders en eventuele sessie-ID afhandelen. Gebruik bij voorkeur een MCP-SDK. De legacy handshake met protocol `2025-06-18` en de standaardonderhandeling van de gekoppelde Rust-SDK worden getest.

## Routes

| Methode | Route | Resultaat |
|---|---|---|
| GET | `/api/v1/health` | Serverstatus en appversie |
| GET | `/api/v1/models` | Modeloverzicht en revisie |
| GET | `/api/v1/tools` | Toolnamen, beschrijvingen, JSON Schema |
| POST | `/api/v1/tools/{name}` | Toolaanroep; body is het argumentobject |
| GET | `/api/v1/artifacts/{id}` | Exportbytes met Content-Type |
| GET | `/api/v1/openapi.json` | OpenAPI 3.1-schema met concrete operation-routes |
| MCP | `/mcp` | Officiële SDK-transport, tools/list en tools/call |

## Tools

| Tool | Argumenten | Gedrag |
|---|---|---|
| `models_list` | `{}` | Model-ID, naam, schema, object-/mesh-/driehoekaantallen, waarschuwingen en revisie |
| `objects_query` | optioneel `model`, `class`, `text`, `property`, `operator`, `value`, `offset`, `limit` | Gepagineerde objecten met metadata; exact IFC-type; property als `Pset_WallCommon.FireRating` |
| `object_get` | `model`, `element` | Attributen, propertysets, propertytypes, materialen, classificaties en parent |
| `validate` | optioneel `ids_xml` | Basiskwaliteit of gedeeltelijke IDS-evaluator |
| `clash_check` | optioneel `model_a/b`, `class_a/b`, `clearance`, `tolerance`, `distance` | Native IfcLite-clashes op geladen meshes; alle objecten, onafhankelijk van viewer-zichtbaarheid |
| `models_diff` | `before`, `after` | GlobalId-vergelijking van twee modellen; metadata en meshes op 1 mm raster |
| `model_export` | `format`: `glb`, `svg` of `csv`; `height` voor SVG; optioneel `column` voor CSV | Tijdelijk exportartifact; alle geladen objecten, geen viewport-clipping |
| `artifact_read` | `id` | Base64-export tot 4 MiB; grotere exports via REST-download |
| `view_state` | `{}` | UI-koppeling, revisie, selectie, wachtrijlengte en laatst toegepaste selectieopdracht |
| `view_select` | `revision`, `objects:[{"model":0,"element":42}]`, optioneel `focus` | Selectieopdracht voor gekoppelde desktop, antwoord `queued` met request-ID |

Een nieuwe serverinstantie krijgt een eigen revisie-ID, zodat oude selectieopdrachten ook na een herstart worden geweigerd. Behandel revisies als ondoorzichtige getallen en lees ze via models_list. Model-ID's zijn indices binnen de huidige revisie; element-ID's zijn STEP-nummers binnen dat model. Gebruik GlobalId voor duurzame koppelingen tussen exports. Filteroperators: `contains`, `equals`, `missing`, `present`, `greater`, `less`. Numerieke vergelijking vereist een eindig getal. Gebruik `models_list` om de actuele revisie te lezen voordat je selecteert.

## Grenzen en fouten

- Twee berekeningen tegelijk; meer geeft 429. Zware berekeningen staan op workerthreads, zodat HTTP en de UI responsief blijven.
- HTTP-body maximaal 1 MiB, met maximaal 15 seconden voor het ontvangen van de body; query maximaal 1000 objecten per pagina. Een afgewezen verbinding wordt gesloten zodat halfgelezen uploads niet worden hergebruikt.
- UI-wachtrij maximaal 32 opdrachten, maximaal 1000 objecten per selectie.
- Exportartifacts maximaal 64 MiB totaal en 8 artifacts. Bij overschrijding van de cachecapaciteit worden vorige artifacts verwijderd; opnieuw exporteren geeft een nieuw ID. Modelherladen en serverstop wissen de cache.
- Er is geen endpoint voor willekeurige bestandspaden, shellcommando's, IFC-bewerking of externe uploads. Modellen worden via de UI of expliciete CLI-argumenten geladen.
- Standaard-validatie met nul modellen levert een lege lijst, geen certificering of conformiteitsverklaring. IDS-dekking blijft gedeeltelijk; de API verandert die beperking niet.
- UI-besturing omvat selectie en camera-focus; issues, scans, camera-instellingen en projectopslag hebben in deze versie nog geen API.
- Onderliggende langlopende geometrieberekeningen zijn niet onderbreekbaar; een verbroken clientverbinding kan de berekening laten afronden. Resultaten zijn niet duurzaam opgeslagen.
- REST-operatiefouten: `{"error":"..."}` met 400/404/409/429/500. Authenticatiefouten: 401; Host/Origin: 403; te grote body: 413. Parserfouten kunnen de standaard Axum-tekstresponse gebruiken.
- MCP maakt onderscheid tussen protocolfouten en toolfouten (`isError:true` plus gestructureerde foutinhoud).

## Verifiëren

```sh
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-features --all-targets --locked -- -D warnings
cargo build --all-features --locked
python3 scripts/test-native-protocols.py target/debug/openaec-bim-validator
OPENAEC_SCREENSHOT_TO=/tmp/openaec-mcp.png OPENAEC_SCREENSHOT_REVIEW=mcp \
  target/debug/openaec-bim-validator test/2786_CLT_model.ifc
```

Protocolreferenties: [MCP lifecycle](https://modelcontextprotocol.io/specification/2025-06-18/basic/lifecycle), [MCP tools](https://modelcontextprotocol.io/specification/2025-06-18/server/tools), [officiële Rust-SDK](https://github.com/modelcontextprotocol/rust-sdk).
