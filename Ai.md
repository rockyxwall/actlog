# AGENT.md — ACTLog

> This file is the single source of truth for the project. It must be updated
> every time the code, structure, or logic changes. When starting a new context
> window, read this file first before reading any code.

---

## What this is

A portable Windows activity tracker. No installation required. Ships as a
single `.exe` inside a zip. Runs from the system tray. Tracks which app the
user is in and for how long, stores it in a local SQLite database, and shows
stats on demand.

Inspired by: github.com/aardappel/procrastitracker
Developer repo: github.com/rockyxwall/actlog

---

## Who is building this and how to behave

The developer is new to programming. They know basic HTML and CSS but this is
their first real programming project. They are learning Python by building this.

- Do not give complete copy-paste solutions. Give hints, similar examples with
  different domains, and explain the concept behind the answer.
- Explain why something is done a certain way, not just how.
- When asked to write code directly, write it. Otherwise teach first.
- Correct structure and architecture decisions early — bad habits here are hard
  to undo later.
- The developer uses `uv` as the package manager. Respect that.

---

## Project structure

```
actlog/
├── main.py          — entry point only
├── pyproject.toml
└── src/
    ├── tracker.py   — detects active window and process name, runs polling loop
    ├── db.py        — all sqlite logic and path resolution
    ├── tray.py      — system tray icon and menu
    ├── startup.py   — windows autostart via registry
    └── stats.py     — query and category logic
```

Distributed to users as:

```
actlog/
├── actlog.exe
└── database/
    └── actlog.db    — created on first run
```

---

## Core concepts

**Portable path pattern** — all paths resolve relative to the exe using a
`sys.frozen` guard. Never hardcoded, never relative to cwd.

**Session-based tracking** — one db row per continuous window focus period.
Not one row per poll tick. A session is written when focus changes or the user
goes AFK.

**AFK detection** — Windows `GetLastInputInfo` via ctypes. If no input for
180 seconds, discard time and do not write a session.

**Threading** — pystray owns the main thread on Windows. The tracker polling
loop runs in a daemon thread. Pause/resume uses `threading.Event`.

**Single responsibility** — each file does one job. tracker detects, db
stores, tray displays, stats queries. Nothing bleeds between files.

---

## Database schema

```sql
CREATE TABLE IF NOT EXISTS sessions (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    process_name TEXT    NOT NULL,
    started_at   TEXT    NOT NULL,
    ended_at     TEXT    NOT NULL,
    duration_s   INTEGER NOT NULL
)
```

Timestamps are ISO 8601 strings via `datetime.now().isoformat()`.

---

## MVP progress

- [x] Active window process name printing to terminal
- [ ] Session tracking writing to SQLite
- [ ] System tray with pause/resume
- [ ] Stats view and export
- [ ] PyInstaller portable build with autostart