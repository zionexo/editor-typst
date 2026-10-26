# Controlla (e, a richiesta, installa) cio' che serve per lavorare su Typster
# e per provare il pacchetto MSIX su un PC nuovo.
#
# Uso (dalla radice del progetto):
#   powershell -ExecutionPolicy Bypass -File tools\setup-nuovo-pc.ps1               solo controllo
#   powershell -ExecutionPolicy Bypass -File tools\setup-nuovo-pc.ps1 -Installa      installa il minimo mancante
#   powershell -ExecutionPolicy Bypass -File tools\setup-nuovo-pc.ps1 -Installa -ConMsvc
#         ... aggiunge Rust + Visual Studio Build Tools (pesante: alcuni GB) per ricompilare l'app
#
# Il minimo per PROVARE l'MSIX gia' pronto: winapp CLI + modalita' sviluppatore.
# Il resto serve solo per ricompilare l'app (build MSVC).
param([switch]$Installa, [switch]$ConMsvc)
$ErrorActionPreference = "Continue"

function Ha([string]$cmd) { return [bool](Get-Command $cmd -ErrorAction SilentlyContinue) }
function Riga([string]$nome, [bool]$ok, [string]$nota) {
    $stato = if ($ok) { "OK    " } else { "MANCA " }
    Write-Host ("{0} {1,-34} {2}" -f $stato, $nome, $nota)
}

Write-Host "== Sistema =="
$os = Get-CimInstance Win32_OperatingSystem
Write-Host ("Windows: {0} (build {1})" -f $os.Caption, $os.BuildNumber)
$admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
Write-Host "Amministratore: $admin"
$dev = (Get-ItemProperty "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\AppModelUnlock" -ErrorAction SilentlyContinue).AllowDevelopmentWithoutDevLicense
Riga "Modalita' sviluppatore" ($dev -eq 1) "(Impostazioni > Privacy e sicurezza > Per sviluppatori)"
$wv = Get-ItemProperty "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" -ErrorAction SilentlyContinue
Riga "Runtime WebView2" ($null -ne $wv) $(if ($wv) { "versione " + $wv.pv } else { "serve per avviare Typster" })

Write-Host ""
Write-Host "== Strumenti =="
$elenco = @(
    @{ n = "winget";   c = "winget";  id = $null;                  minimo = $true;  nota = "gestore pacchetti di Windows" },
    @{ n = "git";      c = "git";     id = "Git.Git";              minimo = $true;  nota = "storia del progetto" },
    @{ n = "winapp";   c = "winapp";  id = "Microsoft.WinAppCli";  minimo = $true;  nota = "crea e prova l'MSIX" },
    @{ n = "python (py)"; c = "py";   id = "Python.Python.3.12";   minimo = $true;  nota = "controllo quota" },
    @{ n = "node";     c = "node";    id = "OpenJS.NodeJS.LTS";    minimo = $false; nota = "solo per ricompilare il frontend" },
    @{ n = "cargo";    c = "cargo";   id = "Rustlang.Rustup";      minimo = $false; nota = "solo per ricompilare (toolchain MSVC)" }
)
$mancanti = @()
foreach ($e in $elenco) {
    $ok = Ha $e.c
    Riga $e.n $ok $e.nota
    if (-not $ok -and $e.id) { $mancanti += $e }
}
# Build Tools C++ (serve a Rust/MSVC)
$vsw = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$haVc = $false
if (Test-Path $vsw) { $haVc = [bool](& $vsw -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath) }
Riga "Visual Studio Build Tools (C++)" $haVc "solo per ricompilare"

if (-not $Installa) {
    Write-Host ""
    Write-Host "Solo controllo. Per installare il minimo mancante: aggiungi -Installa (e -ConMsvc per ricompilare)."
    exit 0
}
if (-not (Ha "winget")) { Write-Host "winget non disponibile: installa 'App Installer' dal Microsoft Store."; exit 1 }

Write-Host ""
Write-Host "== Installazione =="
foreach ($e in $mancanti) {
    if ($e.minimo -or $ConMsvc) {
        Write-Host ("Installo {0} ..." -f $e.id)
        winget install $e.id --source winget --accept-package-agreements --accept-source-agreements
    }
}
if ($ConMsvc -and -not $haVc) {
    Write-Host "Installo Visual Studio Build Tools con il carico di lavoro C++ (puo' richiedere molto tempo) ..."
    winget install Microsoft.VisualStudio.2022.BuildTools --source winget --accept-package-agreements --accept-source-agreements --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
}
if ($ConMsvc -and (Ha "rustup")) {
    rustup toolchain install 1.96.0 --profile minimal -c rustfmt -c clippy
    rustup default 1.96.0-x86_64-pc-windows-msvc
}
Write-Host ""
Write-Host "Fatto. CHIUDI e RIAPRI il terminale per aggiornare il PATH, poi rilancia questo script senza -Installa per ricontrollare."
