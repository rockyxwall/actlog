use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use anyhow::{Result, Context};
use rusqlite::Connection;

use windows::Win32::Foundation::{HWND, LPARAM, BOOL, TRUE, FALSE};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowThreadProcessId, EnumChildWindows,
    GetWindowTextLengthW, GetWindowTextW
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_NAME_FORMAT
};
use windows::core::PWSTR;

use crate::afk;
use crate::merge;

struct UwpResolverState {
    parent_pid: u32,
    child_pid: Option<u32>,
}

unsafe extern "system" fn enum_child_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let state = &mut *(lparam.0 as *mut UwpResolverState);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        
        if pid != 0 && pid != state.parent_pid {
            state.child_pid = Some(pid);
            return FALSE; // Stop enumeration
        }
        TRUE // Continue enumeration
    }
}

fn get_process_name(pid: u32) -> Result<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
            .context("Failed to open process")?;
        
        let mut buffer = [0u16; 1024];
        let mut size = buffer.len() as u32;
        
        let res = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        );
        
        let _ = windows::Win32::Foundation::CloseHandle(handle);
        
        if res.is_ok() {
            let path = String::from_utf16_lossy(&buffer[..size as usize]);
            let filename = std::path::Path::new(&path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or(path);
            Ok(filename)
        } else {
            anyhow::bail!("Access denied or query failed");
        }
    }
}

fn get_window_title(hwnd: HWND) -> String {
    unsafe {
        let length = GetWindowTextLengthW(hwnd);
        if length == 0 {
            return String::new();
        }
        let mut buffer = vec![0u16; (length + 1) as usize];
        let copied_count = GetWindowTextW(hwnd, &mut buffer);
        if copied_count > 0 {
            String::from_utf16_lossy(&buffer[..copied_count as usize])
        } else {
            String::new()
        }
    }
}

pub fn run_tracker_loop(
    conn: Connection,
    device_id: String,
    shutdown: Arc<AtomicBool>,
) -> Result<()> {
    while !shutdown.load(Ordering::Relaxed) {
        let start_cycle = SystemTime::now();
        
        // 1. Check AFK state
        let idle_secs = afk::get_idle_seconds();
        let (app, title, source) = if idle_secs >= 180 {
            ("idle".to_string(), "".to_string(), "afk".to_string())
        } else {
            // 2. Query foreground window
            let hwnd = unsafe { GetForegroundWindow() };
            if hwnd == HWND::default() {
                // Skip cycle if no foreground window
                thread::sleep(Duration::from_secs(3));
                continue;
            }
            
            let mut pid = 0u32;
            unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
            
            if pid == 0 {
                thread::sleep(Duration::from_secs(3));
                continue;
            }
            
            // Resolve process name
            let mut process_name = match get_process_name(pid) {
                Ok(name) => name,
                Err(_) => "<Elevated Process>".to_string(),
            };
            
            // UWP wrapper handling
            if process_name == "ApplicationFrameHost.exe" {
                let mut state = UwpResolverState {
                    parent_pid: pid,
                    child_pid: None,
                };
                unsafe {
                    let lparam = LPARAM(&mut state as *mut UwpResolverState as isize);
                    let _ = EnumChildWindows(hwnd, Some(enum_child_proc), lparam);
                }
                
                if let Some(child_pid) = state.child_pid {
                    if let Ok(child_name) = get_process_name(child_pid) {
                        process_name = child_name;
                    }
                }
            }
            
            // Lock screen check
            if process_name == "LogonUI.exe" || process_name == "LockApp.exe" {
                ("locked".to_string(), "".to_string(), "afk".to_string())
            } else {
                let window_title = get_window_title(hwnd);
                (process_name, window_title, "foreground".to_string())
            }
        };
        
        // 3. Persist / Merge session
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("Time went backwards")?
            .as_millis() as i64;
            
        if let Err(e) = merge::record_session(&conn, &device_id, &app, &title, &source, ts) {
            log::error!("Error recording session: {:?}", e);
        }
        
        // Align to exactly 3-second intervals
        let elapsed = SystemTime::now()
            .duration_since(start_cycle)
            .unwrap_or(Duration::from_secs(0));
            
        let sleep_duration = Duration::from_secs(3).checked_sub(elapsed)
            .unwrap_or(Duration::from_secs(0));
            
        thread::sleep(sleep_duration);
    }
    
    Ok(())
}
