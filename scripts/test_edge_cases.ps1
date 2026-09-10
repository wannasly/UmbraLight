Write-Host "=========================================" -ForegroundColor Cyan
Write-Host "       LightGUI Edge Cases Test          " -ForegroundColor Cyan
Write-Host "=========================================" -ForegroundColor Cyan

# Cleanup
Get-Process -Name "lightgui", "lightgui-settings" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 500

$trayExe = (Resolve-Path "target/release/lightgui.exe").Path
$settingsExe = (Resolve-Path "target/release/lightgui-settings.exe").Path

# Win32 definitions
$win32Def = @"
using System;
using System.Runtime.InteropServices;
using System.Text;

public class WinHelperEdge {
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

# ----------------------------------------------------------------------------
# 1. Standalone Settings Fallback Mode (Daemon NOT Running)
# ----------------------------------------------------------------------------
Write-Host "`n[Edge Case 1] Launching lightgui-settings.exe in Standalone Fallback Mode (No Daemon)..." -ForegroundColor Yellow

$settingsProc = Start-Process -FilePath $settingsExe -PassThru
Start-Sleep -Milliseconds 1000

$settingsHwnd = [WinHelperEdge]::FindProcessWindow($settingsProc.Id, "lightgui_settings_wndclass")
if ($settingsHwnd -ne [IntPtr]::Zero) {
    Write-Host "  PASS: Settings window created successfully without daemon! HWND: $settingsHwnd" -ForegroundColor Green
} else {
    Write-Host "  FAIL: Settings window was NOT found!" -ForegroundColor Red
}

$settingsProc.Refresh()
$ws = [math]::Round($settingsProc.WorkingSet64 / 1MB, 2)
Write-Host "  Standalone Settings RAM: $ws MB | Threads: $($settingsProc.Threads.Count) | Handles: $($settingsProc.HandleCount)"

# Close settings
[WinHelperEdge]::PostMessage($settingsHwnd, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
Start-Sleep -Milliseconds 500
$settingsProc.Refresh()
if ($settingsProc.HasExited) {
    Write-Host "  PASS: Standalone settings exited cleanly on WM_CLOSE!" -ForegroundColor Green
} else {
    Write-Host "  FAIL: Standalone settings did not exit!" -ForegroundColor Red
    Stop-Process -Id $settingsProc.Id -Force
}

# ----------------------------------------------------------------------------
# 2. Single-Instance Mutex for lightgui.exe
# ----------------------------------------------------------------------------
Write-Host "`n[Edge Case 2] Testing Single-Instance Mutex for lightgui.exe..." -ForegroundColor Yellow

$inst1 = Start-Process -FilePath $trayExe -PassThru
Start-Sleep -Milliseconds 1000
Write-Host "  Instance 1 started (PID: $($inst1.Id))"

# Try starting Instance 2
$inst2 = Start-Process -FilePath $trayExe -PassThru
Write-Host "  Instance 2 launched (PID: $($inst2.Id))"

Start-Sleep -Milliseconds 1500
$inst2.Refresh()
if ($inst2.HasExited) {
    Write-Host "  PASS: Instance 2 detected existing mutex and exited immediately!" -ForegroundColor Green
} else {
    Write-Host "  FAIL: Instance 2 is still running (Single-Instance Mutex FAILED)!" -ForegroundColor Red
    Stop-Process -Id $inst2.Id -Force
}

# Also note: Instance 2 spawns settings app when duplicate detected
$spawnedSettings = Get-Process -Name "lightgui-settings" -ErrorAction SilentlyContinue
if ($spawnedSettings) {
    Write-Host "  PASS: Instance 2 opened settings window for the user before exiting!" -ForegroundColor Green
    $spawnedSettings | Stop-Process -Force
}

# Clean up Instance 1
$trayHwnd = [WinHelperEdge]::FindProcessWindow($inst1.Id, "lightgui_tray_wndclass")
if ($trayHwnd -ne [IntPtr]::Zero) {
    [WinHelperEdge]::PostMessage($trayHwnd, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
}
Start-Sleep -Milliseconds 500
$inst1.Refresh()
if (-not $inst1.HasExited) { Stop-Process -Id $inst1.Id -Force }

# ----------------------------------------------------------------------------
# 3. Single-Instance Check for lightgui-settings.exe
# ----------------------------------------------------------------------------
Write-Host "`n[Edge Case 3] Testing Single-Instance Check for lightgui-settings.exe..." -ForegroundColor Yellow

$sInst1 = Start-Process -FilePath $settingsExe -PassThru
Start-Sleep -Milliseconds 1000
Write-Host "  Settings Instance 1 started (PID: $($sInst1.Id))"

$sInst2 = Start-Process -FilePath $settingsExe -PassThru
Write-Host "  Settings Instance 2 launched (PID: $($sInst2.Id))"

Start-Sleep -Milliseconds 1500
$sInst2.Refresh()
if ($sInst2.HasExited) {
    Write-Host "  PASS: Settings Instance 2 detected existing window and exited immediately!" -ForegroundColor Green
} else {
    Write-Host "  FAIL: Settings Instance 2 is still running!" -ForegroundColor Red
    Stop-Process -Id $sInst2.Id -Force
}

# Clean up Instance 1
$sHwnd = [WinHelperEdge]::FindProcessWindow($sInst1.Id, "lightgui_settings_wndclass")
if ($sHwnd -ne [IntPtr]::Zero) {
    [WinHelperEdge]::PostMessage($sHwnd, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
}
Start-Sleep -Milliseconds 500
$sInst1.Refresh()
if (-not $sInst1.HasExited) { Stop-Process -Id $sInst1.Id -Force }

Write-Host "`n=========================================" -ForegroundColor Cyan
Write-Host " Edge Case Testing Complete" -ForegroundColor Cyan
Write-Host "=========================================" -ForegroundColor Cyan
