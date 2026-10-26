# Misure dei requisiti non funzionali (F5-018, PROGETTO.md §9). Ripetibile, senza percorsi assoluti.
# Uso:  powershell -ExecutionPolicy Bypass -File tools\misura_nfr.ps1 [-Avvii 5] [-AttesaMemoria 10] [-SaltaAnteprima]
# Exe di produzione: <TargetDir>\release\typster.exe con TargetDir = $env:CARGO_TARGET_DIR, altrimenti
# $env:LOCALAPPDATA\et_target. Setup: output\installer\*.exe (prodotto da tools\installer.ps1).
# Il "primo avvio a freddo" e' il primo avvio dello script (dipende dalla cache di sistema: e' quello
# subito dopo una build/riavvio a essere davvero freddo); segue un avvio di riscaldamento scartato.
# L'app non resta mai aperta: ogni avvio e' chiuso con CloseMainWindow e, se serve, Stop-Process.
param(
  [int]$Avvii = 5,
  [int]$AttesaMemoria = 10,
  [switch]$SaltaAnteprima
)
$ErrorActionPreference = 'Stop'
$radice = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $env:LOCALAPPDATA 'et_target' }
$exe = Join-Path $target 'release\typster.exe'
if (-not (Test-Path $exe)) { throw "Exe di produzione mancante: $exe" }

function Chiudi([System.Diagnostics.Process]$p) {
  if ($p.HasExited) { return }
  [void]$p.CloseMainWindow()
  if (-not $p.WaitForExit(5000)) { Stop-Process -Id $p.Id -Force; $p.WaitForExit() }
  # Attende anche gli eventuali msedgewebview2 orfani del processo chiuso.
  Start-Sleep -Milliseconds 1500
}

# Un avvio: ritorna i ms fino a finestra visibile (MainWindowHandle != 0 e titolo con "Typster").
function Avvia([switch]$Tieni) {
  $sw = [System.Diagnostics.Stopwatch]::StartNew()
  $p = Start-Process -FilePath $exe -PassThru
  $ms = $null
  while ($sw.Elapsed.TotalSeconds -lt 30) {
    $p.Refresh()
    if ($p.HasExited) { throw "typster.exe terminato prematuramente (exit $($p.ExitCode))" }
    if ($p.MainWindowHandle -ne 0 -and $p.MainWindowTitle -like '*Typster*') { $ms = $sw.ElapsedMilliseconds; break }
    Start-Sleep -Milliseconds 10
  }
  if ($null -eq $ms) { Chiudi $p; throw 'Timeout: finestra non comparsa entro 30 s' }
  if ($Tieni) { return @{ Processo = $p; Ms = $ms } }
  Chiudi $p
  return $ms
}

function Mediana([double[]]$v) {
  $s = $v | Sort-Object; $n = $s.Count
  if ($n % 2) { $s[[int](($n - 1) / 2)] } else { ($s[$n / 2 - 1] + $s[$n / 2]) / 2 }
}

"== Dimensioni =="
$mb = { param($b) '{0:N2} MB ({1:N0} B)' -f ($b / 1MB), $b }
"typster.exe : $(& $mb (Get-Item $exe).Length)"
$dll = Join-Path (Split-Path $exe) 'WebView2Loader.dll'
if (Test-Path $dll) { "WebView2Loader.dll : $(& $mb (Get-Item $dll).Length)" }
$setup = Get-ChildItem (Join-Path $radice 'output\installer') -Filter *.exe -ErrorAction SilentlyContinue | Sort-Object LastWriteTime | Select-Object -Last 1
if ($setup) { "setup : $($setup.Name) $(& $mb $setup.Length)" } else { 'setup : assente (eseguire tools\installer.ps1)' }

"== Avvio -> finestra visibile (ms) =="
$freddo = Avvia
"primo avvio (a freddo, non riscaldato): $freddo"
[void](Avvia)   # riscaldamento scartato
$tempi = 1..$Avvii | ForEach-Object { Avvia }
"avvii ($Avvii): $($tempi -join ', ')"
"mediana: $(Mediana $tempi)  min: $(($tempi | Measure-Object -Minimum).Minimum)  max: $(($tempi | Measure-Object -Maximum).Maximum)"

"== Memoria a riposo (dopo $AttesaMemoria s) =="
$r = Avvia -Tieni
try {
  Start-Sleep -Seconds $AttesaMemoria
  # Catena di discendenti di typster.exe via Win32_Process.ParentProcessId.
  $tutti = Get-CimInstance Win32_Process
  $ids = New-Object System.Collections.Generic.HashSet[int]
  [void]$ids.Add($r.Processo.Id)
  do {
    $prima = $ids.Count
    foreach ($q in $tutti) { if ($ids.Contains([int]$q.ParentProcessId)) { [void]$ids.Add([int]$q.ProcessId) } }
  } while ($ids.Count -ne $prima)
  $righe = foreach ($id in $ids) {
    $q = Get-Process -Id $id -ErrorAction SilentlyContinue
    if ($q) { [pscustomobject]@{ Pid = $id; Nome = $q.ProcessName; WS_MB = [math]::Round($q.WorkingSet64 / 1MB, 1); Private_MB = [math]::Round($q.PrivateMemorySize64 / 1MB, 1) } }
  }
  $righe | Sort-Object Nome, Pid | Format-Table -AutoSize | Out-String -Width 120
  "TOTALE ($($righe.Count) processi): WorkingSet {0:N1} MB, PrivateBytes {1:N1} MB" -f ($righe | Measure-Object WS_MB -Sum).Sum, ($righe | Measure-Object Private_MB -Sum).Sum
} finally { Chiudi $r.Processo }

"== Latenza anteprima (core, profilo release) =="
if ($SaltaAnteprima) { 'saltata' } else { & (Join-Path $PSScriptRoot 'misura.ps1') }

"== Offline =="
'Nessuna rete all''avvio: ureq e'' usato solo in crates\typst-core\src\packages.rs (indice pacchetti) e dal downloader typst-kit'
'(scarica pacchetti @preview). Verifica: Select-String -Path crates,src-tauri\src -Pattern ureq,reqwest -Recurse'
Get-ChildItem (Join-Path $radice 'crates'), (Join-Path $radice 'src-tauri\src') -Recurse -Include *.rs | Select-String -Pattern 'ureq::|reqwest' | ForEach-Object { "  $($_.Path.Substring($radice.Length + 1)):$($_.LineNumber)" }

$resta = Get-Process typster -ErrorAction SilentlyContinue
if ($resta) { $resta | Stop-Process -Force; 'ATTENZIONE: typster rimasto aperto, chiuso a forza' } else { 'nessun typster residuo' }
