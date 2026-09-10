# Lightgui Performance Budgets & Measurement Methodology

## 1. Performance Goals & Motivation

Umbra (`myguiproxy`) consumes significant system resources primarily due to Microsoft Edge WebView2, Chromium multi-process architecture, and the React 19 / Vite frontend runtime. A proxy client is a background utility that often runs 24/7; therefore, its steady-state resource footprint must be negligible.

**Lightgui** enforces strict resource budgets to ensure near-zero impact on host battery life, memory availability, and CPU scheduling.

---

## 2. Umbra vs. Lightgui Budget Comparison

| Metric | Umbra (Measured) | Lightgui Budget (Hard Cap) | Improvement |
| :--- | :--- | :--- | :--- |
| **Idle RAM (WorkingSet64)** | 140 MB – 220 MB | **< 15 MB** | **> 90% reduction** |
| **Idle RAM (Private Bytes)** | 110 MB – 180 MB | **< 10 MB** | **> 92% reduction** |
| **Settings Open (WorkingSet64)** | 180 MB – 260 MB | **< 35 MB** | **> 85% reduction** |
| **After Settings Closed** | 140 MB – 220 MB (WebView2 persists) | **< 15 MB** (Process terminates: 0 MB) | **Complete reclamation** |
| **Idle CPU Utilization** | 0.2% – 1.5% (JS timers, animations) | **~0.0%** (Pure `GetMessageW` event sleep) | **Zero scheduler load** |
| **Active OS Threads** | 24 – 38 threads | **$\le$ 3 threads** (Main Win32, IPC server, async runtime) | **> 88% reduction** |
| **Kernel Handle Count** | 800 – 1400 handles | **< 150 handles** | **> 85% reduction** |
| **OS Processes** | 5 – 7 processes (Tauri + WebView2 workers) | **1 process** (`lightgui.exe`; +1 on-demand when settings open) | **Eliminates process clutter** |
| **Cold Start to Ready** | 1200 ms – 2800 ms | **< 150 ms** | **15x faster** |

---

## 3. Operational State Budgets

### State 1: Tray Idle (Disconnected)
- **Processes**: 1 (`lightgui.exe`)
- **WorkingSet64**: $\le 12\text{ MB}$
- **PrivateMemorySize64**: $\le 8\text{ MB}$
- **CPU %**: `0.0%` (Thread blocked in `GetMessageW`)
- **Threads**: $\le 3$ (Win32 GUI thread, Named Pipe listener thread, Core worker thread)
- **Handles**: $< 120$

### State 2: Tray Active (Connected & Proxying)
- **Processes**: 2 (`lightgui.exe` + `sing-box.exe`)
- **`lightgui.exe` WorkingSet64**: $\le 15\text{ MB}$
- **`lightgui.exe` CPU %**: $< 0.1\%$ (Traffic frame parsing at 1 Hz, IP Helper queries)
- **Threads**: $\le 4$
- **Handles**: $< 150$

### State 3: Settings Dialog Open
- **Processes**: 2–3 (`lightgui.exe` + `lightgui-settings.exe` [+ `sing-box.exe` if connected])
- **`lightgui-settings.exe` WorkingSet64**: $\le 20\text{ MB}$
- **Total Combined WorkingSet64**: $\le 35\text{ MB}$
- **Threads**: Settings UI runs on a single main thread + 1 IPC client thread.
- **CPU %**: Intermittent $< 0.5\%$ during window interaction / scrolling.

### State 4: Post-Settings Teardown (Closed)
- When the user closes the Settings window:
  - `lightgui-settings.exe` exits immediately via `ExitProcess(0)`.
  - All allocated memory, GDI handles, and threads are reclaimed by Windows kernel.
  - Combined memory returns immediately to **State 1 or 2 (< 15 MB)**.

---

## 4. Measurement Methodology

### 4.1 Monitored Parameters
Resource consumption must be measured using Windows NT native process counters:
1. **WorkingSet64**: Actual physical RAM (resident pages) mapped to the process.
2. **PrivateMemorySize64**: Memory allocated exclusively to the process that cannot be shared with other processes.
3. **ThreadCount**: Number of active OS threads scheduled for execution.
4. **HandleCount**: Number of open kernel objects (files, sockets, events, mutexes).
5. **CPU Percent**: Cumulative CPU time divided by elapsed wall clock time.

### 4.2 PowerShell Benchmark Script

The following specification script (`measure-lightgui.ps1`) executes continuous sampling and outputs statistical metrics:

