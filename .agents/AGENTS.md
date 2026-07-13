# Agent Instructions — ACTLog

Windows time tracker. Chrome extension is the PRIMARY UI. Tray app is background collector.

## Rules

### Do
- `windows-rs` for Win32, `rusqlite bundled`, `tray-icon`+`muda`, `tiny_http`, `egui`/`eframe` (ui-bin only)
- Sync crates (no tokio unless justified)
- `anyhow`/`thiserror` — no unwrap/expect in production
- `cargo clippy` + `cargo fmt` before every commit
- REST API: GET endpoints only — extension is read-only
- Resolve all file paths relative to `current_exe()` (portable) or `%APPDATA%` (installed), never CWD
- Handle `GetForegroundWindow` NULL, access denied, and `ApplicationFrameHost.exe` wrapping
- Use `VACUUM INTO` for DB backups, not `std::fs::copy`
- Open separate SQLite connections for writer vs readers — no `Mutex<Connection>`
- Use `chrome.alarms` API in extension background scripts (not `setInterval`)
- Add `Access-Control-Allow-Origin` header to all server responses

### Do Not
- Full Tauri, WebSocket, registry writes, tokio for core/UI
- Browser tab/URL data collection in the extension
- POST/PUT endpoints for extension activity data
- `unsafe` unless windows-rs requires it
- Commit `.env`/credentials/binaries
- Hardcode absolute paths
- `std::fs::copy` a live WAL-mode SQLite database
- Use `std::env::current_dir()` for storage paths
- Add deps without checking Cargo.toml first

## Tech Stack
- **Win32**: `windows-rs`
- **DB**: `rusqlite` (bundled, WAL)
- **Tray**: `tray-icon` + `muda`
- **UI**: `egui`/`eframe` in separate process (`actlog-ui.exe`), spawned on demand from tray
- **Server**: `tiny_http` REST on `127.0.0.1:5566` with thread pool
- **Event loop (ui-bin)**: `tao` via `eframe`
- **Single instance**: Win32 named mutex
- No tokio, no Tauri, no WebSocket, no registry writes

## Core Concepts (quick ref)
See `.agents/features.md#core-concepts` for full details.

- **Foreground tracking** (`core/tracker.rs`): ~3s poll via `GetForegroundWindow` → `GetWindowThreadProcessId` → `QueryFullProcessImageNameW`. Handles `ApplicationFrameHost.exe` (child window enum), access denied (fallback string), lock screen (`LogonUI.exe` → AFK).
- **AFK** (`core/afk.rs`): `GetLastInputInfo`, 180s idle → `source=afk`. Caps delta on sleep resume.
- **Heartbeat-merge** (`core/merge.rs`): If latest row matches app+title and gap ≤ 6-9s, extend `end_utc`. Else insert new UUID7 row.
- **Storage** (`core/db.rs`): WAL mode. Dedicated writer connection (tracker thread). Separate reader connections (server, ui-bin). Schema: `sessions(app, title, start_utc, end_utc, source, device_id)`.
- **Crash recovery**: On startup, cap open-ended last session and insert `source=crash_gap` row.
- **Multi-device**: `device_id` column from day one, generated once per machine.
- **Threading**: tray main, tracker daemon, AFK daemon, server daemon. Shutdown via `Arc<AtomicBool>`.
- **Data flow**: Tray collects → REST (GET) → Extension polls. Extension sends no data.

## Distribution
See `.agents/features.md#distribution` for full details.

- **Portable**: All files alongside exe. Backups in `current_exe().parent()/backups/`.
- **Installed**: Backups in `%APPDATA%/actlog/backups/`.
- **Auto-start**: Start Menu shortcut via `mslnk`, not registry.
- Always resolve paths via `current_exe()`, never CWD.

## Backup & Data
See `.agents/features.md#backup-and-data` for full details.

- **Auto-backup**: Both JSON export (`.json.gz`) and DB snapshot via `VACUUM INTO`.
- **Export/import**: JSON array of sessions. Idempotent (dedup by `start_utc` + `source`).
- **Data loss guarantee**: 3s poll, WAL mode, crash recovery caps open sessions. Max mis-attribution = poll interval.

## Single Instance
See `.agents/features.md#single-instance` for full details.

Win32 named mutex (`CreateMutexW`). Second instance exits immediately. Port 5566 is secondary guard.

## Extension
See `.agents/features.md#extension` for full details.

- **Background polling**: `chrome.alarms` every 5-10min (MV3 kills `setInterval` in service workers).
- **Live mode**: 3s polling when extension UI is open (runs in page context).
- **CORS**: Every response includes `Access-Control-Allow-Origin: chrome-extension://<id>`.
