# LinkedIn — concept, nog niet gepubliceerd

We bouwen OpenAEC BIM Validator om tot een native IFC-viewer en coördinatieomgeving, volledig in Rust.

De nieuwe interface brengt de dagelijkse BIM-workflow samen: modellen openen, objectgegevens onderzoeken, clashes controleren, issues beoordelen en scans met het ontwerp vergelijken. Met de OpenAEC-huisstijl als basis en IfcLite voor parsing, geometrie en clashdetectie.

Wat er nu werkt:
• Meerdere IFC-modellen bekijken, doorsnijden en meten.
• Objecttabellen filteren en exporteren, en modelrevisies vergelijken op GlobalId.
• Harde clashes en benodigde vrije ruimte controleren.
• Issues beheren en uitwisselen via BCF 2.1/3.0.
• Puntenwolken registreren en afwijkingen ten opzichte van IFC onderzoeken.
• 3D-geometrie naar GLB en horizontale doorsneden naar SVG exporteren.
• Andere tools verbinden via de lokale API en MCP-server, met beheer en testen in de app.

De richting is een samenhangende, open BIM-werkplek. Dit is een releasecandidate: volledige IDS-dekking, gedeelde projecten, Revit-integratie en de overige functionaliteit van ClashControl en IfcLite zijn nog in ontwikkeling.

We zoeken BIM-coördinatoren die de workflow met hun eigen modellen willen beproeven. Welke controle of dagelijkse handeling kost jou nu het meeste tijd?

https://github.com/OpenAEC-Foundation/OpenAEC-BIM-validator

#OpenAEC #OpenBIM #IFC #BIM #Rust

---

Publicatievoorbereiding:
- Voeg docs/native-viewer.png, docs/native-data.png en docs/native-issues.png toe als carrousel. Dit zijn echte appbeelden, geen ontwerpvisualisaties.
- Controleer vóór publicatie dat de release daadwerkelijk op GitHub staat; voeg dan de definitieve releaselink toe.
- Claim Windows-/macOS-beschikbaarheid en ondertekening pas nadat de vier platformjobs en certificaatcontroles zijn geslaagd.
- Dit document is uitsluitend een concept; er is niets naar LinkedIn verzonden.
