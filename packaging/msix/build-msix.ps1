# Costruisce il pacchetto MSIX NON firmato di Typster.
#
# Per lo Store di Microsoft il pacchetto va caricato senza firma: dopo la
# certificazione lo firma Microsoft. Per provarlo in locale serve la modalita'
# sviluppatore di Windows (vedi prova-locale.ps1).
#
# Prerequisiti: winapp CLI (winget install Microsoft.WinAppCli), Windows 11.
# Uso (dalla radice del repository):
#   powershell -ExecutionPolicy Bypass -File packaging\msix\build-msix.ps1
#   ... -Exe target\release\typster.exe -Dll ""     (build MSVC: nessuna DLL)
#
# Il manifesto (Package.appxmanifest) ha Name e Publisher SEGNAPOSTO: per lo Store
# vanno sostituiti con i valori assegnati da Partner Center.
param(
    [string]$Exe = "output\Typster-prova\typster.exe",
    [string]$Dll = "output\Typster-prova\WebView2Loader.dll",
    [string]$Lavoro = "output\msix"
)
$ErrorActionPreference = "Stop"
$env:WINAPP_CLI_TELEMETRY_OPTOUT = "1"
$radice = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
Set-Location $radice

if (-not (Get-Command winapp -ErrorAction SilentlyContinue)) {
    throw "winapp non trovato: installalo con 'winget install Microsoft.WinAppCli' e riapri il terminale."
}
if (-not (Test-Path $Exe)) { throw "File mancante: $Exe" }
if ($Dll -ne "" -and -not (Test-Path $Dll)) { throw "File mancante: $Dll (per una build MSVC passa -Dll '')" }

if (Test-Path $Lavoro) { Remove-Item $Lavoro -Recurse -Force }
New-Item -ItemType Directory -Force "$Lavoro\dist" | Out-Null
Copy-Item $Exe "$Lavoro\dist"
if ($Dll -ne "") { Copy-Item $Dll "$Lavoro\dist" }
Copy-Item packaging\msix\Package.appxmanifest $Lavoro
Copy-Item packaging\msix\Assets "$Lavoro\Assets" -Recurse

Set-Location $Lavoro
winapp pack .\dist --manifest .\Package.appxmanifest --no-sign
if ($LASTEXITCODE -ne 0) { throw "winapp pack e' fallito (codice $LASTEXITCODE)" }

Get-ChildItem *.msix | ForEach-Object {
    "{0}  {1:N0} byte  SHA-256 {2}" -f $_.FullName, $_.Length, (Get-FileHash $_.FullName -Algorithm SHA256).Hash
}
