# ACTLog (ACTivity Log)
```
actlog/
├── main.py          ← entry point only, nothing else
└── src/
    ├── tracker.py   ← already started: get active window + process
    ├── db.py        ← all SQLite logic lives here
    ├── tray.py      ← system tray icon + menu
    ├── startup.py   ← Windows registry read/write
    └── stats.py     ← queries for stats display
```
## MVP Roadmap

### Phase 1 — Core Data: Track and Store

- [ ] Retrieve the active window process name (exe) alongside the window title
- [ ] Design the SQLite schema: `sessions(id, ts_start, ts_end, window_title, process_name, duration_s)`
- [ ] Implement `src/db.py` with connect, insert, and basic query functions
- [ ] Refactor `src/tracker.py` to accumulate time per window and flush a session to the database on focus change

### Phase 2 — System Tray: Run Headless

- [ ] Add `pystray` and implement `src/tray.py` with a generated runtime icon
- [ ] Move the tracking loop to a background thread; tray owns the main thread
- [ ] Implement tray menu actions: Pause/Resume and Exit
- [ ] Display total time tracked today as the tray icon tooltip

### Phase 3 — Stats: View and Export

- [ ] Implement `src/stats.py` with queries for time per application (today and this week)
- [ ] Add category mapping (e.g. `chrome.exe` → browser, `code.exe` → dev)
- [ ] Build a stats popup window using `tkinter` opened from the tray menu
- [ ] Add CSV and JSON export options to the tray menu

### Phase 4 — Portable Build: No Installation Required

- [ ] Apply the portable path pattern (`sys.frozen` guard) so all file paths are relative to the executable
- [ ] Implement `src/startup.py` to read and write the Windows autostart registry key (`HKCU\...\Run`)
- [ ] Add an Enable/Disable Startup toggle to the tray menu
- [ ] Bundle the application with PyInstaller (`--onefile --noconsole`) into a single distributable `.exe`
- [ ] End-to-end test: run the build from a clean folder, enable startup, and verify autostart after a logout/login cycle
