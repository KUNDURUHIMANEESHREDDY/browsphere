import psutil, win32gui, win32process, win32con, time
windows=[]
def enum(hwnd,_):
    if win32gui.IsWindowVisible(hwnd):
        _,pid=win32process.GetWindowThreadProcessId(hwnd)
        try: pname=psutil.Process(pid).name().lower()
        except: return
        if pname=='browsphere-core.exe':
            windows.append((hwnd,pid,win32gui.GetWindowText(hwnd)))
win32gui.EnumWindows(enum,None)
print("BROWSPHERE_WINDOWS",windows)
for hwnd,pid,title in windows:
    win32gui.PostMessage(hwnd,win32con.WM_CLOSE,0,0)
    print("CLOSED_REQUESTED",pid,title)
time.sleep(3)
print("REMAINING",[(p.pid,p.name()) for p in psutil.process_iter() if 'browsphere-core' in p.name().lower()])
