# window_tracker.py
import win32gui
import win32process
import psutil

def window_name():
    fgwcode = win32gui.GetForegroundWindow()
    pid = win32process.GetWindowThreadProcessId(fgwcode)
    app_name = psutil.Process(pid[1]).name()
    print(app_name)