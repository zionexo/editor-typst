# Rigenera le misure di prestazione del core (F5-007/008/009).
# Uso: powershell -ExecutionPolicy Bypass -File tools\misura.ps1
# Bundle Rust: $env:RUST_BUNDLE, altrimenti <repo>\..\code_tools\rust_bundle.
# Target dir: $env:CARGO_TARGET_DIR, altrimenti $env:LOCALAPPDATA\et_target (percorso senza spazi).
$radice = Split-Path -Parent $PSScriptRoot
$bundle = if ($env:RUST_BUNDLE) { $env:RUST_BUNDLE } else { Join-Path $radice "..\code_tools\rust_bundle" }
. (Join-Path $bundle "activate.ps1")
Set-Location $radice
if (-not $env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR = Join-Path $env:LOCALAPPDATA "et_target" }
cargo run --release -q -p typst-core --example misura
