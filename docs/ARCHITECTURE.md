# Native architectuur

## Uitvoering

Eén Rust-desktopproces. `eframe` beheert venster/input en `egui` tekent de interface. `glow` tekent IFC-meshes met OpenGL 3.3 in dezelfde viewport. De huidige app bevat geen browser, Python-server of webview.

## Modules

| Module | Verantwoordelijkheid |
|---|---|
| `main.rs` | Desktopstart en headless `--inspect` |
| `app.rs` | Werkruimte, achtergrondtaken, selectie, projecten en UI |
| `model.rs` | IFC-decodering, metadata, hiërarchie en IfcLite-geometrie |
| `renderer.rs` | GPU-buffers, shading, draadmodel en clipping |
| `camera.rs` | Camerabasis, fit, orbit en ray picking |
| `view_cube.rs` | Meedraaiende oriëntatiekubus met polygon-hit-testing |
| `clash.rs` | IfcLite-clashsession, scope en clashregels |
| `validation.rs` | Modelkwaliteit en gedeeltelijke IDS-evaluatie |
| `issues.rs`, `bcf.rs`, `bcf_topics.rs` | Duurzame topics en BCF 2.1/3.0-uitwisseling |
| `pointcloud.rs` | Scanlagen, registratie, ICP, driehoek-BVH en afwijkingsclusters |
| `inspection.rs`, `quality.rs` | Metingen, zichtlijnen en configureerbare kwaliteitschecks |
| `report.rs` | Standalone PDF-overzicht |
| `theme.rs`, `widgets.rs` | OpenAEC-tokens, fonts en pictogrammen |

## Coördinaten

IFC-eenheden worden naar meters geconverteerd. IfcLite RTC-offsets verschuiven georeferentie vóór tessellatie; mesh-origins en absolute grenzen worden als `f64` bewaard. GPU-posities blijven lokaal en worden relatief aan de camera verwerkt. Zo worden kleine afmetingen en diepteverschillen niet eerst afgerond door absolute wereldcoördinaten naar `f32` te converteren. Camera-UI en sommige clipinstellingen gebruiken nog `f32`; dit is geen volledige dubbelprecisie-viewer.

## Identiteit en gegevens

Binnen de actieve werkruimte is een object `(modelindex, STEP-id)`. BCF gebruikt IFC-GlobalIds. STEP-ids zijn niet wereldwijd uniek en mogen niet over modellen worden samengevoegd. Projectbestanden verwijzen naar IFC- en scanbestanden; ze bevatten de modelinhoud niet. Issues, opmerkingen, snapshots, resultaten, clashregels en scanregistraties staan wel in het project. Pointcloud-core wordt ongewijzigd gelinkt vanuit een vastgezette Git-revisie; bron en LGPL-licentie staan in `native/assets/`.

Modelimport en clashcontrole draaien op achtergrondthreads. Resultaten gaan via kanalen naar de UI. Een project sluiten verbreekt de ontvangende verbinding, zodat oude resultaten niet in het nieuwe project terechtkomen.

## Uitbreidingsrichting

Zie [FEATURE-PARITY.md](FEATURE-PARITY.md). Nieuwe coördinatiefuncties krijgen testbare gegevensmodellen buiten de UI. Adapters voor BCF, connectoren en synchronisatie delen dezelfde issue-identiteit. De extensions-UI uit de style-repo dient als vormgevingsreferentie; een runtime voor externe extensions bestaat nog niet.

## Native data en uitwisseling (0.3)

`data.rs` bevat propertyfilters, CSV en GlobalId-gebaseerde revisievergelijking. `app/data_ui.rs` koppelt de onderliggende resultaten aan dezelfde model-scoped selectie als boom/inspecteur. De vergelijking weigert dubbele GUIDs; meshcoördinaten worden in wereldruimte op een 1 mm raster vergeleken. Geometrische equivalentie bij andere triangulatie wordt niet bewezen.

`export.rs` schrijft GLB 2.0 met Y-up coördinaten en behoudt de IFC-world-origin in scene-extras. SVG-export snijdt driehoeken op een absolute Z-hoogte. Beide gebruiken de zichtbaarheid van objecten; viewport-clipping wordt niet op GLB-meshes toegepast.

`scripts/package-native.py` assembleert uitsluitend build-artifacts. De app heeft geen Python-runtime nodig. De releaseworkflow bouwt op elk doelplatform, voert tests/Clippy uit, ondertekent Windows/macOS en genereert build-provenance voor de archieven. Ontbrekende signingsecrets blokkeren tagged releases.
