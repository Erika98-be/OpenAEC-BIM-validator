# Native desktoprelease

Versie: **0.3.0-rc.1**. Dit is een releasecandidate met gedeeltelijke functionele dekking, geen claim van volledige ClashControl/IfcLite-pariteit. De actuele grenzen staan in FEATURE-PARITY.md.

## Pakketten

| Platform | Runner | Pakket |
|---|---|---|
| Linux x64 | Ubuntu 22.04 | `.tar.gz`, executable; glibc 2.35 of nieuwer voor CI-build |
| Windows x64 | Windows Server 2022 | `.zip`, standalone `.exe` |
| macOS Apple Silicon | macOS 15 | `.zip` met `.app`, minimum macOS 12 |
| macOS Intel | macOS 15 Intel | `.zip` met `.app`, minimum macOS 12 |

De runtime is Rust; Python wordt alleen in CI gebruikt om pakketten te assembleren. Linux heeft OpenGL 3.3, X11/Wayland en een desktopportal nodig. Er is nog geen MSI, DMG of distributiespecifiek Linux-installatiepakket. Buildsucces bewijst geen volledige platform-UI-test; controleer openen, selecteren, dialoogvensters en export op elk platform vóór stabiele publicatie.

Elk pakket bevat README, grenzen, broncode/lockfile, buildinformatie, assetlicenties en een dependency-inventaris. Het bronarchief kan opnieuw worden gebouwd met `cargo build --release --locked`; crates en de vastgelegde pointcloud-core Git-revisie worden opgehaald via Cargo. Pointcloud-core valt onder LGPL-3.0; zie de meegeleverde notice voor bron en herlinken. Deze repository heeft nog geen algemene rootlicentie; dit pakket voegt die niet stilzwijgend toe.

## Ondertekening

Windows: Authenticode/SHA256 en RFC3161 timestamp, gevolgd door `signtool verify /pa /all`.
Mac: Developer ID Application, hardened runtime, Apple notarization, ticket stapling en Gatekeeper-verificatie.
Alle CI-pakketten: SHA256 plus GitHub/Sigstore build-provenance-attestation. De attestation bewijst herkomst; ze vervangt geen Windows-/Apple-certificaat.

Vereiste repositorysecrets (alleen via GitHub Settings invoeren):

- `WINDOWS_CERTIFICATE_BASE64`: PFX van de organisatie; `WINDOWS_CERTIFICATE_PASSWORD`.
- `MACOS_CERTIFICATE_BASE64`: Developer ID Application P12; `MACOS_CERTIFICATE_PASSWORD`; `MACOS_SIGNING_IDENTITY`.
- `APPLE_ID`, `APPLE_TEAM_ID`, `APPLE_APP_PASSWORD`: Apple notarization-account en app-specifiek wachtwoord.

Voor hardware-/cloudgebaseerde Windows-certificaten moet de SignTool-provider door de organisatie worden geconfigureerd; de huidige workflow ondersteunt aangeleverde PFX-certificaten. Geen sleutels in broncode of chat plaatsen. De macOS tijdelijke keychain en certificaatbestanden worden na gebruik verwijderd.

## Uitvoeren

1. Commit de gewenste broncode en push de branch.
2. Start `Native desktop release` via workflow_dispatch voor een proefbuild. Met `signed=false` ontstaan expliciet ongetekende proefartifacts; ze worden niet automatisch als release gepubliceerd.
3. Configureer certificaten en voer een ondertekende proefrun uit. Controleer alle vier platformjobs en de artifacts.
4. Tag de geteste commit met `v0.3.0-rc.1` en push de tag. Tag en Cargo-versie moeten overeenkomen.
5. Alleen als alle builds, tests, handtekeningen en notarization slagen, maakt de workflow een GitHub prerelease met de release notes en pakketten.

Tagged builds stoppen bij ontbrekende/ongeldige certificaten; ze publiceren nooit stilzwijgend ongetekende Windows-/macOS-pakketten. CI-attestations verifiëren met `gh attestation verify <pakket> --repo OpenAEC-Foundation/OpenAEC-BIM-validator`. Controleer checksums met `sha256sum -c <pakket>.sha256` (Windows: `Get-FileHash`).

Lokale Linux-build:

```sh
cargo build --release --locked
python3 scripts/package-native.py --target x86_64-unknown-linux-gnu
```

Een lokaal gebouwd pakket heeft geen GitHub-attestation en de glibc-eis van de gebruikte buildmachine. Het is dus niet automatisch gelijkwaardig aan de CI-release.

Bronnen: [GitHub attestations](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/increase-security-rating), [Microsoft SignTool](https://learn.microsoft.com/en-us/windows/win32/seccrypto/signtool), [Apple notarization](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution).
