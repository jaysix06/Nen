param([string]$Output = 'dist/THIRD_PARTY_NOTICES.txt')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$cargo = Join-Path $env:USERPROFILE '.cargo/bin/cargo.exe'
Push-Location $projectRoot
try {
    $json = & $cargo metadata --locked --filter-platform x86_64-pc-windows-msvc --format-version 1
    if ($LASTEXITCODE -ne 0) { throw 'Dependency metadata failed.' }
    $metadata = $json | ConvertFrom-Json
    $resolved = @{}
    foreach ($node in $metadata.resolve.nodes) { $resolved[$node.id] = $true }
    $packages = $metadata.packages | Where-Object { $resolved.ContainsKey($_.id) -and $_.name -ne 'still' } | Sort-Object name,version
    $text = New-Object Text.StringBuilder
    [void]$text.AppendLine('Still: resolved Rust dependency notices (including build/test dependencies).')
    [void]$text.AppendLine('Unmodified source releases are available at the linked crate pages.')
    [void]$text.AppendLine()
    [void]$text.AppendLine('Phosphor Icons: Fill SVG artwork, embedded locally.')
    [void]$text.AppendLine([IO.File]::ReadAllText((Join-Path $projectRoot 'assets/phosphor-fill/SOURCE.md')))
    [void]$text.AppendLine([IO.File]::ReadAllText((Join-Path $projectRoot 'assets/phosphor-fill/LICENSE')))
    foreach ($package in $packages) {
        [void]$text.AppendLine()
        [void]$text.AppendLine(('=' * 72))
        [void]$text.AppendLine(($package.name + ' ' + $package.version))
        [void]$text.AppendLine(('License: ' + $package.license))
        [void]$text.AppendLine(('Source: https://crates.io/crates/' + $package.name + '/' + $package.version))
        if ($package.repository) { [void]$text.AppendLine(('Repository: ' + $package.repository)) }
        $directory = Split-Path $package.manifest_path -Parent
        $files = @(Get-ChildItem -LiteralPath $directory -File | Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE)([-_.].*)?$' })
        if ($package.license_file) {
            $explicit = Join-Path $directory $package.license_file
            if (Test-Path -LiteralPath $explicit) { $files += Get-Item -LiteralPath $explicit }
        }
        foreach ($file in ($files | Sort-Object FullName -Unique)) {
            [void]$text.AppendLine()
            [void]$text.AppendLine(('--- ' + $file.Name + ' ---'))
            [void]$text.AppendLine([IO.File]::ReadAllText($file.FullName))
        }
    }
    $path = Join-Path $projectRoot $Output
    [IO.File]::WriteAllText($path, $text.ToString())
} finally { Pop-Location }
