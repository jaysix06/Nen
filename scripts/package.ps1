$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
& (Join-Path $PSScriptRoot 'cargo.ps1') -CargoArgs @('build','--release','--locked')
if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
$destination = Join-Path $projectRoot 'dist'
New-Item -ItemType Directory -Path $destination -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $projectRoot 'target/release/still.exe') -Destination (Join-Path $destination 'Still.exe')
Copy-Item -LiteralPath (Join-Path $projectRoot 'README.md') -Destination $destination
Copy-Item -LiteralPath (Join-Path $projectRoot 'LICENSE') -Destination $destination
$documentation = Join-Path $destination 'docs'
New-Item -ItemType Directory -Path $documentation -Force | Out-Null
foreach ($name in @('architecture.md','validation.md')) {
    $source = Join-Path $projectRoot ('docs/' + $name)
    if (Test-Path -LiteralPath $source) { Copy-Item -LiteralPath $source -Destination $documentation }
}
$screenshots = Join-Path $documentation 'screenshots'
New-Item -ItemType Directory -Path $screenshots -Force | Out-Null
Get-ChildItem -LiteralPath (Join-Path $projectRoot 'docs/screenshots') -File | ForEach-Object {
    Copy-Item -LiteralPath $_.FullName -Destination $screenshots
}
& (Join-Path $PSScriptRoot 'notices.ps1')
Get-FileHash -LiteralPath (Join-Path $destination 'Still.exe') -Algorithm SHA256 | Format-List
