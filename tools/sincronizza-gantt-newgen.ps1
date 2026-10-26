# Copia nel repository il pacchetto Typst "gantt-newgen" (sviluppato a parte, cartella
# typst_gantty) cosi' com'e' pubblicabile: solo i file che il pacchetto distribuisce
# (typst.toml, lib.typ, LICENSE, COPYING, src\, data\locales\). Il core lo incorpora
# nell'eseguibile (crates\typst-core\pacchetti\gantt-newgen, vedi build.rs).
#
# Rifiuta di copiare se nei file compaiono dati personali o percorsi locali.
# Scrive ORIGINE.txt (versione, data, impronta SHA-256 dell'insieme dei file); ORIGINE.txt
# NON viene incorporato nell'eseguibile.
#
# Uso (dalla radice del progetto):
#   powershell -ExecutionPolicy Bypass -File tools\sincronizza-gantt-newgen.ps1
#   ... -Origine "C:\percorso\typst_gantty"     (altra cartella sorgente)
param(
    [string]$Origine = "C:\Local Storage\wpy\3.11dot\notebooks\typst_gantty"
)
$ErrorActionPreference = "Stop"
$radice = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$dest = Join-Path $radice "crates\typst-core\pacchetti\gantt-newgen"
$origine = (Resolve-Path $Origine).Path

# File distribuiti dal pacchetto (stessa logica di `exclude` in typst.toml, ma esplicita).
$file = @("typst.toml", "lib.typ", "LICENSE", "COPYING")
$file += Get-ChildItem (Join-Path $origine "src") -Recurse -File | ForEach-Object { $_.FullName.Substring($origine.Length + 1) }
$file += Get-ChildItem (Join-Path $origine "data\locales") -File | ForEach-Object { $_.FullName.Substring($origine.Length + 1) }
$file = $file | Sort-Object

# Controllo dati personali (LICENSE e COPYING sono testi di licenza: esclusi dal controllo)
$schema = [regex]::Escape($env:USERNAME) + '|C:[\\/]|/c/Users|Local Storage|@[A-Za-z0-9.-]+\.(com|it|net|org)\b|gmail|outlook'
$trovati = $file | Where-Object { $_ -notin @("LICENSE", "COPYING") } | ForEach-Object {
    Select-String -Path (Join-Path $origine $_) -Pattern $schema -ErrorAction SilentlyContinue
}
if ($trovati) {
    $trovati | ForEach-Object { Write-Host ("{0}:{1}: {2}" -f $_.Path, $_.LineNumber, $_.Line.Trim()) }
    throw "Dati personali o percorsi locali nei file del pacchetto: pulisci la sorgente prima di sincronizzare."
}

if (Test-Path $dest) { Remove-Item $dest -Recurse -Force }
New-Item -ItemType Directory -Force $dest | Out-Null
$impronte = @()
foreach ($f in $file) {
    $a = Join-Path $origine $f
    $b = Join-Path $dest $f
    New-Item -ItemType Directory -Force (Split-Path $b -Parent) | Out-Null
    Copy-Item $a $b
    $impronte += "{0}  {1}" -f (Get-FileHash $b -Algorithm SHA256).Hash.ToLower(), $f.Replace("\", "/")
}
$insieme = [System.BitConverter]::ToString(
    [System.Security.Cryptography.SHA256]::Create().ComputeHash([System.Text.Encoding]::UTF8.GetBytes(($impronte -join "`n")))
).Replace("-", "").ToLower()
$versione = (Select-String -Path (Join-Path $dest "typst.toml") -Pattern '^version\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
$testo = @(
    "Pacchetto: gantt-newgen $versione (autore: nexodux, LGPL-3.0-or-later)",
    "Sincronizzato il: $(Get-Date -Format 'yyyy-MM-dd HH:mm')",
    "File: $($file.Count)   Impronta dell'insieme (SHA-256): $insieme",
    "Origine: cartella del progetto 'typst_gantty' (sviluppo separato); non modificare qui:",
    "rilanciare tools\sincronizza-gantt-newgen.ps1 dopo ogni modifica del pacchetto.",
    ""
) + $impronte
[System.IO.File]::WriteAllText((Join-Path $dest "ORIGINE.txt"), (($testo -join "`n") + "`n"), (New-Object System.Text.UTF8Encoding($false)))
Write-Host "Copiati $($file.Count) file in $dest"
Write-Host "gantt-newgen $versione  impronta $insieme"
