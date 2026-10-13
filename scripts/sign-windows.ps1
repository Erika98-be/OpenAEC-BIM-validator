$ErrorActionPreference = 'Stop'
foreach ($name in @('WINDOWS_CERTIFICATE_BASE64', 'WINDOWS_CERTIFICATE_PASSWORD')) {
  if (-not [Environment]::GetEnvironmentVariable($name)) { throw "Missing signing secret: $name" }
}
$certificate = Join-Path $env:RUNNER_TEMP 'openaec-signing.pfx'
try {
  [IO.File]::WriteAllBytes($certificate, [Convert]::FromBase64String($env:WINDOWS_CERTIFICATE_BASE64))
  $signtool = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin\*\x64\signtool.exe' | Sort-Object FullName -Descending | Select-Object -First 1
  if (-not $signtool) { throw 'Windows SDK SignTool missing' }
  $binary = Get-ChildItem 'dist\*\openaec-bim-validator.exe'
  if (@($binary).Count -ne 1) { throw 'Expected exactly one staged executable' }
  & $signtool.FullName sign /f $certificate /p $env:WINDOWS_CERTIFICATE_PASSWORD /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 $binary.FullName
  if ($LASTEXITCODE -ne 0) { throw 'Authenticode signing failed' }
  & $signtool.FullName verify /pa /all /v $binary.FullName
  if ($LASTEXITCODE -ne 0) { throw 'Authenticode verification failed' }
} finally {
  Remove-Item $certificate -ErrorAction SilentlyContinue
}
