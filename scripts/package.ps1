$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
& (Join-Path $PSScriptRoot 'cargo.ps1') -CargoArgs @('build','--release','--locked')
if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
& (Join-Path $PSScriptRoot 'stop-development.ps1') -IncludePackaged
$destination = Join-Path $projectRoot 'dist'
New-Item -ItemType Directory -Path $destination -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $projectRoot 'target/release/nen.exe') -Destination (Join-Path $destination 'Nen.exe')
$legacyExecutable = Join-Path $destination 'Still.exe'
# Preserve an already-enabled startup entry belonging to this exact packaged executable.
$startup = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Run', $true)
try {
    if ($startup -and $startup.GetValue('Still') -ieq ('"' + $legacyExecutable + '" --startup')) {
        $startup.SetValue('Nen', ('"' + (Join-Path $destination 'Nen.exe') + '" --startup'))
        $startup.DeleteValue('Still')
    }
} finally { if ($startup) { $startup.Dispose() } }
if (Test-Path -LiteralPath $legacyExecutable) { Remove-Item -LiteralPath $legacyExecutable }
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
Get-FileHash -LiteralPath (Join-Path $destination 'Nen.exe') -Algorithm SHA256 | Format-List
