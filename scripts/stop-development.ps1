param([switch]$IncludePackaged)
$projectRoot = Split-Path $PSScriptRoot -Parent
$targets = @('target/debug/nen.exe','target/release/nen.exe','target/debug/still.exe','target/release/still.exe') | ForEach-Object { [IO.Path]::GetFullPath((Join-Path $projectRoot $_)) }
if ($IncludePackaged) { $targets += @('dist/Nen.exe','dist/Still.exe') | ForEach-Object { [IO.Path]::GetFullPath((Join-Path $projectRoot $_)) } }
$running = @(Get-Process -Name nen,still -ErrorAction SilentlyContinue | Where-Object { $_.Path -in $targets })
if ($running.Count -eq 0) { return }
if (-not ('NenBuildLifecycle' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class NenBuildLifecycle {
    private delegate bool EnumProc(IntPtr hwnd, IntPtr parameter);
    [DllImport("user32.dll")] private static extern bool EnumWindows(EnumProc callback, IntPtr parameter);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);
    [DllImport("user32.dll")] private static extern bool PostMessage(IntPtr hwnd, uint message, IntPtr wparam, IntPtr lparam);
    public static void Quit(uint target) {
        EnumWindows((hwnd, parameter) => {
            uint processId; GetWindowThreadProcessId(hwnd, out processId);
            if (processId == target) PostMessage(hwnd, 0x8410, (IntPtr)1, IntPtr.Zero);
            return true;
        }, IntPtr.Zero);
    }
}
'@
}
foreach ($process in $running) {
    [NenBuildLifecycle]::Quit($process.Id)
    if (-not $process.WaitForExit(10000)) {
        throw 'Nen is still running. Choose Quit from its tray menu before rebuilding.'
    }
}
