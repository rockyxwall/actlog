# ACTLog — Detailed Feature Reference

This file contains in-depth explanations for design decisions. AI agents should read the relevant section when AGENTS.md points here.

---

## Core Concepts

### Foreground Tracking (`core/tracker.rs`)

- **Mechanism**: Poll foreground every ~3s via `GetForegroundWindow` → `GetWindowThreadProcessId` → `QueryFullProcessImageNameW` (windows-rs).
- **UWP/ApplicationFrameHost trap**: Modern Windows apps (Calculator, Settings, Terminal, WhatsApp) run inside `ApplicationFrameHost.exe`. When the exe resolves to `ApplicationFrameHost.exe`, enumerate child windows via `EnumChildWindows` to find the real hosted process PID and query that instead.
- **Access denied**: `QueryFullProcessImageNameW` returns `ERROR_ACCESS_DENIED` for elevated/system processes (PID 0, 4, Task Manager, admin cmd). Fall back to `"<Elevated Process>"` / `"<System>"` — never unwrap or crash.
- **Lock screen**: If foreground resolves to `LogonUI.exe` or `LockApp.exe`, treat as AFK state (`source=afk, app=locked`).
- **NULL window handle**: `GetForegroundWindow` can return NULL (no foreground window). Skip the poll cycle silently.

### AFK Detection (`core/afk.rs`)

- **Mechanism**: `GetLastInputInfo` returns tick count of last user input. If current tick - last input tick ≥ 180s, emit `source=afk, app=idle`.
- **Sleep resume**: After system sleep, `GetLastInputInfo` returns a very large delta (the sleep duration). Cap AFK detection at a sane maximum (e.g., 1 hour) to avoid logging days of idle.
- **Lock screen interaction**: When lock screen is detected by the tracker, the AFK daemon should be aware — lock screen already implies AFK, no need to double-emit.

### Heartbeat-Merge (`core/merge.rs`)

- **Algorithm**: `record(source, app, title, ts)`:
  1. Query the latest session row.
  2. If it matches app+title AND `0 ≤ ts - end_utc ≤ GAP_LIMIT_SECONDS` (6-9s), update `end_utc` to `ts`.
  3. Otherwise, insert a new row with UUID7, `start_utc=ts`, `end_utc=ts`.
- **Max data loss**: = poll interval (~3s). Between polls, app switches are credited to the previous app.
- **GAP_LIMIT rationale**: Set to ~2× poll interval (6-9s for 3s poll). This absorbs brief glitches (window flicker, process list refresh) without fragmenting sessions.

### Storage (`core/db.rs`)

- **WAL mode**: Write-Ahead Logging for concurrent reads without blocking writes.
- **Connection model**: 
  - **Writer**: Single dedicated `rusqlite::Connection` on the tracker thread. Never wrapped in `Mutex`.
  - **Readers**: Separate `rusqlite::Connection` instances for the server thread and `ui-bin` process. No `Mutex` needed — WAL allows concurrent readers with an active writer.
  - **Multi-process**: `ui-bin` opens its own connection to the same `.sqlite` file. WAL mode supports this.
- **Schema**:
  ```sql
  CREATE TABLE sessions (
      id TEXT PRIMARY KEY,       -- UUID7
      app TEXT NOT NULL,          -- executable name or fallback
      title TEXT NOT NULL,        -- window title
      start_utc INTEGER NOT NULL, -- unix ms
      end_utc INTEGER NOT NULL,   -- unix ms
      source TEXT NOT NULL,       -- 'foreground' | 'afk' | 'crash_gap'
      device_id TEXT NOT NULL     -- stable per-machine UUID
  );
  ```
- **Index**: `CREATE INDEX idx_sessions_start ON sessions(start_utc);`

### Crash Recovery

- On startup, query the last session row.
- If its `end_utc` is recent (within GAP_LIMIT) or it has no reasonable end, cap `end_utc` to its last known heartbeat value.
- Insert a `source=crash_gap` row marking the gap: `app="<System Crash>"`, `start_utc=<capped end_utc>`, `end_utc=<current time>`.
- This prevents the merge logic from extending a pre-crash session across the crash boundary.

### Multi-Device

- `device_id` column included from day one in the schema.
- Generated once per machine on first run, stored in a config file (`config.toml` next to exe or in `%APPDATA%/actlog/`).
- Enables future multi-machine merge: timelines from different devices can be reconciled by `device_id` + `start_utc`.

### Threading Model (tray-bin)

```
Main thread:   tray icon, event loop (tao)
Tracker thread: foreground polling loop (~3s), writer connection
AFK thread:     idle detection loop
Server thread:  tiny_http listener, spawns worker threads per request
```
- All threads share an `Arc<AtomicBool>` shutdown flag.
- On shutdown signal (tray menu "Quit"), all threads join cleanly.
- The writer connection is only accessed from the tracker thread — no lock needed.
- Server threads open their own reader connections from a pool or on-demand.

---

## Distribution

### Portable (zip)

- All files live alongside the exe in one folder:
  ```
  actlog/
  ├── actlog-tray.exe
  ├── actlog-ui.exe
  ├── actlog.sqlite      (data)
  ├── config.toml         (settings, device_id)
  └── backups/
      ├── sessions-2026-07-14T12-00-00.json.gz
      └── sessions-2026-07-14T12-00-00.db
  ```
