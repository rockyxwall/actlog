import sqlite3
import sys
from pathlib import Path

def db_path():
    # figure out where this script/exe actually lives
    if getattr(sys, "frozen", False):
        base = Path(sys.executable).parent
    else:
        base = Path(__file__).parent.parent
    # creates db if not present
    data_folder = base / "database"
    db_path = data_folder / "actlog.db"
    data_folder.mkdir(parents=True, exist_ok=True)
    return(db_path)

def db_initialize():
    # connects to db
    conn = sqlite3.connect(db_path())
    cursor = conn.cursor()
    #create scema
    cursor.execute("""
        CREATE TABLE IF NOT EXISTS sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            process_name TEXT NOT NULL,
            started_at TEXT NOT NULL,
            ended_at TEXT NOT NULL,
            duration_s INTEGER NOT NULL
        )
    """)
    conn.commit()
    conn.close()