```powershell
<#
.SYNOPSIS
    Monitors and benchmarks Lightgui and sing-box resource consumption.
.PARAMETER ProcessName
    Base process name (default: "lightgui")
.PARAMETER DurationSeconds
    Duration of the benchmark run (default: 30)
.PARAMETER IntervalMs
    Sampling interval in milliseconds (default: 500)
#>
param (
    [string]$ProcessName = "lightgui",
    [int]$DurationSeconds = 30,
    [int]$IntervalMs = 500
)

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host "  Lightgui Performance Benchmark" -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan

$samples = @()
$endTime = (Get-Date).AddSeconds($DurationSeconds)

while ((Get-Date) -lt $endTime) {
    $procs = Get-Process -Name $ProcessName, "$ProcessName-settings", "sing-box" -ErrorAction SilentlyContinue
    if ($procs) {
        $totalWS = ($procs | Measure-Object -Property WorkingSet64 -Sum).Sum / 1MB
        $totalPM = ($procs | Measure-Object -Property PrivateMemorySize64 -Sum).Sum / 1MB
        $totalThreads = ($procs | Measure-Object -Property Threads -Sum).Sum
        $totalHandles = ($procs | Measure-Object -Property Handles -Sum).Sum
        
        $mainProc = $procs | Where-Object { $_.ProcessName -eq $ProcessName } | Select-Object -First 1
        $mainWS = if ($mainProc) { $mainProc.WorkingSet64 / 1MB } else { 0 }
        $mainPM = if ($mainProc) { $mainProc.PrivateMemorySize64 / 1MB } else { 0 }
        $mainHandles = if ($mainProc) { $mainProc.Handles } else { 0 }
        $mainThreads = if ($mainProc) { $mainProc.Threads.Count } else { 0 }

        $samples += [PSCustomObject]@{
            Timestamp      = (Get-Date).ToString("HH:mm:ss.fff")
            MainWorkingSet = [Math]::Round($mainWS, 2)
            MainPrivateMB  = [Math]::Round($mainPM, 2)
            MainHandles    = $mainHandles
            MainThreads    = $mainThreads
            TotalWorkingSet= [Math]::Round($totalWS, 2)
            TotalPrivateMB = [Math]::Round($totalPM, 2)
            TotalHandles   = $totalHandles
        }
    }
    Start-Sleep -Milliseconds $IntervalMs
}

if ($samples.Count -eq 0) {
    Write-Warning "No target processes found matching '$ProcessName'."
    exit 1
}

$avgMainWS = ($samples | Measure-Object -Property MainWorkingSet -Average).Average
$maxMainWS = ($samples | Measure-Object -Property MainWorkingSet -Maximum).Maximum
$avgMainPM = ($samples | Measure-Object -Property MainPrivateMB -Average).Average
$avgHandles = ($samples | Measure-Object -Property MainHandles -Average).Average
$maxHandles = ($samples | Measure-Object -Property MainHandles -Maximum).Maximum
$avgThreads = ($samples | Measure-Object -Property MainThreads -Average).Average

Write-Host "`nBenchmark Results ($($samples.Count) samples over ${DurationSeconds}s):" -ForegroundColor Green
Write-Host "  Main Process WorkingSet:  Avg = $([Math]::Round($avgMainWS, 2)) MB, Max = $([Math]::Round($maxMainWS, 2)) MB (Budget: < 15.0 MB)"
Write-Host "  Main Process Private MB:  Avg = $([Math]::Round($avgMainPM, 2)) MB (Budget: < 10.0 MB)"
Write-Host "  Main Process Handles:     Avg = $([Math]::Round($avgHandles, 0)), Max = $maxHandles (Budget: < 150)"
Write-Host "  Main Process Threads:     Avg = $([Math]::Round($avgThreads, 0)) (Budget: <= 3)"

if ($avgMainWS -le 15.0 -and $maxHandles -le 150 -and $avgThreads -le 4) {
    Write-Host "`n>>> PERFORMANCE GATE: PASSED <<<" -ForegroundColor Green
    exit 0
} else {
    Write-Host "`n>>> PERFORMANCE GATE: FAILED <<<" -ForegroundColor Red
    exit 2
}
```

---

## 5. Architectural Safeguards for Performance

1. **GDI Resource Management**:
   - `lightgui-settings.exe` strictly frees custom fonts (`DeleteObject`), brush resources, and device contexts (`ReleaseDC`).
   - Ensures handle count never leaks across window repaint cycles.
2. **Lazy Computation**:
   - Process list enumeration (`CreateToolhelp32Snapshot`) and full path queries (`QueryFullProcessImageNameW`) are never invoked in `lightgui.exe`. They are executed only on the dedicated worker thread of `lightgui-settings.exe` when the process selector is clicked.
3. **Zero-Allocation Logging**:
   - Core log ring buffer (`LogBuffer`) in `lightgui-core` is fixed at a maximum capacity of 2,000 entries. New entries pop the oldest record, preventing unbounded heap expansion.
4. **Buffered Persistence**:
   - Live byte counters accumulate in memory and persist to NVMe/SSD only once every 15 seconds (`TOTALS_FLUSH_INTERVAL`), preventing I/O stalls and disk write churn.
