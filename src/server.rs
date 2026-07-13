use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use anyhow::{Result, Context};
use tiny_http::{Server, Response, Header, Method};
use serde::Serialize;
use rusqlite::params;

#[derive(Serialize)]
struct Session {
    id: String,
    app: String,
    title: String,
    start_utc: i64,
    end_utc: i64,
    source: String,
    device_id: String,
}

fn handle_request(request: tiny_http::Request) -> Result<()> {
    let url = request.url();
    
    // We only support GET /api/sessions
    if request.method() != &Method::Get || !url.starts_with("/api/sessions") {
        let response = Response::from_string("Not Found")
            .with_status_code(404)
            .with_header(Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap());
        request.respond(response).context("Failed to send 404 response")?;
        return Ok(());
    }
    
    // Parse 'since' parameter
    let mut since: Option<i64> = None;
    if let Some(pos) = url.find("since=") {
        let param_val = &url[pos + 6..];
        let val_str = param_val.split('&').next().unwrap_or(param_val);
        if let Ok(parsed) = val_str.parse::<i64>() {
            since = Some(parsed);
        }
    }
    
    // If not provided, default to last 24 hours
    let since_val = since.unwrap_or_else(|| {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        current_time - (24 * 60 * 60 * 1000) // last 24h in ms
    });
    
    // Open a new read-only DB connection
    let conn = crate::db::open_reader_conn()?;
    
    // Query sessions
    let mut stmt = conn.prepare(
        "SELECT id, app, title, start_utc, end_utc, source, device_id 
         FROM sessions 
         WHERE end_utc > ?1 
         ORDER BY start_utc ASC"
    ).context("Failed to prepare query sessions statement")?;
    
    let session_iter = stmt.query_map([since_val], |row| {
        Ok(Session {
            id: row.get(0)?,
            app: row.get(1)?,
            title: row.get(2)?,
            start_utc: row.get(3)?,
            end_utc: row.get(4)?,
            source: row.get(5)?,
            device_id: row.get(6)?,
        })
    }).context("Failed to execute sessions query")?;
    
    let mut sessions = Vec::new();
    for session in session_iter {
        sessions.push(session?);
    }
    
    // Serialize to JSON
    let json_data = serde_json::to_string(&sessions).context("Failed to serialize sessions to JSON")?;
    
    // Send response
    let response = Response::from_string(json_data)
        .with_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap())
        .with_header(Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap());
        
    request.respond(response).context("Failed to send sessions response")?;
    
    Ok(())
}

pub fn run_server(shutdown: Arc<AtomicBool>) -> Result<()> {
    let server = Server::http("127.0.0.1:5566")
        .map_err(|e| anyhow::anyhow!("Failed to start tiny_http server: {}", e))?;
        
    println!("REST API server listening on 127.0.0.1:5566");
    
    while !shutdown.load(Ordering::Relaxed) {
        match server.try_recv() {
            Ok(Some(request)) => {
                std::thread::spawn(move || {
                    if let Err(e) = handle_request(request) {
                        eprintln!("Error handling request: {:?}", e);
                    }
                });
            }
            Ok(None) => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                eprintln!("Server receive error: {:?}", e);
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
    
    Ok(())
}

// Re-export Duration for the sleep calls
use std::time::Duration;
