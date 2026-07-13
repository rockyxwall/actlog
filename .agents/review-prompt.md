You are an expert Rust systems architect with 15+ years of experience in desktop application design, Win32 programming, and browser extension architecture. Your task is to critically review the following design for a Windows time-tracker project (ACTLog).

## Your role
- Be genuinely critical. The user WANTS to find real problems before writing code.
- If everything looks good, say so — do not invent fake problems.
- Every claim you make must be specific and actionable: not "this might be a problem" but "On Windows 10 22H2, QueryFullProcessImageNameW returns ERROR_ACCESS_DENIED for PID 4 (System) and PIDs owned by TrustedInstaller — the code needs to handle this by silently skipping those processes."
- If unsure about something, say "I don't know" rather than guessing.

## The design to review

### Tech stack
- Tracking: windows-rs (GetForegroundWindow → GetWindowThreadProcessId → QueryFullProcessImageNameW)
- AFK: GetLastInputInfo, idle ≥180s → source=afk, app=idle
- Merge: heartbeat-merge (if latest row matches app+title and gap ≤ 6-9s, extend end_utc; else insert new UUID7 row)
- DB: rusqlite bundled, WAL mode, Mutex for writes, separate readers
- Server: tiny_http REST on 127.0.0.1:5566
- Tray: tray-icon + muda
- Desktop UI: egui/eframe minimal window (launchable from tray)
- Extension sync: REST (read-only from tray to extension)
- Event loop: tao (via eframe)
- No WebSocket, no tokio, no Tauri, no registry writes

### Data flow
Tray app (background collector) → local REST API (GET only) → Chrome extension (primary UI). Extension never sends data to tray. Extension polls every 5-10min in background, switches to 3s polling when user opens the extension UI, resumes background rate when UI closes.

### Distribution
- Portable (zip): all files including backups alongside the exe
- Installed (exe): backups in %APPDATA%/actlog/backups/
- Auto-start via Start Menu shortcut

### Backup
Both:
1. Compressed JSON export (sessions.json.gz) — portable, inspectable
2. WAL-safe SQLite DB hot-copy — full state restore

### Import/export
JSON format. Idempotent import (dedup by start_utc + source).

### Schema
sessions(app, title, start_utc, end_utc, source, device_id) — device_id included from day one for multi-device.

### Poll
~3s foreground poll. GAP_LIMIT 6-9s. Max data loss = ~3s.

### Binary size
opt-level="z", lto=true, panic="abort", strip=true. Target <50MB.

### Workspace layout
actlog/core/ (tracker, afk, merge, db)
actlog/server/ (REST API)
actlog/ui/ (egui window)
actlog/tray-bin/ (main binary)
actlog/extension/ (Chrome MV3 extension)

## What I need you to analyze

