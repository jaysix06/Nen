param([Parameter(ValueFromRemainingArguments = $true)][string[]]$CargoArgs)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
if ($CargoArgs.Count -gt 0 -and $CargoArgs[0] -in @('build','test','run','rustc')) {
    & (Join-Path $PSScriptRoot 'stop-development.ps1')
}
$cargoBin = Join-Path $env:USERPROFILE '.cargo/bin'
$env:Path = "$cargoBin;$env:Path"
$portableRoot = Join-Path $projectRoot '.tools/msvc'
if (Test-Path $portableRoot) {
    $compiler = Get-ChildItem "$portableRoot/VC/Tools/MSVC" -Directory | Sort-Object Name -Descending | Select-Object -First 1
    $sdk = Get-ChildItem "$portableRoot/Windows Kits/10/Lib" -Directory | Sort-Object Name -Descending | Select-Object -First 1
    if ($compiler -and $sdk) {
        $vc = $compiler.FullName
        $kit = "$portableRoot/Windows Kits/10"
        $version = $sdk.Name
        $env:Path = "$vc/bin/Hostx64/x64;$kit/bin/$version/x64;$env:Path"
        $env:INCLUDE = "$vc/include;$kit/Include/$version/ucrt;$kit/Include/$version/shared;$kit/Include/$version/um;$kit/Include/$version/winrt"
        $env:LIB = "$vc/lib/x64;$kit/Lib/$version/ucrt/x64;$kit/Lib/$version/um/x64"
        $env:GPUI_FXC_PATH = "$kit/bin/$version/x64/fxc.exe"
    }
}
Push-Location $projectRoot
try {
    # Windows PowerShell 5 treats native stderr as an error stream.
    # Cargo writes normal build progress there; its exit code determines success.
    $ErrorActionPreference = 'Continue'
    & cargo @CargoArgs
    exit $LASTEXITCODE
} finally {
    Pop-Location
}
