#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod tracker;
mod afk;
mod merge;
mod server;
mod logging;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use anyhow::{Result, Context};

use windows::Win32::System::Threading::{CreateMutexW, ReleaseMutex};
use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
use windows::Win32::UI::WindowsAndMessaging::{
    PeekMessageW, TranslateMessage, DispatchMessageW, MSG, PM_REMOVE
};
use windows::core::w;

use tray_icon::{TrayIconBuilder, Icon, menu::{Menu, MenuItem, MenuEvent}};

struct MutexGuard(HANDLE);

impl Drop for MutexGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = ReleaseMutex(self.0);
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

fn main() -> Result<()> {
    // 1. Single Instance Check (Named Mutex)
    let mutex_handle = unsafe {
        CreateMutexW(None, false, w!("Local\\ACTLog-Instance-Mutex"))
            .context("Failed to create named mutex")?
    };
    
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        eprintln!("Another instance of ACTLog is already running. Exiting.");
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(mutex_handle);
        }
        std::process::exit(1);
    }
    let _mutex_guard = MutexGuard(mutex_handle);
    
    // Initialize Logging (must be after single instance check to prevent file write lock contention/truncation)
    logging::init_logging().context("Failed to initialize logging")?;
    
    // 2. Initialize Database & Device ID
    let db_conn = db::init_db().context("Failed to initialize database")?;
    let device_id = db::get_or_create_device_id().context("Failed to get or create device ID")?;
    log::info!("Initialized database. Device ID: {}", device_id);
    
    // 3. Startup Recovery (Crash Gap Check)
    db::perform_startup_recovery(&db_conn, &device_id)
        .context("Failed to perform database startup recovery")?;
    
    // 4. Create Tray Icon Menu
    let tray_menu = Menu::new();
    let quit_item = MenuItem::new("Quit", true, None);
    tray_menu.append(&quit_item).context("Failed to append quit item to menu")?;
    
    // Build circular blue icon (32x32)
    let mut pixels = vec![0u8; 32 * 32 * 4];
    for y in 0..32 {
        for x in 0..32 {
            let idx = (y * 32 + x) * 4;
            let dx = x as f32 - 15.5;
            let dy = y as f32 - 15.5;
            if dx * dx + dy * dy <= 144.0 {
                pixels[idx] = 40;     // R
                pixels[idx+1] = 110;  // G
                pixels[idx+2] = 230;  // B
                pixels[idx+3] = 255;  // A
            } else {
                pixels[idx] = 0;
                pixels[idx+1] = 0;
                pixels[idx+2] = 0;
                pixels[idx+3] = 0;    // Transparent
            }
        }
    }
    let icon = Icon::from_rgba(pixels, 32, 32).context("Failed to create tray icon from RGBA")?;
    
    let tray_result = TrayIconBuilder::new()
        .with_tooltip("ACTLog Time Tracker")
        .with_menu(Box::new(tray_menu))
        .with_icon(icon)
        .build();
        
    let (_tray_icon, has_tray) = match tray_result {
        Ok(ti) => {
            log::info!("Tray icon created. Press Quit to exit.");
            (Some(ti), true)
        }
        Err(e) => {
            log::warn!("Warning: Failed to create tray icon (headless environment?): {:?}", e);
            log::info!("Running in headless daemon mode. Use Ctrl+C or kill the process to stop.");
            (None, false)
        }
    };
    
    // 5. Spawn Threads
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    
    // Spawn Server thread
    let server_shutdown = shutdown_flag.clone();
    let _server_thread = thread::spawn(move || {
        if let Err(e) = server::run_server(server_shutdown) {
            log::error!("REST server thread error: {:?}", e);
        }
    });
    
    // Spawn Tracker thread
    let tracker_shutdown = shutdown_flag.clone();
    let tracker_device_id = device_id.clone();
    let tracker_thread = thread::spawn(move || {
        match db::init_db() {
            Ok(tracker_conn) => {
                if let Err(e) = tracker::run_tracker_loop(tracker_conn, tracker_device_id, tracker_shutdown) {
                    log::error!("Tracker thread error: {:?}", e);
                }
            }
            Err(e) => {
                log::error!("Failed to initialize tracker DB connection: {:?}", e);
            }
        }
    });
    
    // 6. Non-blocking Message Loop (PeekMessageW) / Headless Loop
    if has_tray {
        let quit_id = quit_item.id();
        while !shutdown_flag.load(Ordering::Relaxed) {
            let mut msg = MSG::default();
            unsafe {
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            
            while let Ok(event) = MenuEvent::receiver().try_recv() {
                if event.id == quit_id {
                    log::info!("Quit requested via tray menu.");
                    shutdown_flag.store(true, Ordering::SeqCst);
                    break;
                }
            }
            
            thread::sleep(Duration::from_millis(10));
        }
    } else {
        // Headless execution: sleep in loop until shutdown or Ctrl+C
        while !shutdown_flag.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(500));
        }
    }
    
    // 7. Cleanup & Graceful Join
    log::info!("Shutting down threads...");
    let _ = tracker_thread.join();
    // Since tiny_http might be blocking on TcpListener, we can join with a short sleep or exit.
    // The main process returning will drop _tray_icon, _mutex_guard, and terminate the server thread immediately.
    
    log::info!("ACTLog exited cleanly.");
    Ok(())
}
