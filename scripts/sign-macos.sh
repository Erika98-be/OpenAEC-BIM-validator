#!/usr/bin/env bash
set -euo pipefail
: "${MACOS_CERTIFICATE_BASE64:?Missing Developer ID certificate}"
: "${MACOS_CERTIFICATE_PASSWORD:?Missing certificate password}"
: "${MACOS_SIGNING_IDENTITY:?Missing Developer ID identity}"
: "${APPLE_ID:?Missing Apple ID}"
: "${APPLE_TEAM_ID:?Missing team ID}"
: "${APPLE_APP_PASSWORD:?Missing app-specific password}"
keychain="$RUNNER_TEMP/openaec-signing.keychain-db"
certificate="$RUNNER_TEMP/openaec-signing.p12"
keypass="$(openssl rand -hex 32)"
cleanup() { security delete-keychain "$keychain" >/dev/null 2>&1 || true; rm -f "$certificate"; }
trap cleanup EXIT
printf '%s' "$MACOS_CERTIFICATE_BASE64" | base64 --decode > "$certificate"
security create-keychain -p "$keypass" "$keychain"
security set-keychain-settings -lut 21600 "$keychain"
security unlock-keychain -p "$keypass" "$keychain"
security import "$certificate" -k "$keychain" -P "$MACOS_CERTIFICATE_PASSWORD" -T /usr/bin/codesign
security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$keypass" "$keychain" >/dev/null
apps=(dist/*/*.app)
[[ ${#apps[@]} == 1 && -d "${apps[0]}" ]] || { echo 'Expected one staged app'; exit 1; }
app="${apps[0]}"
codesign --force --options runtime --timestamp --keychain "$keychain" --sign "$MACOS_SIGNING_IDENTITY" "$app"
codesign --verify --deep --strict --verbose=2 "$app"
ditto -c -k --sequesterRsrc --keepParent "$app" "$RUNNER_TEMP/notarize.zip"
xcrun notarytool submit "$RUNNER_TEMP/notarize.zip" --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_PASSWORD" --wait
xcrun stapler staple "$app"
xcrun stapler validate "$app"
spctl --assess --type execute --verbose=2 "$app"
