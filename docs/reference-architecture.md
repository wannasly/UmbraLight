# Umbra (`myguiproxy`) Reference Architecture Analysis

## 1. Executive Summary & Overview

Umbra (`myguiproxy`) is a Windows GUI client for the [sing-box](https://sing-box.sagernet.org/) proxy platform. It is implemented as a hybrid desktop application utilizing **Tauri v2** (Rust backend) combined with a **React 19 / Vite / Tailwind CSS** frontend rendered inside Microsoft Edge WebView2.

While functional and feature-rich, Umbra's reliance on WebView2 and a full web stack introduces substantial memory footprint (100–220 MB RAM at idle across 5+ processes), non-trivial startup latency, and IPC serialization overhead. This document provides an exhaustive, code-level analysis of Umbra's backend mechanics, network integration, process supervision, and architectural limitations to serve as the baseline specification for `lightgui`.

---

## 2. Sing-box Process Lifecycle & Supervision

The sing-box core lifecycle is managed in `src-tauri/src/singbox/process.rs` and `src-tauri/src/singbox/job.rs`.

### 2.1 Directory Layout and Paths
Umbra resolves sing-box files inside the application data directory (`%APPDATA%\com.umbra.proxy\`):
- **Core Binary**: `bin\sing-box.exe` (validated via `singbox::version::core_path`)
- **Working Directory**: `work\` (contains `cache.db` and runtime files)
- **Generated Configuration**: `config\generated.json`

### 2.2 Windows Job Object Kernel Binding (`job.rs`)
Child process termination on Windows is notoriously fragile:
- Standard `tokio::process::Command::kill_on_drop(true)` only runs during normal Rust unwinding.
- If the parent is terminated forcefully (Task Manager "End Process", system shutdown, sudden crash, or debugging break), Rust destructors do not execute. The orphaned `sing-box.exe` process continues running in the background, keeping the Wintun adapter, Windows Filtering Platform (WFP) rules, or WinINET system proxy locks active.

To guarantee zero orphaned core processes under all conditions, Umbra binds the spawned child to an unnamed **Windows Job Object**:
1. **Creation**: `CreateJobObjectW(None, PCWSTR::null())`
2. **Configuration**: `SetInformationJobObject` with `JobObjectExtendedLimitInformation` and limit flag:
   ```c
   info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
   ```
3. **Assignment**: Process handle assigned via `AssignProcessToJobObject(job_handle, child_process_handle)` immediately following `cmd.spawn()`.
4. **Kernel Guarantee**: The Job handle is retained globally in a `static OnceLock<Option<Job>>` for the lifetime of the application. When the parent process terminates for any reason, Windows kernel closes the parent's job handle, automatically terminating all processes assigned to the job.

### 2.3 Configuration Generation (`config.rs`)
Configuration generation is pure in-memory JSON AST construction (`singbox::config::generate`):
- **Reserved Outbound Tags**: `["proxy", "auto", "direct"]`. Server tags are sanitized and deduplicated against this list.
- **Outbounds Structure**:
  - `selector` outbound tagged `"proxy"` pointing to `auto` and all parsed server tags.
  - `urltest` outbound tagged `"auto"` probing servers via `settings.ping_url` at 3-minute intervals with a 50ms tolerance.
  - Individual server outbounds for each node.
  - Direct outbound tagged `"direct"`.
- **DNS Section**:
  - `dns-remote`: Type `https`, server `1.1.1.1`, detour `proxy`.
  - `dns-local`: Type `local`.
  - Independent cache enabled; strategy configured via `settings.ip_strategy`.
- **Inbounds**:
  - **System Proxy Mode**: Inbound type `mixed`, tag `mixed-in`, listen `127.0.0.1:<mixed_port>`.
  - **TUN Mode**: Inbound type `tun`, tag `tun-in`, interface name `umbra-tun`, IP addresses `172.19.0.1/30` and `fdfe:dcba:9876::1/126`, MTU `settings.tun_mtu` (default 9000), `auto_route: true`, `strict_route: settings.tun_strict_route`, `stack: settings.tun_stack` (`system`, `gvisor`, or `mixed`).

### 2.4 Preflight Config Check (`sing-box check`)
Before spawning the core, Umbra executes a synchronous validation pass:
```powershell
sing-box.exe check -c <config_path>
```
Executed with `CREATE_NO_WINDOW (0x08000000)` and stdin set to null. If check fails:
- Stderr is captured, and up to 4000 characters are streamed into the log buffer.
- `short_reason()` extracts the first non-empty line (capped at 160 characters) to show a concise UI toast, preventing UI flooding.

### 2.5 Process Spawning & Startup Confirmation
Spawning uses `tokio::process::Command`:
- Arguments: `run -c generated.json --disable-color`
- Standard I/O: stdout and stderr piped to asynchronous reader tasks.
- Windows creation flags: `CREATE_NO_WINDOW`.
- **Readiness Verification (`confirm_started`)**:
  - Polls `child.try_wait()` every 100ms for up to `STARTUP_CONFIRM = 1500ms`.
  - Any fatal immediate startup errors (port conflicts, missing `wintun.dll`, network interface initialization failure) cause the process to exit within this 1.5s window. If it exits, the error code is captured and reported immediately.

### 2.6 Graceful Teardown vs. Hard Kill
Shutdown occurs in `singbox::process::stop()`:
1. If WinINET proxy was enabled (`proxy_owned == true`), `system_proxy::disable_proxy()` is executed first.
2. The supervisor notification flag (`shared.stopping`) is marked `true`, and `stop_notify.notify_one()` is triggered.
3. The child process receives `child.start_kill()` (SIGTERM equivalent).
4. A grace timeout of `KILL_GRACE = 3s` is awaited.
5. If the child does not exit within 3 seconds, `child.kill()` (SIGKILL / `TerminateProcess`) is called.
6. The supervisor task join handle is awaited up to `STOP_JOIN_TIMEOUT = 12s`.

### 2.7 Crash Supervision & Backoff
If sing-box exits unexpectedly while not in a stopping state:
- Clean up system proxy immediately if owned.
- If uptime was $\ge 60\text{s}$ (`HEALTHY_RESET`), reset restart attempt counter to 0.
- Increment attempt counter: maximum `MAX_RESTART_ATTEMPTS = 3`.
- Backoff sleep duration:
  - Attempt 1: 1s
  - Attempt 2: 3s
  - Attempt 3: 9s
- Emit event `core://crashed` (`CrashInfo { code, will_restart, attempt }`).
- If attempt count exceeds 3, transition status to `Disconnected` and log failure.

---

## 3. Clash API Integration

Sing-box provides an embedded Clash-compatible REST/WebSocket API (`src-tauri/src/singbox/clash_api.rs`).

### 3.1 Controller Setup
- Endpoint: `127.0.0.1:<clash_port>`
- Authentication: Bearer token secret (`clash_secret`) generated randomly per session.
- Client requests bypass system proxy via `reqwest::ClientBuilder::no_proxy()`.

### 3.2 Dynamic Server Switching
When switching active servers in GUI:
- Target outbound tag is resolved from server ID.
- HTTP request sent:
  ```http
  PUT /proxies/proxy HTTP/1.1
  Authorization: Bearer <clash_secret>
  Content-Type: application/json

  {"name": "<target_tag>"}
  ```
- Instantaneous switch without interrupting active listening sockets or rewriting configuration.

### 3.3 Delay Testing
- Latency probing endpoint:
  ```http
  GET /proxies/<tag>/delay?timeout=5000&url=<ping_url> HTTP/1.1
  Authorization: Bearer <clash_secret>
  ```
- Returns JSON payload `{"delay": <latency_ms>}`.

### 3.4 Traffic Monitoring Stream
- WebSocket connection: `ws://127.0.0.1:<clash_port>/traffic?token=<clash_secret>`
- sing-box pushes a JSON message once per second:
  ```json
  {"up": 102400, "down": 2048000}
  ```
- **Totals Accumulator & Disk Write Throttling**:
  - Frames arrive at 1 Hz. Writing to disk on every frame causes excessive NVMe/SSD wear.
  - `TotalsAccumulator` buffers bytes in memory.
  - Flushes to `profiles.json` every `TOTALS_FLUSH_INTERVAL = 15s`, or immediately upon server switch.

### 3.5 TUN Interface Traffic via `GetIfTable2` (`net/traffic.rs`)
In TUN mode, Clash API traffic counters can miss hardware-accelerated/bypass packets. Umbra falls back to direct Windows MIB interface tables:
- Invokes Windows IP Helper API `GetIfTable2(&mut table)`.
- Iterates `MIB_IF_ROW2` entries searching for `row.Alias.eq_ignore_ascii_case("umbra-tun")`.
- Reads `row.OutOctets` (upload) and `row.InOctets` (download).
- Frees table memory via `FreeMibTable(table)`.

---

## 4. Subscription & Node Parsing

Subscription mechanics reside in `src-tauri/src/subscription.rs` and `src-tauri/src/parser/`.

### 4.1 Transport & Headers
- Default User-Agent: `v2rayN/7.13 Umbra/<version>` (panels serve raw link lists to v2rayN instead of complex proprietary formats).
- Hardware ID headers for device limits (e.g. Remnawave panels):
  - `x-hwid`: Salted SHA-256 hash of machine GUID
  - `x-device-os`: `"Windows"`
  - `x-ver-os`: `"11 (26100)"`
  - `x-device-model`: E.g. `"ASUSTeK COMPUTER INC. ROG STRIX B550-F"`

### 4.2 Content Decoding Pipeline
1. UTF-8 BOM (`\u{feff}`) stripped.
2. If first line contains `://`, treated as plain URI list.
3. If not, whitespace stripped and base64 decoded (trying both standard and URL-safe base64 with auto-padding).
4. Explicit rejection of Clash YAML (`proxies:`) and sing-box JSON configurations.

### 4.3 Anti-Placeholder Guard
Remnawave/Marzban panels under device exhaustion or policy blocks return HTTP 200 with dummy nodes instead of HTTP 4xx errors:
- Node address `0.0.0.0`, `127.0.0.1`, or `::` on port `1`.
- Associated headers: `x-hwid-not-supported`, `x-hwid-max-devices-reached`.
- `drop_placeholders` identifies port 1 loopback/zero entries and maps them to actionable user errors (`AppError::DeviceLimit` or `AppError::HwidRequired`).

### 4.4 Metadata Extraction
- `subscription-userinfo`: Parses `upload=X; download=Y; total=Z; expire=T` into `SubscriptionQuota`.
- `profile-title`: Extracts human-readable profile title; supports `base64:<text>` encoding.
- `profile-update-interval`: Auto-update interval in hours.
- `support-url`, `profile-web-page-url`.

---

## 5. Protocol Parsers

Parsers reside in `src-tauri/src/parser/vless.rs` and `src-tauri/src/parser/hysteria2.rs`.

### 5.1 VLESS Parser (`vless://`)
- **Query Parsing**: Custom parser preserving `+` characters in base64 parameters (such as `pbk`).
- **Security Modes**:
  - `reality`: Requires `pbk` (public key). Optional `sid` (short ID), `fp` (uTLS fingerprint, default `chrome`), `sni`. ALPN is cleared.
  - `tls`: Supports `sni`, `alpn`, `allowInsecure`.
  - `none`.
- **Transports**:
  - `tcp`: Supports flow `xtls-rprx-vision`.
  - `ws`: `path` (cleans up `?ed=` early-data parameters), `host` header.
  - `grpc`: `serviceName`.
  - `httpupgrade`: `path`, `host`.
  - Rejection: Explicitly rejects `xhttp` / `splithttp` (Xray only, unsupported in sing-box) and `headerType=http`.
- **Packet Encoding**: Always configured to `xudp`.

### 5.2 Hysteria2 Parser (`hysteria2://` / `hy2://`)
- **Multi-Port Rejection**: Immediately rejects port ranges (`:443-445`) or comma lists (`:443,444`).
- **Authentication**: Extracts password from URI userinfo.
- **Obfuscation**: Parses `obfs=salamander` and `obfs-password=<pwd>`.
- **TLS/SNI**: Fallback from `sni` parameter to domain server hostname.

---

## 6. Routing Engine & Windows Integration

### 6.1 Routing Model (`models.rs`, `config.rs`)
- **Rule Structure**:
  - Process rules: `process_name` (e.g. `Discord.exe`) or `process_path` (`C:\...`).
  - Domain rules: `domain_suffix` (e.g. `.google.com`), `domain` (exact), `domain_keyword`, `domain_regex`.
  - IP CIDR: `ip_cidr` (e.g. `192.168.0.0/16`).
  - Actions: `proxy`, `direct`, `reject` (block).
- **Built-in Presets**:
  - **Discord Voice UDP Direct**: Bypasses UDP traffic for `["Discord.exe", "DiscordCanary.exe", "DiscordPTB.exe"]` to prevent TCP head-of-line blocking inside XUDP.
  - **Private IP Bypass**: `ip_is_private: true -> direct`.
  - **RU Rule-Sets**: Remote binary `.srs` sets for `geosite-category-ru.srs` and `geoip-ru.srs` with 7-day auto-update.
  - **Default Fallback**: Target `route_default` (`proxy` or `direct`).

### 6.2 Process Enumeration (`commands/processes.rs`)
To populate the process picker:
1. `EnumWindows`: Iterates top-level windows; filters by `IsWindowVisible`; extracts titles via `GetWindowTextW`; maps `pid -> title`.
2. `CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)`: Enumerates all processes.
3. Filters system noise: `svchost.exe`, `dwm.exe`, `csrss.exe`, `lsass.exe`, `services.exe`, etc.
4. `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` + `QueryFullProcessImageNameW`: Resolves absolute disk path for each process.
5. Sorts processes with visible windows first, followed by alphabetical process name.

### 6.3 System Proxy Mode (`proxy/system_proxy.rs`)
Modifies WinINET proxy settings in Windows Registry:
- Key: `HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings`
- Values:
  - `ProxyEnable` = `1`
  - `ProxyServer` = `"127.0.0.1:<port>"`
  - `ProxyOverride` = `"localhost;127.*;10.*;172.16.*;...;172.31.*;192.168.*;<local>"`
- **Notification**: Calls Win32 `InternetSetOptionW(None, INTERNET_OPTION_SETTINGS_CHANGED, None, 0)` and `INTERNET_OPTION_REFRESH`.
- **Crash Safety**: Reads and stores existing registry values in `settings.json` under `proxy_backup` *before* touching registry. `startup_recovery()` restores values if a previous session died while owning proxy.

### 6.4 TUN Mode & Elevation (`proxy/elevation.rs`)
- **Elevation Check**: `OpenProcessToken` + `GetTokenInformation(TokenElevation)`.
- **Elevation Relaunch**: `ShellExecuteW(None, "runas", current_exe, "--resume-tun", ..., SW_SHOWNORMAL)`.
- **Wintun Driver Lifecycle**: `wintun.dll` creates virtual adapter via `SwDeviceCreate`. No `SwDeviceSetLifetime` is set, so adapter is bound to process handle; Windows kernel destroys adapter when process closes.
- **WFP Filter Cleanup**: `strict_route` firewall rules use `FWPM_SESSION_FLAG_DYNAMIC`, which the Base Filtering Engine (BFE) automatically purges when the session handle closes.

---

## 7. Storage, Migrations & HWID

### 7.1 Atomic Persistence (`storage.rs`)
- `write_json<T>` writes to `<filename>.tmp` followed by `fs::rename()`.
- Catches Windows file-locking collisions with automatic removal and retry.

### 7.2 Migration System (`storage_migration.rs`)
- V1 Schema: Flat server structures with protocol string fields.
- V2 Schema: Polymorphic enum `ProxyKind::Vless(VlessNode)` / `ProxyKind::Hysteria2(Hysteria2Node)`.
- Migrations preserve server IDs, counters, and subscriptions seamlessly.

### 7.3 HWID Calculation (`hwid.rs`)
- Reads `HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid` using 64-bit registry view (`KEY_WOW64_64KEY`).
- Computes `SHA-256("umbra-hwid-v1" + machine_guid)`, truncated to 32 hex characters.
- Extracts manufacturer & product model from `SYSTEM\HardwareConfig\Current`.

---

## 8. The Architectural Baggage of Umbra

| Overhead Dimension | Cause in Umbra | Consequence |
| :--- | :--- | :--- |
| **RAM Consumption** | Microsoft Edge WebView2 multi-process architecture (Browser, GPU, Renderer, Crashpad) + React DOM runtime. | **100 MB – 220 MB RAM** at idle. |
| **Process Count** | WebView2 spawns 4 to 6 auxiliary `msedgewebview2.exe` processes per window. | Clutters process tree; high handle count (>800). |
| **Background Footprint** | Closing the window hides it (`win.hide()`) rather than unloading WebView2. | WebView2 continues holding 100+ MB RAM in tray mode. |
| **IPC Serialization** | Traffic stats (1/s) and log batches (4/s) serialize Rust structs to JSON strings, routed through Tauri IPC to JavaScript. | Unnecessary CPU cycles and garbage collector churn. |
| **Cold Start Latency** | Node.js modules, webview runtime initialization, DOM hydration. | 800ms – 2500ms launch time. |
| **Dependency Bloat** | `node_modules` (React 19, Vite, Tailwind, Lucide, Radix UI, TypeScript). | Complex build pipeline, large release binaries. |

These structural overheads establish the imperative for **Lightgui**: a native Win32 Rust architecture that reduces memory consumption by 90%, drops WebView2 entirely, and provides instantaneous response.
