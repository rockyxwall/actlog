use rusqlite::{Connection, params};
use anyhow::{Result, Context};
use uuid::Uuid;

pub fn record_session(
    conn: &Connection,
    device_id: &str,
    app: &str,
    title: &str,
    source: &str,
    ts: i64,
) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT id, app, title, start_utc, end_utc, source 
         FROM sessions 
         ORDER BY end_utc DESC LIMIT 1"
    ).context("Failed to prepare last session query")?;
    
    let last_row = stmt.query_row([], |row| {
        Ok((
            row.get::<_, String>(0)?, // id
            row.get::<_, String>(1)?, // app
            row.get::<_, String>(2)?, // title
            row.get::<_, i64>(3)?,    // start_utc
            row.get::<_, i64>(4)?,    // end_utc
            row.get::<_, String>(5)?, // source
        ))
    });
    
    match last_row {
        Ok((id, last_app, last_title, _start_utc, end_utc, last_source)) => {
            let gap = ts - end_utc;
            if last_app == app && last_title == title && last_source == source && gap >= 0 && gap <= 9000 {
                conn.execute(
                    "UPDATE sessions SET end_utc = ?1 WHERE id = ?2",
                    params![ts, id],
                ).context("Failed to update session end_utc")?;
            } else {
                let new_id = Uuid::now_v7().to_string();
                conn.execute(
                    "INSERT INTO sessions (id, app, title, start_utc, end_utc, source, device_id)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![new_id, app, title, ts, ts, source, device_id],
                ).context("Failed to insert new session")?;
            }
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            let new_id = Uuid::now_v7().to_string();
            conn.execute(
                "INSERT INTO sessions (id, app, title, start_utc, end_utc, source, device_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![new_id, app, title, ts, ts, source, device_id],
            ).context("Failed to insert first session")?;
        }
        Err(e) => {
            return Err(e).context("Error querying last session");
        }
    }
    
    Ok(())
}
