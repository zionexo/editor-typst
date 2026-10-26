# Pacchettizzazione Windows (F5-016) con Inno Setup PORTABILE (skill installer-inno-setup-portabile).
# Prepara una cartella di staging con typster.exe + WebView2Loader.dll (obbligatoria per il
# build GNU: l'exe la importa dinamicamente) + NOTICE e licenses/ (obbligatori, F5-024)
# e compila il template immutabile app_cartella.iss del bundle Inno.
#
# Presuppone il toolchain host (niente --target esplicito): binario in <TargetDir>\release.
# Uso:  powershell -ExecutionPolicy Bypass -File tools\installer.ps1 [-TargetDir C:\Users\x\et_target] [-Inno <root bundle>]
param(
  [string]$TargetDir = $(if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $env:LOCALAPPDATA 'et_target' }),
  [string]$Inno = $(if ($env:INNO_SETUP_BUNDLE_ROOT) { $env:INNO_SETUP_BUNDLE_ROOT } else { Join-Path $PSScriptRoot '..\..\code_tools\inno_setup_bundle' })
)
$ErrorActionPreference = 'Stop'
# AppId esplicito e STABILE (GUID generato una volta per Typster): identifica l'installazione per
# aggiornamenti e disinstallazione. NON cambiarlo tra una versione e l'altra.
$AppId = '628922B9-55E0-4255-9A4E-35A5049F89FD'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$rel  = Join-Path $TargetDir 'release'
$iscc = Join-Path $Inno 'innosetup\ISCC.exe'
$tpl  = Join-Path $Inno 'templates\app_cartella.iss'
foreach ($f in @((Join-Path $rel 'typster.exe'), (Join-Path $rel 'WebView2Loader.dll'), $iscc, $tpl)) {
  if (-not (Test-Path $f)) { throw "File mancante: $f" }
}
$ver = (Get-Content (Join-Path $root 'src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json).version
if (-not $ver) { throw 'version assente in src-tauri\tauri.conf.json' }

$stage = Join-Path $root 'output\stage'
$out   = Join-Path $root 'output\installer'
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Force $stage, $out | Out-Null
Copy-Item (Join-Path $rel 'typster.exe') $stage
Copy-Item (Join-Path $rel 'WebView2Loader.dll') $stage
foreach ($n in 'NOTICE', 'LICENSE') { if (Test-Path (Join-Path $root $n)) { Copy-Item (Join-Path $root $n) $stage } }
if (Test-Path (Join-Path $root 'licenses')) { Copy-Item (Join-Path $root 'licenses') $stage -Recurse }
$mancanti = @('NOTICE', 'licenses') | Where-Object { -not (Test-Path (Join-Path $root $_)) }
if ($mancanti) { throw "Mancano $($mancanti -join ', '): obbligatori per l'installer (F5-024)." }

# MSYS_NO_PATHCONV/MSYS2_ARG_CONV_EXCL: innocui in PowerShell, utili se lanciato da git-bash.
$env:MSYS2_ARG_CONV_EXCL = '*'
& $iscc "/DSorgente=$stage" '/DNomeApp=Typster' "/DVersioneApp=$ver" '/DExePrincipale=typster.exe' '/DArchitetture=x64compatible' `
  '/DEditore=Nexodux' "/DAppId=$AppId" "/DIconaSetup=$(Join-Path $root 'src-tauri\icons\icon.ico')" "/O$out" $tpl
if ($LASTEXITCODE -ne 0) { throw "ISCC ha fallito (exit $LASTEXITCODE)" }
Get-ChildItem $out -Filter *.exe | Select-Object FullName, Length
