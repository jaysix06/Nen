param([string]$Executable = 'target/release/nen.exe', [int]$IdleSeconds = 10)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$binary = (Resolve-Path (Join-Path $projectRoot $Executable)).Path
$data = Join-Path $projectRoot ('.tools/profile-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $data -Force | Out-Null
if (-not ('NenProfileWindow' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class NenProfileWindow {
    private delegate bool EnumProc(IntPtr hwnd, IntPtr parameter);
    [DllImport("user32.dll")] private static extern bool EnumWindows(EnumProc callback, IntPtr parameter);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);
    [DllImport("user32.dll")] private static extern bool PostMessage(IntPtr hwnd, uint message, IntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll")] private static extern bool ShowWindowAsync(IntPtr hwnd, int command);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetWindowText(IntPtr hwnd, System.Text.StringBuilder text, int length);
    public static IntPtr MainWindow(uint target) {
        IntPtr result = IntPtr.Zero;
        EnumWindows((hwnd, parameter) => {
            uint processId; GetWindowThreadProcessId(hwnd, out processId);
            if (processId == target) {
                var title = new System.Text.StringBuilder(100);
                GetWindowText(hwnd, title, 100);
                if (title.ToString() == "Nen") { result = hwnd; return false; }
            }
            return true;
        }, IntPtr.Zero);
        return result;
    }
    public static void Show(IntPtr window) { PostMessage(window, 0x8410, IntPtr.Zero, IntPtr.Zero); }
    public static void Hide(IntPtr window) { ShowWindowAsync(window, 0); }
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
$previousData = $env:NEN_DATA_DIR
$process = $null
try {
    $env:NEN_DATA_DIR = $data
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $process = Start-Process -FilePath $binary -ArgumentList '--demo' -PassThru -WindowStyle Hidden
    do {
        Start-Sleep -Milliseconds 20
        $process.Refresh()
        if ($process.HasExited) { throw 'Nen exited before its window opened.' }
        if ($watch.Elapsed.TotalSeconds -gt 15) { throw 'Timed out waiting for Nen.' }
        $window = [NenProfileWindow]::MainWindow($process.Id)
    } while ($window -eq [IntPtr]::Zero)
    $windowMs = $watch.Elapsed.TotalMilliseconds
    [NenProfileWindow]::Show($window)
    Start-Sleep -Seconds 2
    $process.Refresh()
    $cpu = $process.TotalProcessorTime.TotalMilliseconds
    $sample = [Diagnostics.Stopwatch]::StartNew()
    Start-Sleep -Seconds $IdleSeconds
    $process.Refresh()
    if ($process.HasExited) { throw 'The profiled process exited during measurement.' }
    $foregroundCpu = ($process.TotalProcessorTime.TotalMilliseconds - $cpu) / $sample.Elapsed.TotalMilliseconds * 100
    [NenProfileWindow]::Hide($window)
    Start-Sleep -Seconds 1
    $process.Refresh()
    $cpu = $process.TotalProcessorTime.TotalMilliseconds
    $sample.Restart()
    Start-Sleep -Seconds $IdleSeconds
    $process.Refresh()
    if ($process.HasExited) { throw 'The profiled process exited during measurement.' }
    [PSCustomObject]@{
        WindowCreatedMs = [Math]::Round($windowMs, 1)
        WorkingSetMiB = [Math]::Round($process.WorkingSet64 / 1MB, 1)
        PrivateMemoryMiB = [Math]::Round($process.PrivateMemorySize64 / 1MB, 1)
        VisibleIdleCpuOneCorePercent = [Math]::Round($foregroundCpu, 3)
        TrayIdleCpuOneCorePercent = [Math]::Round(($process.TotalProcessorTime.TotalMilliseconds - $cpu) / $sample.Elapsed.TotalMilliseconds * 100, 3)
        SampleSeconds = [Math]::Round($sample.Elapsed.TotalSeconds, 1)
    } | ConvertTo-Json
} finally {
    if ($process -and -not $process.HasExited) {
        [NenProfileWindow]::Quit($process.Id)
        if (-not $process.WaitForExit(10000)) { Write-Warning 'Choose Quit in the test instance tray menu to finish the profile.' }
    }
    $env:NEN_DATA_DIR = $previousData
}
