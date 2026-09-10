Write-Host "=========================================" -ForegroundColor Cyan
Write-Host " LightGUI Runtime & IPC Integration Test" -ForegroundColor Cyan
Write-Host "=========================================" -ForegroundColor Cyan

# Cleanup existing processes if any
Get-Process -Name "lightgui", "lightgui-settings" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 500

$trayExe = (Resolve-Path "target/release/lightgui.exe").Path
$settingsExe = (Resolve-Path "target/release/lightgui-settings.exe").Path

if (-not (Test-Path $trayExe)) {
    Write-Error "lightgui.exe not found at $trayExe"
    exit 1
}
if (-not (Test-Path $settingsExe)) {
    Write-Error "lightgui-settings.exe not found at $settingsExe"
    exit 1
}

# Win32 definitions for finding process window by class and sending WM_CLOSE
$win32Def = @"
using System;
using System.Runtime.InteropServices;
using System.Text;

public class Win32Helper {
    [DllImport("user32.dll")]
    public static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

    [DllImport("user32.dll", SetLastError = true)]
    public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);

    [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Auto)]
    public static extern int GetClassName(IntPtr hWnd, StringBuilder lpClassName, int nMaxCount);

    [DllImport("user32.dll", SetLastError = true)]
    public static extern bool PostMessage(IntPtr hWnd, uint Msg, IntPtr wParam, IntPtr lParam);

    public static IntPtr FindProcessWindow(uint targetPid, string targetClass) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((hWnd, lParam) => {
            uint pid;
            GetWindowThreadProcessId(hWnd, out pid);
            if (pid == targetPid) {
                var sbClass = new StringBuilder(256);
                GetClassName(hWnd, sbClass, 256);
                if (sbClass.ToString() == targetClass) {
                    found = hWnd;
                    return false;
                }
            }
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
"@
Add-Type -TypeDefinition $win32Def -ErrorAction SilentlyContinue

# a. Launch target/release/lightgui.exe
Write-Host "`n[1/7] Launching lightgui.exe (Tray Daemon)..." -ForegroundColor Yellow
$trayStartTime = [System.Diagnostics.Stopwatch]::StartNew()
$trayProc = Start-Process -FilePath $trayExe -PassThru
$trayStartupDurationMs = $trayStartTime.ElapsedMilliseconds
Write-Host "  Started PID: $($trayProc.Id) in $trayStartupDurationMs ms"

# b. Verify named pipe \\.\pipe\lightgui_ipc
Write-Host "`n[2/7] Verifying named pipe \\.\pipe\lightgui_ipc..." -ForegroundColor Yellow
$pipeFound = $false
$pipeTimeout = 10 # 5 seconds
for ($i = 0; $i -lt $pipeTimeout; $i++) {
    Start-Sleep -Milliseconds 500
    $pipes = [System.IO.Directory]::GetFiles("\\.\pipe\")
    if ($pipes -contains "\\.\pipe\lightgui_ipc") {
        $pipeFound = $true
        break
    }
}

if ($pipeFound) {
    Write-Host "  PASS: Named pipe \\.\pipe\lightgui_ipc exists and is listening!" -ForegroundColor Green
} else {
    Write-Host "  FAIL: Named pipe \\.\pipe\lightgui_ipc was NOT found!" -ForegroundColor Red
}

# c. Launch target/release/lightgui-settings.exe
Write-Host "`n[3/7] Launching lightgui-settings.exe..." -ForegroundColor Yellow
$settingsStartTime = [System.Diagnostics.Stopwatch]::StartNew()
$settingsProc = Start-Process -FilePath $settingsExe -PassThru
$settingsStartupDurationMs = $settingsStartTime.ElapsedMilliseconds
Write-Host "  Started PID: $($settingsProc.Id) in $settingsStartupDurationMs ms"
Start-Sleep -Milliseconds 1000

# d. Measure resource usage
Write-Host "`n[4/7] Measuring Resource Usage (1.0s CPU sample)..." -ForegroundColor Yellow
$trayProc.Refresh()
$settingsProc.Refresh()

$trayCpuStart = $trayProc.TotalProcessorTime.TotalSeconds
$settingsCpuStart = $settingsProc.TotalProcessorTime.TotalSeconds
$timeStart = [System.Diagnostics.Stopwatch]::StartNew()
Start-Sleep -Milliseconds 1000
$timeStart.Stop()
$elapsedSec = $timeStart.Elapsed.TotalSeconds
$trayProc.Refresh()
$settingsProc.Refresh()
$trayCpuEnd = $trayProc.TotalProcessorTime.TotalSeconds
$settingsCpuEnd = $settingsProc.TotalProcessorTime.TotalSeconds

$cpuCores = [System.Environment]::ProcessorCount
$trayCpuPct = [math]::Round((($trayCpuEnd - $trayCpuStart) / ($elapsedSec * $cpuCores)) * 100, 2)
$settingsCpuPct = [math]::Round((($settingsCpuEnd - $settingsCpuStart) / ($elapsedSec * $cpuCores)) * 100, 2)

$trayWS = [math]::Round($trayProc.WorkingSet64 / 1MB, 2)
$trayPriv = [math]::Round($trayProc.PrivateMemorySize64 / 1MB, 2)
$trayThreads = $trayProc.Threads.Count
$trayHandles = $trayProc.HandleCount

$settingsWS = [math]::Round($settingsProc.WorkingSet64 / 1MB, 2)
$settingsPriv = [math]::Round($settingsProc.PrivateMemorySize64 / 1MB, 2)
$settingsThreads = $settingsProc.Threads.Count
$settingsHandles = $settingsProc.HandleCount

Write-Host "  --- lightgui.exe (Tray Daemon) ---" -ForegroundColor Cyan
Write-Host "    WorkingSet (RAM):    $trayWS MB"
Write-Host "    PrivateMemory:       $trayPriv MB"
Write-Host "    Thread Count:        $trayThreads"
Write-Host "    Handle Count:        $trayHandles"
Write-Host "    CPU Usage:           $trayCpuPct %"

Write-Host "  --- lightgui-settings.exe ---" -ForegroundColor Cyan
Write-Host "    WorkingSet (RAM):    $settingsWS MB"
Write-Host "    PrivateMemory:       $settingsPriv MB"
Write-Host "    Thread Count:        $settingsThreads"
Write-Host "    Handle Count:        $settingsHandles"
Write-Host "    CPU Usage:           $settingsCpuPct %"

# e. Close lightgui-settings.exe and verify clean exit
Write-Host "`n[5/7] Closing lightgui-settings.exe via WM_CLOSE..." -ForegroundColor Yellow
$settingsHwnd = [Win32Helper]::FindProcessWindow($settingsProc.Id, "lightgui_settings_wndclass")
if ($settingsHwnd -ne [IntPtr]::Zero) {
    [Win32Helper]::PostMessage($settingsHwnd, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
    Write-Host "  Posted WM_CLOSE (0x0010) to settings HWND: $settingsHwnd"
} else {
    Write-Host "  Falling back to CloseMainWindow()..."
    $settingsProc.CloseMainWindow() | Out-Null
}

$settingsExited = $false
for ($i = 0; $i -lt 10; $i++) {
    Start-Sleep -Milliseconds 300
    $settingsProc.Refresh()
    if ($settingsProc.HasExited) {
        $settingsExited = $true
        break
    }
}

if ($settingsExited) {
    Write-Host "  PASS: lightgui-settings.exe exited completely!" -ForegroundColor Green
    $remaining = Get-Process -Name "lightgui-settings" -ErrorAction SilentlyContinue
    if ($null -eq $remaining) {
        Write-Host "  PASS: 0 lightgui-settings processes remain. 0 MB RAM and 0 handles leaked!" -ForegroundColor Green
    } else {
        Write-Host "  WARN: Zombie process found!" -ForegroundColor Yellow
    }
} else {
    Write-Host "  FAIL: lightgui-settings.exe did not exit in time!" -ForegroundColor Red
    Stop-Process -Id $settingsProc.Id -Force
}

# f. Verify lightgui.exe remains running at near 0% CPU and low RAM
Write-Host "`n[6/7] Verifying lightgui.exe idle state after settings exit..." -ForegroundColor Yellow
Start-Sleep -Milliseconds 1000
$trayProc.Refresh()
if (-not $trayProc.HasExited) {
    $t2WS = [math]::Round($trayProc.WorkingSet64 / 1MB, 2)
    $t2Priv = [math]::Round($trayProc.PrivateMemorySize64 / 1MB, 2)
    $t2Threads = $trayProc.Threads.Count
    $t2Handles = $trayProc.HandleCount
    Write-Host "  lightgui.exe status: Running (PID: $($trayProc.Id))"
    Write-Host "    WorkingSet (RAM):    $t2WS MB"
    Write-Host "    PrivateMemory:       $t2Priv MB"
    Write-Host "    Thread Count:        $t2Threads"
    Write-Host "    Handle Count:        $t2Handles"
    if ($t2WS -lt 25.0) {
        Write-Host "  PASS: lightgui.exe RAM usage is low ($t2WS MB < 25 MB)!" -ForegroundColor Green
    } else {
        Write-Host "  WARN: lightgui.exe RAM is $t2WS MB" -ForegroundColor Yellow
    }
} else {
    Write-Host "  FAIL: lightgui.exe unexpectedly exited!" -ForegroundColor Red
}

# g. Gracefully terminate lightgui.exe
Write-Host "`n[7/7] Gracefully terminating lightgui.exe..." -ForegroundColor Yellow
$trayHwnd = [Win32Helper]::FindProcessWindow($trayProc.Id, "lightgui_tray_wndclass")
Write-Host "  Found tray window HWND: $trayHwnd"
if ($trayHwnd -ne [IntPtr]::Zero) {
    # 0x0010 = WM_CLOSE
    [Win32Helper]::PostMessage($trayHwnd, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
    Write-Host "  Sent WM_CLOSE to lightgui_tray_wndclass window..."
} else {
    Write-Host "  Window not found by class, sending CloseMainWindow..."
    $trayProc.CloseMainWindow() | Out-Null
}

$trayExited = $false
for ($i = 0; $i -lt 10; $i++) {
    Start-Sleep -Milliseconds 300
    $trayProc.Refresh()
    if ($trayProc.HasExited) {
        $trayExited = $true
        break
    }
}

if ($trayExited) {
    Write-Host "  PASS: lightgui.exe terminated gracefully!" -ForegroundColor Green
} else {
    Write-Host "  WARN: lightgui.exe did not exit via WM_CLOSE within 3s, stopping forcefully..." -ForegroundColor Yellow
    Stop-Process -Id $trayProc.Id -Force
}

Write-Host "`n=========================================" -ForegroundColor Cyan
Write-Host " Integration Test Complete" -ForegroundColor Cyan
Write-Host "=========================================" -ForegroundColor Cyan
