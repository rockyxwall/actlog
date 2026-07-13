use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::System::SystemInformation::GetTickCount;

pub fn get_idle_seconds() -> u32 {
    let mut last_input_info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };

    unsafe {
        if GetLastInputInfo(&mut last_input_info).as_bool() {
            let current_tick = GetTickCount();
            let idle_ms = current_tick.wrapping_sub(last_input_info.dwTime);
            return idle_ms / 1000;
        }
    }
    0
}
