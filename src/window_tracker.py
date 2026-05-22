# window_tracker.py
import win32gui
import win32process
import psutil

# Get the handle of the foreground window
hwnd = win32gui.GetForegroundWindow()

# Get the title text from the handle
window_title = win32gui.GetWindowText(hwnd)

print(f"Active Window Title: {window_title}")
