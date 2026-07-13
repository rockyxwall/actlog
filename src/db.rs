use std::env;
use std::path::PathBuf;
use anyhow::{Result, Context};
use rusqlite::{Connection, params};

pub fn get_app_dir() -> Result<PathBuf> {
    let exe_path = env::current_exe().context("Failed to get current executable path")?;
    let dir = exe_path.parent().context("Executable has no parent directory")?;
    Ok(dir.to_path_buf())
}

pub fn init_db() -> Result<Connection> {
    let db_path = get_app_dir()?.join("actlog.sqlite");
    let conn = Connection::open(db_path).context("Failed to open SQLite database")?;
    
    // Enable WAL mode
    conn.pragma_update(None, "journal_mode", &"WAL")
        .context("Failed to set WAL journal mode")?;
        
    // Create tables
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            app TEXT NOT NULL,
            title TEXT NOT NULL,
            start_utc INTEGER NOT NULL,
            end_utc INTEGER NOT NULL,
            source TEXT NOT NULL,
            device_id TEXT NOT NULL
        );",
        [],
    ).context("Failed to create sessions table")?;
    
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sessions_start ON sessions(start_utc);",
        [],
    ).context("Failed to create index on sessions table")?;
    
    Ok(conn)
}

pub fn open_reader_conn() -> Result<Connection> {
    let db_path = get_app_dir()?.join("actlog.sqlite");
    let conn = Connection::open(db_path).context("Failed to open reader connection")?;
    conn.pragma_update(None, "journal_mode", &"WAL")
        .context("Failed to set reader WAL journal mode")?;
    Ok(conn)
}

pub fn get_or_create_device_id() -> Result<String> {
    let file_path = get_app_dir()?.join("device_id.txt");
    if file_path.exists() {
        let content = std::fs::read_to_string(&file_path)
            .context("Failed to read device_id.txt")?;
        let trimmed = content.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }
    
    let new_id = uuid::Uuid::new_v4().to_string();
    std::fs::write(&file_path, &new_id)
        .context("Failed to write device_id.txt")?;
    Ok(new_id)
}

pub fn perform_startup_recovery(conn: &Connection, device_id: &str) -> Result<()> {
    // Get the latest session row from sessions
    let mut stmt = conn.prepare(
        "SELECT id, app, title, start_utc, end_utc, source, device_id 
         FROM sessions 
         ORDER BY end_utc DESC LIMIT 1"
    ).context("Failed to prepare startup recovery query")?;
    
    let last_session = stmt.query_row([], |row| {
        Ok((
            row.get::<_, String>(0)?, // id
            row.get::<_, String>(1)?, // app
            row.get::<_, String>(2)?, // title
            row.get::<_, i64>(3)?,    // start_utc
            row.get::<_, i64>(4)?,    // end_utc
            row.get::<_, String>(5)?, // source
        ))
    });
    
    if let Ok((_id, _app, _title, _start_utc, end_utc, _source)) = last_session {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .context("Time went backwards")?
            .as_millis() as i64;
            
        // If end_utc is in the past, insert a crash gap session
        if current_time > end_utc {
            let gap_id = uuid::Uuid::now_v7().to_string();
            conn.execute(
                "INSERT INTO sessions (id, app, title, start_utc, end_utc, source, device_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    gap_id,
                    "<System Gap>",
                    "",
                    end_utc,
                    current_time,
                    "crash_gap",
                    device_id,
                ],
            ).context("Failed to insert crash gap session")?;
        }
    }
    
    Ok(())
}
