$ErrorActionPreference = 'Stop'

$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$runtime = Join-Path $root 'target\runtime-smoke'
New-Item -ItemType Directory -Force -Path $runtime | Out-Null
$oldAppData = $env:APPDATA
$env:APPDATA = $runtime
$tray = $null
$settings = $null

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class LightGuiSmokeWin32 {
    public delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr param);
    [DllImport("user32.dll")]
    public static extern bool EnumWindows(EnumWindowsProc callback, IntPtr param);
    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)]
    public static extern int GetClassNameW(IntPtr hwnd, StringBuilder name, int max);
    public static IntPtr FindProcessWindow(uint processId, string className) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((hwnd, param) => {
            uint candidate;
            GetWindowThreadProcessId(hwnd, out candidate);
            if (candidate == processId) {
                var name = new StringBuilder(256);
                GetClassNameW(hwnd, name, 256);
                if (name.ToString() == className) { found = hwnd; return false; }
            }
            return true;
        }, IntPtr.Zero);
        return found;
    }
    [DllImport("user32.dll")]
    public static extern bool PostMessageW(IntPtr hwnd, uint message, IntPtr w, IntPtr l);
}
'@

function Send-LightGuiRequest($request) {
    $pipe = [System.IO.Pipes.NamedPipeClientStream]::new('.', 'lightgui_ipc', [System.IO.Pipes.PipeDirection]::InOut)
    try {
        $pipe.Connect(3000)
        $json = $request | ConvertTo-Json -Compress -Depth 30
        $bytes = [Text.Encoding]::UTF8.GetBytes($json)
        $length = [BitConverter]::GetBytes([int]$bytes.Length)
        $pipe.Write($length, 0, 4)
        $pipe.Write($bytes, 0, $bytes.Length)
        $pipe.Flush()
        $header = New-Object byte[] 4
        $read = 0
        while ($read -lt 4) {
            $count = $pipe.Read($header, $read, 4 - $read)
            if ($count -eq 0) { throw 'IPC closed before response length' }
            $read += $count
        }
        $size = [BitConverter]::ToInt32($header, 0)
        if ($size -lt 1 -or $size -gt 10485760) { throw "Invalid IPC response size: $size" }
        $body = New-Object byte[] $size
        $read = 0
        while ($read -lt $size) {
            $count = $pipe.Read($body, $read, $size - $read)
            if ($count -eq 0) { throw 'IPC closed before response body' }
            $read += $count
        }
        return [Text.Encoding]::UTF8.GetString($body) | ConvertFrom-Json
    } finally {
        $pipe.Dispose()
    }
}

try {
    if (Get-Process lightgui -ErrorAction SilentlyContinue) {
        throw 'Another LightGUI tray is running; close it before this single-instance smoke test.'
    }
    $tray = Start-Process -FilePath (Join-Path $root 'target\release\lightgui.exe') -PassThru -WindowStyle Hidden
    Start-Sleep -Milliseconds 700
    $status = Send-LightGuiRequest @{ type = 'getStatus' }
    if ($status.payload.state.status -ne 'disconnected') { throw 'Tray did not start disconnected' }

    $profiles = @{
        version = 2
        manual = @(@{
            id = 'smoke'; name = 'Smoke'; server = '127.0.0.1'; port = 443
            raw = 'vless://test@127.0.0.1:443?security=none#Smoke'
            protocol = 'vless'; uuid = '11111111-2222-3333-4444-555555555555'; security = 'none'
        })
        subscriptions = @()
    }
    $save = Send-LightGuiRequest @{ type = 'saveProfiles'; payload = @{ profiles = $profiles } }
    if ($save.type -ne 'success') { throw "SaveProfiles failed: $($save | ConvertTo-Json -Compress)" }
    $switch = Send-LightGuiRequest @{ type = 'switchServer'; payload = @{ serverId = 'smoke' } }
    if ($switch.type -ne 'success') { throw "SwitchServer failed: $($switch | ConvertTo-Json -Compress)" }
    $status = Send-LightGuiRequest @{ type = 'getStatus' }
    if ($status.payload.state.serverId -ne 'smoke') { throw 'Server switch was not reflected in tray state' }

    $storedSettings = Get-Content -LiteralPath (Join-Path $runtime 'lightgui\settings.json') -Raw | ConvertFrom-Json
    $storedSettings.mixedPort = 28880
    $saveSettings = Send-LightGuiRequest @{ type = 'saveSettings'; payload = @{ settings = $storedSettings } }
    if ($saveSettings.type -ne 'success') { throw "SaveSettings failed: $($saveSettings | ConvertTo-Json -Compress)" }
    $persisted = Get-Content -LiteralPath (Join-Path $runtime 'lightgui\settings.json') -Raw | ConvertFrom-Json
    if ($persisted.mixedPort -ne 28880) { throw 'Settings were not persisted through tray IPC' }

    $settings = Start-Process -FilePath (Join-Path $root 'target\release\lightgui-settings.exe') -PassThru
    Start-Sleep -Milliseconds 700
    $settingsWindow = [LightGuiSmokeWin32]::FindProcessWindow($settings.Id, 'lightgui_settings_wndclass')
    if ($settingsWindow -eq [IntPtr]::Zero) { throw 'Settings window was not created' }

    $tray.Refresh()
    $cpuBefore = $tray.TotalProcessorTime.TotalMilliseconds
    Start-Sleep -Seconds 2
    $tray.Refresh()
    $idleCpu = $tray.TotalProcessorTime.TotalMilliseconds - $cpuBefore
    Write-Output "IPC and Settings: PASS"
    Write-Output "Tray idle: CPU $idleCpu ms / 2 s, RAM $([math]::Round($tray.WorkingSet64 / 1MB, 1)) MB, threads $($tray.Threads.Count), handles $($tray.HandleCount)"

    [LightGuiSmokeWin32]::PostMessageW($settingsWindow, 0x10, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
    if (-not $settings.WaitForExit(3000)) { throw 'Settings failed to exit' }
    $trayWindow = [LightGuiSmokeWin32]::FindProcessWindow($tray.Id, 'lightgui_tray_wndclass')
    if ($trayWindow -eq [IntPtr]::Zero) { throw 'Tray window was not found' }
    [LightGuiSmokeWin32]::PostMessageW($trayWindow, 0x10, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
    if (-not $tray.WaitForExit(3000)) { throw 'Tray failed to exit' }
    Write-Output 'Settings close and tray Exit: PASS'
} finally {
    if ($settings -and -not $settings.HasExited) { $settings.Kill() }
    if ($tray -and -not $tray.HasExited) { $tray.Kill() }
    $env:APPDATA = $oldAppData
}