For each of the following categories, give me:
1. REAL problems that WILL bite the user (not hypothetical edge cases that don't matter)
2. Concrete solutions or alternatives

### Categories
A. **Win32/OS-level issues**: What can go wrong with GetForegroundWindow, GetLastInputInfo, process queries on modern Windows? Handle Win10 vs Win11 differences? What about protected processes, PPL, UWP apps, Windows Terminal?

B. **Threading & event loop**: tray-icon needs an event loop. egui/eframe needs an event loop. tiny_http runs on its own thread. Tracker runs on its own thread. Does this design actually wire together correctly? What's the exact startup sequence? Where does the event loop live?

C. **REST API & extension**: tiny_http is single-threaded sync. Extension polls every 3s in live mode. Can tiny_http handle concurrent requests? What about CORS headers for chrome-extension:// origins? What happens if the tray app isn't running when the extension starts?

D. **Database & data integrity**: Mutex for writes, separate readers — is this actually safe with rusqlite's WAL mode? What about backup consistency (JSON and DB snapshot must be from the same point in time)? Import dedup logic — what if timestamps have collision? Schema migration strategy?

E. **Dependency choices**: Are there known issues with specific versions of these crates? Does eframe's tao version conflict with tray-icon's tao version? Does tiny_http support the features needed (CORS headers, chunked responses, keep-alive)? Is there a better crate choice?

F. **Portability & distribution**: How does the portable version handle the backup path being "./backups/" relative to the exe? What about the first-run experience — empty DB, extension shows nothing? How does the user configure the backup folder?

G. **Missing pieces**: What important things are NOT in this design that should be? (Examples: logging framework, panic handling, error reporting, install/uninstall, config file format, CTRL+C handling, Windows session lock/unlock events)

## Output format
For each category, output:
```
## Category: [name]
### Problems found: [number]
1. [Problem description + why it WILL manifest in practice]
   - Fix: [concrete solution with code/crate suggestion]

### No issues: (if nothing real to flag)
```

## Warnings (read carefully)
- Do NOT suggest replacing tiny_http with axum/tokio unless you have a SPECIFIC concrete reason. The user chose sync to keep binary small.
- Do NOT suggest "add more abstraction layers" or "make it more generic" unless it solves a real problem.
- Do NOT suggest WebSocket. It was rejected for MV3 service worker lifecycle issues.
- Do NOT suggest full Tauri. It was rejected for binary size.
- Do NOT suggest registry writes. It was rejected for portability.
- Do NOT invent problems about scaling to millions of users — this is a single-user local app.
- If a crate is commonly used and well-maintained, do NOT flag it as a risk without evidence.
- Focus on problems that will cause data loss, crashes, UX failure, or major rework.

---

## Full design context below

Everything from here down is the decision log — the questions the user was asked, their answers, and the final design decisions they made. Use this to understand WHY each decision was made so you can evaluate whether the reasoning is sound.

---

# ACTLog — Design Decision Log

## Original User Requirements (as stated)

- Independent watcher like ActivityWatch but truly bare-minimum UI
- Collects a lot of data, fully shown through a Chrome extension
- Chrome extension is the PRIMARY app — not the desktop tray app
- Data must be properly backed up automatically
- Must not lose data
- Seamless import and export data feature
- Portable (zip) and installed (exe) distribution modes
- Single self-contained folder (no registry writes, no external DLLs)
- <50MB binary, few moving parts

---

## Questions Asked & Answers Given

### Q1: Desktop UI scope
*How minimal should the desktop UI be?*

**User answer**: Minimal egui window (launchable from tray menu)

**Decision**: egui/eframe minimal stats window. Tray icon + menu. Chrome extension is the full UI.

---

### Q2: Extension sync method
*REST vs WebSocket for extension↔tray communication?*

**User answer**: REST (tiny_http) — Recommended

**Decision**: tiny_http REST on 127.0.0.1:5566. No WebSocket. Avoids MV3 service worker lifecycle issues.

---

### Q3: Backup strategy
*JSON export only, DB snapshot only, or both?*

*Explanation given:*
- JSON export: compressed, human-readable, scriptable, ~100KB/day
- DB snapshot: full SQLite copy, instant restore, binary
- Both: JSON for portability/inspection, DB for full restore

**User answer**: Both (Recommended)

**Decision**: Auto-backup writes both sessions.json.gz AND a hot-copy of the SQLite DB (via WAL-safe copy) on a timer.

---

### Q4: Import/export format
*JSON (portable) vs SQLite-only? Concern: "won't it become too big?"*

*Explanation given:*
- ~60 sessions/day × 150 bytes = ~9KB/day
- ~3.3MB/year uncompressed
- ~300-500KB/year gzip compressed
- Not a size concern even for decades of data

**User answer**: JSON (portable)

**Decision**: Export/import as JSON array of session objects. Trivially portable, scriptable, inspectable.

---

### Q5: Extension data collection scope
*What does the Chrome extension collect?*

*Clarification later: user said extension does NOT send browser data to tray app. Extension is read-only.*

**User answer**: No browser data collected. Extension only reads from tray API.

**Decision**: Extension is pure visualization. No tab tracking, no URL/navigation data collected. Only GET endpoints on the REST API.

---

### Q6: Poll interval (data loss tolerance)
*~500ms (tight), ~3s (balanced), or ~10s (coarse)?*

*Explanation given:*
- 500ms poll: max ~1s mis-attribution, ~2% CPU, tight accuracy
- 3s poll: max ~3s mis-attribution, negligible CPU, recommended balance
- 10s poll: max ~10s mis-attribution, minimal CPU, coarser summaries
- GAP_LIMIT should be ~2× poll interval

**User answer**: ~3 seconds (balanced)

**Decision**: ~3s foreground poll. GAP_LIMIT = 6-9s for heartbeat-merge.

---

### Q7: Auto-start with Windows
*Tray app auto-start at login?*

**User answer**: Yes, auto-start (Recommended)

**Decision**: Register via Start Menu shortcut (not registry). No registry writes.

---

### Q8: Backup location
*Where should backups go?*

*User clarified:* Portable version → alongside the exe. Installed version → app data folder.

**User answer**: %APPDATA%/actlog for installed version

**Decision**: Portable: `./backups/` next to exe. Installed: `%APPDATA%/actlog/backups/`.

---

### Q9: REST API port
*Which port for the tiny_http server?*

**User answer**: 5566 (confirmed free — not used by PostgreSQL 5432, ActivityWatch 5600, or any major service)

**Decision**: 127.0.0.1:5566

---

### Q10: Multi-device support
*Single device or multi-device ready?*

**User answer**: Multi-device ready (Recommended)

**Decision**: Include `device_id` column in sessions schema from day one. Tag data per machine for future multi-machine merge.

---

### Q11: Extension fetch strategy (re-asked after data flow clarification)
*How often does the extension fetch data from the tray app?*

*Explanation given:*
- Since extension is read-only and is the PRIMARY UI, it needs to fetch data
- Background polling vs on-demand vs both

**User answer**: 
- Background: pull every 5-10 minutes
- When user opens extension UI: pull every 3 seconds (live mode)
- When user closes extension UI: resume 5-10 minute background interval

**Decision**: Two-tier polling. Background 5-10min. Live 3s when UI visible.

---

## Final Architecture Summary

```
┌──────────────────────────────────────────────────────────┐
│                    Windows Machine                        │
│                                                          │
│  ┌─────────────────┐    ┌────────────────────────────┐  │
│  │   tray-bin       │    │   Chrome Extension         │  │
│  │   (background)   │    │   (PRIMARY UI)             │  │
│  │                  │    │                            │  │
│  │  core/           │    │  Polls GET /sessions       │  │
│  │  └─ tracker.rs   │◄───│  on background 5-10min    │  │
│  │  └─ afk.rs       │    │  + live 3s when open      │  │
│  │  └─ merge.rs     │    │                            │  │
│  │  └─ db.rs        │    │  Never sends data to tray  │  │
│  │                  │    │  (read-only client)        │  │
│  │  server/         │    │                            │  │
│  │  └─ api.rs       │───►│  Port 5566 (tiny_http)    │  │
│  │      (GET only)  │    │                            │  │
│  │                  │    │  Shows stats, charts,      │  │
│  │  ┌──────────────┐│    │  history, reports          │  │
│  │  │ Backup:      ││    │                            │  │
│  │  │ JSON + DB    ││    │                            │  │
│  │  │ snapshots    ││    │                            │  │
│  │  └──────────────┘│    └────────────────────────────┘  │
│  └─────────────────┘                                     │
│                                                          │
│  ┌─ egui window (optional, launched from tray menu) ──┐  │
│  │ Minimal stats panel. Not the primary UI.            │  │
│  └──────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────┘
```

### Key Invariants
1. **Extension is the main app** — tray app is a headless collector with a tiny optional stats window
2. **Read-only data flow** — extension never POSTs data to tray
3. **No browser data collection** — no tab URLs, no navigation data
4. **Two distribution modes** — portable (zip, all in one folder) and installed (exe, %APPDATA% for backups)
5. **No registry writes** — auto-start via Start Menu shortcut
6. **No WebSocket** — avoids MV3 service worker kill issues
7. **No external runtime** — SQLite bundled, WebView2 not needed (egui instead of wry)
8. **Multi-device ready** — device_id column from day one
9. **Zero data loss design** — 3s poll + WAL mode + JSON/DB backups
10. **Portable import/export** — JSON format, trivially small (~3MB/year)
