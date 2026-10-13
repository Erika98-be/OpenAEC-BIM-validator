# Functionele uitbreiding: ClashControl + IfcLite → native Rust

Werkdocument, 13 oktober 2026. Doel: de coördinatieworkflow van ClashControl beschikbaar maken in OpenAEC BIM Validator, met Rust, IfcLite en de OpenAEC-vormgeving. Dit document is ook de controlelijst voor oplevering; een geplande functie is geen beschikbare functie.

Referentie: [ClashControl](https://www.clashcontrol.io/tour/) en de [publieke repository](https://github.com/clashcontrol-io/ClashControl), bekeken op commit `a85ab785f9caf2f6c8522cb7f8052d94d8a00b5a`. Functionaliteit wordt hier zelfstandig in Rust geïmplementeerd. De webapp wordt niet ingebed.

## Voortgang en acceptatie

| Onderdeel | Huidige native versie | Nodig voor voltooiing |
|---|---|---|
| IFC-federatie | Meerdere modellen; STEP IFC2X3/4/4X3; lokale verwerking | Grote federaties benchmarken, plaatsingscontrole en importdekking uitbreiden |
| Viewer | Orbit, pan, zoom, perspectief/orthografisch, kleuren per type, draadmodel, doorsnedebox | Willekeurige snijvlakken en oorspronkelijke materialen; walk/oppervlak zijn toegevoegd |
| Modelboom | Ruimtelijk, IFC-type, materiaal, classificatie en discipline afgeleid van type | Geautoriseerde discipline-indeling en kleur op classificatie |
| View cube | Camera-afhankelijk; vlakken/randen/hoeken; kompas | Visuele controles in alle hoofdstanden |
| Clashdetectie | Harde clashes; regels met model-/typefilters, tolerantie en clearance | Herhaalruns met statusbehoud aanwezig; clustering en discipline-matrix uitbreiden |
| Zichtlijnen | Oogpunt/doel en blokkades via twee meetpunten | Zichtstraal per blokkade en configureerbare voorschriften |
| Issues | Duurzame topics, status, prioriteit, categorie, eigenaar, opmerkingen en herhaalrunbehoud | Uitgebreide history, conflicten en gedeelde topics |
| BCF | 2.1/3.0-import/export, snapshots, clipvlakken, comments; roundtrip- en XSD-controle | Meerdere viewpoints, camera-roll, schuine clipvlakken, documenten en kleuroverrides |
| IDS | Gedeeltelijke evaluator en XML-editor | `partOf`, datatype-/eenheidssemantiek en referentietestset |
| Datakwaliteit | Basiscontroles, configureerbare metadata-eisen, NL-SfB-aanwezigheid en auditbare score | Volledige ILS-regelsets en NL-SfB-codevalidatie |
| Projecten | Lokale `.oaec` met modellen, standpunten, issues, resultaten, regels en scanregistratie | Modelherladen en projectuitwisseling verder uitbreiden |
| Pointclouds | Import via pointcloud-core, puntweergave, 3-puntsregistratie en ICP | Volledige scanverwerking; huidige limiet: 2 miljoen steekproefpunten |
| As-built | Afstand tot IFC-driehoeken, heatmap, clusterissues en PDF-overzicht | Snapshots per cluster en uitgebreide rapportlay-out; alleen steekproef gecontroleerd |
| Geocontext | RTC-origins voor geometrieprecisie | IfcMapConversion/ProjectedCRS, EPSG en kaartachtergrond |
| Revit | Nog niet aanwezig | Connectorprotocol, model-/property-sync en issue-terugkoppeling; verificatie met Revit |
| Samenwerking | Lokale projectbestanden | Projectcode, gedeelde topics, conflictbehandeling en synchronisatie |
| AI / integraties | Native MCP stdio/HTTP en REST API met tien tools; UI-serverbeheer en viewerselectie | AI-provider, scripting, triage en overige UI-acties via API |
| Extensions | Style-referentie onderzocht | Native register, installatiecontract, versie-/compatibiliteitscontrole; geen fictieve catalogus |
| Offline desktop | Rust-binary, geen webview | Releasepakketten per platform en installatieverificatie |

## IfcLite-dekking

Referentie: [IfcLite README](https://github.com/LTplus-AG/ifc-lite) en de [documentatie](https://ifclite.dev/docs/), geraadpleegd 13 oktober 2026. De upstream TypeScript-toolkit is breder dan de drie gekoppelde Rust-crates. Native aanwezigheid van de kernel betekent niet dat de gehele viewer/SDK is overgenomen.

| IfcLite-functie | Native status | Nog nodig |
|---|---|---|
| IFC parsing/geometrie | IfcLite 20.4.0 native Rust, STEP IFC2X3/4/4X3 | IFCZIP en IFC5/IFCX, importdekking en streaming verbeteren |
| Federatie | Meerdere modellen in één scène | Samenvoegen naar één nieuw IFC-bestand, schema-conversie |
| Query / schedules | Tekst/type/propertyfilters, numerieke vergelijking, bronwaardesom, CSV | SQL/DuckDB-equivalent, samengestelde voorwaarden, meerdere kolommen, eenheidsconversie |
| Modeldiff | GlobalId-identiteit, metadata en meshes, JSON | Volumetrische vergelijking, gereviseerde triangulatie herkennen, uitgebreid wijzigingsrapport |
| Mutations | Inspectie beschikbaar | Propertybewerking, undo/redo, wijzigingshistorie en geldige STEP-export |
| Export | CSV, JSON, GLB, SVG-snede, BCF en PDF-afwijkingsoverzicht | IFC STEP, IFCX, Parquet, JSON-LD |
| 2D drawing | Horizontale driehoek/snijlijnen als SVG | Arcering, maatvoering, sheets en technische tekeningopmaak |
| Lens / kleurregels | Typekleuren en zichtbaarheid | Property-gestuurde kleuren, legenda, bewaarde regelsets |
| IDS en BCF | Zie de beperkingen in bovenstaande matrix | Volledige standaarden/dekking |
| IFC authoring | Niet aanwezig | Entiteiten/geometrie aanmaken, relaties en schema-veilige export |
| SDK / scripts / MCP | REST/OpenAPI en MCP stdio/HTTP via officiële Rust-SDK; tien tools, native beheer/testpaneel | Scriptingomgeving en API-dekking voor issues/scans/projecten |
| IFCX collaboration | Niet aanwezig | IFCX-datamodel, CRDT, server en conflict-/reconnecttests |
| Extensions | Geen uitvoerbare runtime | Manifestcontract, installatie, versiecontrole, native API |
| AI | Niet aanwezig | Expliciete lokale/providerconfiguratie en geteste toolaanroepen |
| Web embedding | Niet van toepassing op de native desktop-app | Eventueel afzonderlijke integratie; geen webview toevoegen |

## Logische UI

| Werkruimte | Doel | Uitvoer |
|---|---|---|
| Bestand | Openen, bewaren en uitwisselen | Native dialoog, project/BCF/rapport/GLB/SVG |
| Model | Objecten kiezen, isoleren en meten | Gedeelde selectie en inspecteur |
| Beeld | Camera, weergave, doorsnede en standpunt | Blijvend zichtbaar 3D-model |
| Data | Objecttabel, filters en revisies | Onderpaneel gekoppeld aan selectie |
| Controleren | Clashregels, kwaliteit, IDS, zichtlijn | Aanklikbare bevindingen |
| Issues | Beoordelen, eigenaar, opmerkingen, BCF | Vast onderpaneel met topiclijst/editor |
| Scans | Registratie, ICP en afwijkingen | Rechterpaneel naast IFC/scan-weergave |
| API / MCP | Server starten, clientconfiguratie, tools testen en activiteiten bekijken | Rechterpaneel met live verbinding naar dezelfde modellen/selectie |

Configuratiedialogen (bijvoorbeeld clashregels en IDS XML) zijn tijdelijk; modeldata, issues en scans zijn vaste werkpanelen. Niet-geïmplementeerde integraties krijgen geen schijnbaar werkende knop.

## Werkvolgorde

1. Native shell en view cube; normale Bestand-bediening en duidelijke pictogrammen.
2. Clashregels/clearance en opgeslagen projectinstellingen.
3. Issuegegevens, BCF-roundtrip en herhaalbare clashresultaten.
4. Viewer- en validatiefuncties, IDS-editor en kwaliteitsscore.
5. Pointclouds en as-built-controle.
6. Geocontext, samenwerking, Revit, AI en extensions.

De volgorde volgt afhankelijkheden: BCF en synchronisatie gebruiken dezelfde duurzame issuegegevens; afwijkings- en zichtlijncontroles leveren dezelfde issues. Een externe integratie is pas afgerond na een echte uitwisseling met de tegenpartij, niet zodra er alleen een knop of adapter bestaat.

## Verificatie

Per onderdeel: gerichte regressietests, echte IFC-fixtures en UI-controle. BCF vereist roundtriptests en referentiebestanden; clearance vereist geometrisch gescheiden objecten met bekende tussenafstand; registraties vereisen bekende transformaties; synchronisatie vereist conflicttests. De status hierboven moet tegelijk met de implementatie worden bijgewerkt.

## OpenAEC-referenties

- [Style book](https://github.com/OpenAEC-Foundation/OpenAEC-style-book): tokens, menu-/ribbonpatronen, instellingen en extensionsbeheer.
- `project-templates/Tauri+React/src/components/backstage/ExtensionManagerPanel.tsx` bevat voorbeelddata; die wordt niet als echte catalogus overgenomen.
- [Open Pointcloud Studio](https://github.com/OpenAEC-Foundation/open-pointcloud-studio), `native/desktop/src/view_cube.rs`: ontwerp- en interactiereferentie voor de eigen egui-cube.
