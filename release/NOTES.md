# OpenAEC BIM Validator 0.3.0-rc.1

Native IFC-coördinatie in Rust, met IfcLite 20.4.0 en OpenAEC-vormgeving. Deze releasecandidate brengt modelinspectie, controles, issues en as-built-vergelijking samen.

## Beschikbare functies

- Meerdere IFC2X3/4/4X3 STEP-modellen, modelboom en property-inspecteur.
- Orbit/walk, perspectief/orthografisch, camera-afhankelijke view cube, doorsnedebox, zichtbaarheid, afstand en vlakke oppervlaktemeting.
- Werkruimtes Model, Beeld, Data, Controleren, Issues en Scans met gedeelde 3D-selectie.
- Objecttabellen, tekst/type/propertyfilters, numerieke vergelijking en CSV-export.
- Revisievergelijking op GlobalId voor metadata en meshes; JSON-export.
- GLB-export van zichtbare geometrie en SVG-export van horizontale doorsneden.
- IfcLite hard-clash- en clearancecontroles, model-/typefilters en opgeslagen regels.
- Zichtlijncontrole en configureerbare modelkwaliteitscontroles.
- Gedeeltelijke IDS-validatie en XML-editor.
- Lokale issues: status, prioriteit, eigenaar, opmerkingen, snapshots; BCF 2.1/3.0-import/export.
- LAS/LAZ/PLY/PCD/XYZ/PTS/PTX/E57-scans, registratie/ICP, afwijkingsheatmap, clusterissues en PDF-overzicht.
- Lokale projectbestanden en standpunten; licht/donker thema.
- Headless CLI voor inspectie, queries, basiskwaliteitscontrole, revisievergelijking en GLB/SVG-export.

## Grenzen van deze candidate

Dit is nog geen volledige overname van ClashControl of IfcLite. IFC-propertybewerking/undo/STEP-export, IFCX/CRDT, SQL, scripting/MCP/AI, Revit-connector, gedeelde projecten en extension-runtime ontbreken. IDS en BCF hebben gedeeltelijke dekking. Scans gebruiken maximaal 2 miljoen steekproefpunten. Revisievergelijking detecteert triangulatieverschillen en is geen volumetrische vergelijking. Bekijk [de volledige functiematrix](https://github.com/OpenAEC-Foundation/OpenAEC-BIM-validator/blob/master/docs/FEATURE-PARITY.md).

## Installatie en verificatie

Pak het artifact voor je platform uit. Windows: start de `.exe`; macOS: kopieer de `.app` naar Applications; Linux: start de executable in een grafische sessie met OpenGL 3.3 en een desktopportal. Projectbestanden verwijzen naar externe IFC-/scanbestanden; die moeten beschikbaar blijven.

De ondertekende releaseworkflow maakt Windows x64, Linux x64, macOS ARM64 en macOS x64. Bij deze notes horen alleen artifacts uit een geslaagde releaseworkflow, met checksums en GitHub build-attestation. Een ongetekende lokale/proefbuild is geen ondertekende release.