- Paths resolved via `std::env::current_exe().parent().unwrap()`. Never use `std::env::current_dir()` — when launched via Start Menu shortcut, CWD is `C:\Windows\System32`.

### Installed (exe)

- Normal installation. Backups go to `%APPDATA%/actlog/backups/`.
- Config and DB stored in `%APPDATA%/actlog/` (or a custom path chosen during install).
- Same path resolution rule applies: use `%APPDATA%` env var, not CWD.

### Auto-Start

- On first run / install, create a `.lnk` shortcut in `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\actlog.lnk`.
- Use the `mslnk` crate to create the shortcut programmatically. No registry writes.
- The shortcut should specify `actlog-tray.exe` with no arguments and the correct "Start In" directory.

---

## Backup & Data

### Auto-Backup Strategy

- Runs on a configurable timer (default: every 15 minutes).
- **JSON export**: Serializes all sessions to `sessions-{timestamp}.json.gz`. Compressed, human-readable, grep-able. ~100KB/day.
- **DB snapshot**: Runs `VACUUM INTO 'backups/sessions-{timestamp}.db'` on the writer connection. This is a SQLite built-in (3.27+) that safely serializes the current DB state (including WAL) into a single consistent file without blocking concurrent writers. Never use `std::fs::copy` on a live WAL-mode database — the `.wal` and `.shm` files will be out of sync.
- **Retention**: Keep last N backups (default: 30). Oldest are auto-deleted.

### Import/Export

- **Export**: `GET /api/export` returns a JSON array of session objects:
  ```json
  [
    {
      "id": "018f...",
      "app": "Code.exe",
      "title": "main.rs - Visual Studio Code",
      "start_utc": 1720000000000,
      "end_utc": 1720000030000,
      "source": "foreground",
      "device_id": "abc-123"
    }
  ]
  ```
- **Import**: Accepts the same JSON format via `POST /api/import`. Idempotent — deduplicates by `start_utc + source`. If a session with matching `start_utc` and `source` already exists, it's skipped (or merged if there's a conflict).
- **File size**: ~9KB/day, ~3.3MB/year uncompressed, ~300-500KB/year gzipped. Not a concern even for decades of data.

---

## Single Instance

- On startup, call `CreateMutexW` with a well-known name (e.g., `Local\ACTLog-Instance-Mutex`).
- If `GetLastError()` returns `ERROR_ALREADY_EXISTS`, exit immediately with a message.
- Port 5566 is a secondary guard — if the mutex fails somehow, binding to the port will fail. But the mutex is the primary mechanism.
- `ui-bin` does NOT need single-instance protection — it's spawned on demand, and multiple instances are fine (user could open multiple stats windows).

---

## Extension

### Background Polling (MV3 Service Worker)

- **Problem**: Manifest V3 background scripts are service workers. Chrome suspends them after ~30s of inactivity. `setInterval` / `setTimeout` will not fire after suspension.
- **Fix**: Use the `chrome.alarms` API for background polling. `chrome.alarms.create("fetchData", { periodInMinutes: 5 })` reliably fires even when the service worker is suspended.
- On alarm fire: fetch `GET http://127.0.0.1:5566/api/sessions?since=<last_fetch_timestamp>` and update local cache (`chrome.storage.local`).

### Live Mode (Extension Page)

- When the user opens the extension popup/dashboard:
  - Cancel the `chrome.alarms` background poll.
  - Start a 3s `setInterval` fetch loop (runs in the extension page's JS context, not the background worker — no suspension risk).
  - Display live-updating stats/charts.
- When the user closes the extension page:
  - Stop the `setInterval` loop.
  - Re-register the `chrome.alarms` background poll.

### CORS

- Chrome blocks `fetch()` from `chrome-extension://<id>` to `http://127.0.0.1:5566` unless the server responds with `Access-Control-Allow-Origin`.
- **Every response** from the tiny_http server must include:
  ```
  Access-Control-Allow-Origin: chrome-extension://<extension-id>
  ```
- For development (unpacked extension), the extension ID changes each load. Use `Access-Control-Allow-Origin: *` during development, or detect the dev origin dynamically.
- For production (Web Store), the extension ID is stable and can be hardcoded.
- **Firefox note**: If Firefox support is added later, `moz-extension://<uuid>` origins are different from Chrome's and must be handled separately.

### CORS Preflight

- Since we only use GET requests and standard headers, no `OPTIONS` preflight is needed. If POST endpoints are added later (e.g., import), `Access-Control-Allow-Methods` headers and preflight handling become necessary.

---

## Known Edge Cases

| Situation | Behavior |
|-----------|----------|
| User switches apps between polls | Max 3s mis-attribution to previous app |
| System sleep during active session | On resume, AFK detects large delta, caps at max. Tracker resumes normal polling. |
| PC crash / power loss | On next startup, crash recovery caps the last session. |
| ApplicationFrameHost.exe wraps UWP | Child window enumeration finds real PID. |
| Access denied on process query | Falls back to `"<Elevated Process>"` or `"<System>"`. |
| No foreground window (desktop focused) | `GetForegroundWindow` returns NULL. Skip poll cycle. |
| User double-clicks exe | Named mutex prevents second instance. |
| Tray app not running, extension opens | Extension shows "Tray app offline" state, retries on next alarm. |
| Port 5566 already in use | Mutex guard should prevent this. If not, log error and retry with a fallback port or exit. |
| SQLite database file missing on startup | Create it fresh with schema migration. |
