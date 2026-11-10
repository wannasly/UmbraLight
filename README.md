# UmbraLight

**Tray-first Windows client for sing-box, built with Rust and Win32.** The background process owns the connection. The native settings window runs in a separate process when opened and communicates with the tray through a Windows named pipe.

[Download for Windows](https://github.com/wannasly/UmbraLight/releases/latest) · [Документация на русском](README.ru.md)

## Run

Use the installer or ZIP from the v1.0.0 release on Windows 10 or 11 x64. The installer includes sing-box and Wintun. Open settings from the tray to import a subscription or server link, select a server, and configure system proxy, TUN, DNS, or routing. TUN requires administrator privileges.

Profiles and settings use `%APPDATA%/lightgui` for compatibility with earlier builds. Keep those files private because they can contain server details and subscription URLs.

## Build and test

Use Rust and Cargo with the Windows MSVC toolchain.

```powershell
cargo test --workspace --locked
cargo build --workspace --release --locked
.\target\release\UmbraLight.exe
```

Keep `UmbraLight-settings.exe` beside the tray executable. A compatible `sing-box.exe` is required for connections, and TUN also needs `wintun.dll`. See the [Russian guide](README.ru.md) for packaging and component placement. The [runtime smoke test](scripts/test_runtime_smoke.ps1) uses a separate data directory and requires a release build.

| Workspace crate | Responsibility |
| --- | --- |
| [Core](crates/lightgui-core) | Profile parsing and storage, sing-box configuration, Windows networking, and IPC |
| [Tray](crates/lightgui-tray) | Background connection lifecycle and tray menu |
| [Settings](crates/lightgui-settings) | On-demand Win32 settings window |
