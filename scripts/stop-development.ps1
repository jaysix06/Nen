param([switch]$IncludePackaged)
$projectRoot = Split-Path $PSScriptRoot -Parent
$targets = @([IO.Path]::GetFullPath((Join-Path $projectRoot 'target/debug/still.exe')), [IO.Path]::GetFullPath((Join-Path $projectRoot 'target/release/still.exe')))
if ($IncludePackaged) { $targets += [IO.Path]::GetFullPath((Join-Path $projectRoot 'dist/Still.exe')) }
$running = @(Get-Process still -ErrorAction SilentlyContinue | Where-Object { $_.Path -in $targets })
if ($running.Count -eq 0) { return }
if (-not ('StillBuildLifecycle' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class StillBuildLifecycle {
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
    [StillBuildLifecycle]::Quit($process.Id)
    if (-not $process.WaitForExit(10000)) {
        throw 'Still is still running. Choose Quit from its tray menu before rebuilding.'
    }
}
