# Prova locale del pacchetto MSIX di Typster, su un PC con la MODALITA' SVILUPPATORE attiva.
#
# Cosa verifica:
#   1. che il pacchetto si registri e l'app si AVVII con identita' di pacchetto;
#   2. DOVE finiscono i dati dell'utente (impostazioni, cronologia, modelli): gli MSIX
#      possono reindirizzare le scritture in %APPDATA% in una cartella privata del pacchetto.
#
# Uso (dalla radice del progetto):
#   powershell -ExecutionPolicy Bypass -File packaging\msix\prova-locale.ps1              costruisce, registra, avvia
#   powershell -ExecutionPolicy Bypass -File packaging\msix\prova-locale.ps1 -Controlla   dopo aver usato l'app: dove sono i dati?
#   powershell -ExecutionPolicy Bypass -File packaging\msix\prova-locale.ps1 -Pulisci     toglie la registrazione di prova
#   ... -SaltaBuild   riusa output\msix gia' costruito
#   ... -Exe <percorso> -Dll <percorso|""> passati a build-msix.ps1
#
# NOTA: questo script e' stato scritto ma NON ancora eseguito (il PC di sviluppo ha la
# modalita' sviluppatore bloccata). Se qualcosa non va, leggere l'errore e correggere.
param(
    [switch]$SaltaBuild,
    [switch]$Controlla,
    [switch]$Pulisci,
    [string]$Exe = "output\Typster-prova\typster.exe",
    [string]$Dll = "output\Typster-prova\WebView2Loader.dll"
)
$ErrorActionPreference = "Stop"
$env:WINAPP_CLI_TELEMETRY_OPTOUT = "1"
$radice = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
Set-Location $radice
$lav = Join-Path $radice "output\msix"
$nomePacchetto = "Nexodux.Typster"   # deve coincidere con Identity/Name del manifesto
$risultati = Join-Path $lav "RISULTATI.md"

function Modalita-Sviluppatore {
    $v = (Get-ItemProperty "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\AppModelUnlock" -ErrorAction SilentlyContinue).AllowDevelopmentWithoutDevLicense
    return ($v -eq 1)
}
function Scrivi([string]$testo) {
    Write-Host $testo
    if (Test-Path $lav) { Add-Content -Path $risultati -Value $testo -Encoding ASCII }
}

if ($Pulisci) {
    Set-Location $lav
    winapp unregister
    Write-Host "Registrazione di prova rimossa."
    exit 0
}

if (-not (Modalita-Sviluppatore)) {
    Write-Host "La modalita' sviluppatore NON e' attiva su questo PC."
    Write-Host "Attivala: Impostazioni > Privacy e sicurezza > Per sviluppatori > Modalita' sviluppatore."
    Write-Host "Se e' disattivata da criteri aziendali, usa un PC senza quel blocco."
    exit 2
}

if ($Controlla) {
    $pkg = Get-AppxPackage -Name $nomePacchetto -ErrorAction SilentlyContinue
    if (-not $pkg) { Write-Host "Pacchetto $nomePacchetto non registrato: lancia prima lo script senza -Controlla."; exit 3 }
    $pfn = $pkg.PackageFamilyName
    Scrivi ""
    Scrivi "## Controllo dei dati ($(Get-Date -Format 'yyyy-MM-dd HH:mm'))"
    Scrivi "PackageFamilyName: $pfn"
    $posti = @(
        @{ n = "APPDATA reale (come l'app non impacchettata)";          p = "$env:APPDATA\typster" },
        @{ n = "APPDATA reindirizzato dal pacchetto (Roaming)";          p = "$env:LOCALAPPDATA\Packages\$pfn\LocalCache\Roaming\typster" },
        @{ n = "LOCALAPPDATA reale (cache indice pacchetti)";            p = "$env:LOCALAPPDATA\typster" },
        @{ n = "LOCALAPPDATA reindirizzato dal pacchetto (Local)";       p = "$env:LOCALAPPDATA\Packages\$pfn\LocalCache\Local\typster" }
    )
    foreach ($q in $posti) {
        if (Test-Path $q.p) {
            $f = Get-ChildItem $q.p -Recurse -File -ErrorAction SilentlyContinue
            Scrivi ("- {0}: PRESENTE ({1} file) -> {2}" -f $q.n, @($f).Count, $q.p)
            $f | Select-Object -First 12 | ForEach-Object { Scrivi ("    " + $_.FullName.Replace($q.p, "...")) }
        } else {
            Scrivi ("- {0}: assente -> {1}" -f $q.n, $q.p)
        }
    }
    Scrivi ""
    Scrivi "Lettura: se i file di impostazioni/modelli stanno nelle cartelle 'Packages\...' l'app impacchettata"
    Scrivi "usa dati separati da quelli dell'app non impacchettata; va deciso come gestirlo."
    exit 0
}

# --- Costruzione e avvio ---
if (-not (Test-Path $lav)) { New-Item -ItemType Directory -Force $lav | Out-Null }
if (-not $SaltaBuild) {
    & powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "build-msix.ps1") -Exe $Exe -Dll $Dll
    if ($LASTEXITCODE -ne 0) { throw "build-msix.ps1 e' fallito" }
}
Set-Content -Path $risultati -Value "# Risultati della prova locale MSIX`r`n`r`nData: $(Get-Date -Format 'yyyy-MM-dd HH:mm')" -Encoding ASCII
Scrivi "PC: $env:COMPUTERNAME - $((Get-CimInstance Win32_OperatingSystem).Caption)"

Set-Location $lav
$winapp = (Get-Command winapp).Source
Scrivi "Registro e avvio il pacchetto con 'winapp run' ..."
$p = Start-Process -FilePath $winapp -ArgumentList @("run", ".\dist") -WorkingDirectory $lav -PassThru -WindowStyle Hidden
Start-Sleep -Seconds 14
$app = Get-Process -Name typster -ErrorAction SilentlyContinue
if ($app) {
    Scrivi "AVVIO: OK - typster.exe e' vivo dopo 14 s (PID $($app[0].Id))."
} else {
    Scrivi "AVVIO: FALLITO - typster.exe non e' in esecuzione dopo 14 s. Rilancia con: winapp run .\dist --verbose (da output\msix)."
    exit 4
}
$pkg = Get-AppxPackage -Name $nomePacchetto -ErrorAction SilentlyContinue
if ($pkg) { Scrivi "Pacchetto registrato: $($pkg.PackageFullName)" } else { Scrivi "ATTENZIONE: Get-AppxPackage non trova $nomePacchetto" }

Write-Host ""
Write-Host "ORA, a mano, nell'app aperta:"
Write-Host "  1. Menu Aspetto: cambia stile o tema.  2. Strumenti > Modelli > 'Salva il documento aperto come modello...': salva un modello."
Write-Host "  3. Salva un documento e modificalo (cronologia).  4. Chiudi l'app."
Write-Host "Poi lancia:  powershell -ExecutionPolicy Bypass -File packaging\msix\prova-locale.ps1 -Controlla"
Write-Host "Risultati in: $risultati"
