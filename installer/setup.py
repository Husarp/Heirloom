"""HeirloomSetup-X.Y.Z.exe - installs, updates and uninstalls Heirloom (APP-STANDARDS.md sections 1 and 5).

Built by scripts/build.ps1, after the Reckless Driving installer: per-user into %LOCALAPPDATA%\\Programs\\Heirloom,
no administrator rights. Install or update is not a separate build: the exe reads DisplayVersion from its uninstall
key and, if Heirloom is already there, says Aktualizuj instead of Zainstaluj.

What it never touches: the family archives. They are ordinary folders the family chose (rodzina.ged, media/,
.heirloom/), anywhere on the disk, and neither an update nor an uninstall goes near them - nor near
%LOCALAPPDATA%\\Heirloom\\archives, where an archive in a read-only folder keeps its history and backup copies.
The program's own settings (%APPDATA%\\Heirloom: recent archives, theme), its thumbnails (%LOCALAPPDATA%\\Heirloom\\
cache) and the window's data (%LOCALAPPDATA%\\com.husarp.heirloom) stay too, unless the box is ticked while
uninstalling - and then they go to the Recycle Bin; Windows asks first if a folder can't go there.

Heirloom saves only when told to, and a running Heirloom says whether closing it now would lose work: it keeps
%LOCALAPPDATA%\\Heirloom\\running\\<pid>.json (crates/heirloom-api/src/running.rs) up to date - unsaved changes, an open
section's draft - and removes it when it exits. With nothing unsaved, OK closes it the way its own ✕ does (WM_CLOSE to
its window). With unsaved work the page says so and offers „Czekaj” (save in Heirloom; the installer watches and goes
on by itself once nothing is left to save or Heirloom is gone), „Zamknij mimo to” (asked once more, then taskkill /F)
and „Anuluj”. An older Heirloom that writes no such file (0.4.0) is asked to close and asks about its changes itself.
Never taskkill without /F: it sends WM_CLOSE to every top-level window of the process, also to the invisible one
Heirloom's event loop runs through, which then hangs Heirloom - the freeze seen with 0.4.0. And no program file is
moved or replaced while heirloom.exe runs (copy_files checks that once more right before).

The window (Heirloom's light look: the colours of src/styles/tokens.css, its logo, serif headings) goes welcome
(Zainstaluj X / Aktualizuj A -> X) -> "Heirloom jest uruchomiony" / "Heirloom ma niezapisane zmiany" when it is open ->
progress (a bar driven by the real steps, „Pokaż szczegóły” for the log) -> finish (a ticked „Uruchom Heirloom”). A
failure keeps the error on screen with „Spróbuj ponownie” and „Zamknij”: the files it replaced are put back first, and
the old version is started again when the window closes. Uninstalling uses the same window.

In-app update: Heirloom asks about unsaved changes, starts `HeirloomSetup-X.Y.Z.exe --update` and exits. This waits
up to 15 s for heirloom.exe to be gone (then the "running" page, as above), shows only the progress page, starts
Heirloom again and closes by itself.

Everything that touches Windows runs on a worker thread (`work`), which only posts to a queue; the Tk thread drains
it (`SetupWindow._drain`).
"""
import csv
import ctypes
import json
import math
import os
import queue
import shutil
import subprocess
import sys
import threading
import time
import tkinter as tk
import tkinter.font as tkfont
import uuid
import winreg
import zipfile
from ctypes import wintypes
from pathlib import Path

from version import VERSION   # written by scripts/build.ps1 from Cargo.toml

APP = "Heirloom"
EXE_NAME = "heirloom.exe"
FOLDER = "Heirloom"
UNINSTALLER = f"Odinstaluj {APP}.exe"
SHORTCUT = f"{APP}.lnk"
NEWLINE = chr(10)   # written this way so the source survives being edited by scripts
NO_WINDOW = getattr(subprocess, "CREATE_NO_WINDOW", 0x08000000)   # (the name exists on Windows only)
UNINSTALL_KEY = rf"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{FOLDER}"
# The Microsoft Edge WebView2 runtime draws Heirloom's window (it comes with Windows 11 and current Windows 10).
WEBVIEW2 = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
CLOSE_WAIT = 15   # seconds a closing Heirloom gets before the window says it is still open
WATCH_EVERY = 0.8   # seconds between two looks at a running Heirloom (the "running" page)


# ---------- Windows folders, asked from Windows itself (a redirected desktop, OneDrive, a moved AppData) ----------

class GUID(ctypes.Structure):
    _fields_ = [("Data1", wintypes.DWORD), ("Data2", wintypes.WORD), ("Data3", wintypes.WORD), ("Data4", ctypes.c_ubyte * 8)]

    def __init__(self, text: str):
        super().__init__()
        u = uuid.UUID(text)
        self.Data1, self.Data2, self.Data3 = u.fields[0], u.fields[1], u.fields[2]
        for i, b in enumerate(u.bytes[8:]):
            self.Data4[i] = b


def known_folder(guid: str) -> Path | None:
    """SHGetKnownFolderPath; None when Windows can't say (then nothing is created or removed there)."""
    out = ctypes.c_wchar_p()
    if ctypes.windll.shell32.SHGetKnownFolderPath(ctypes.byref(GUID(guid)), 0, None, ctypes.byref(out)) != 0:
        return None
    try:
        path = Path(out.value) if out.value else None
    finally:
        ctypes.windll.ole32.CoTaskMemFree(out)
    return path if path and path.is_absolute() else None


LOCAL = known_folder("F1B32785-6FBA-4FCF-9D55-7B8E7F157091")      # %LOCALAPPDATA%
ROAMING = known_folder("3EB685DB-65F9-4CF6-A03A-E3EF65729F3D")    # %APPDATA%
DESKTOP = known_folder("B4BFCC3A-DB2C-424C-B029-7FE99A87C641")
START_MENU = known_folder("A77F5D77-2E2B-44C3-A6A2-ABA601054A51")  # the user's Start menu Programs
INSTALL_DIR = LOCAL / "Programs" / FOLDER if LOCAL else None
# Where an install moves the files it replaces until it has finished (put back if a step fails).
BACKUP = LOCAL / "Programs" / f"{FOLDER}.poprzednia-wersja" if LOCAL else None
# The program's own data. Never the family archives, and never %LOCALAPPDATA%\Heirloom\archives (see the docstring).
# What a running Heirloom says about itself (crates/heirloom-api/src/running.rs): <pid>.json.
RUNNING = LOCAL / "Heirloom" / "running" if LOCAL else None
OWN_DATA = [p for p in (ROAMING and ROAMING / "Heirloom", LOCAL and LOCAL / "Heirloom" / "cache", LOCAL and LOCAL / "com.husarp.heirloom") if p]

SYSTEM32 = Path(os.environ.get("SystemRoot", r"C:\Windows")) / "System32"
# Full paths: a user's PATH is not guaranteed to include System32.
TOOLS = {"tasklist": SYSTEM32 / "tasklist.exe",
         "taskkill": SYSTEM32 / "taskkill.exe",
         "powershell": SYSTEM32 / r"WindowsPowerShell\v1.0\powershell.exe",
         "cmd": SYSTEM32 / "cmd.exe",
         "explorer.exe": Path(os.environ.get("SystemRoot", r"C:\Windows")) / "explorer.exe"}


def tool(name: str) -> str:
    return str(TOOLS.get(name, name))


def run(*args, env: dict | None = None) -> subprocess.CompletedProcess:
    """errors="replace": Windows' tools answer in the console's code page (on a Polish Windows tasklist's „Brak
    uruchomionych zadań spełniających…” has letters the ANSI code page can't read); only ASCII names are looked
    for."""
    return subprocess.run([tool(args[0]), *args[1:]], capture_output=True, text=True, errors="replace",
                          creationflags=NO_WINDOW, env=env)


def payload() -> Path:
    return Path(getattr(sys, "_MEIPASS", Path(__file__).parent)) / "payload.zip"


def installed_version() -> str | None:
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, UNINSTALL_KEY) as key:
            return winreg.QueryValueEx(key, "DisplayVersion")[0]
    except OSError:
        return None


def version_key(text: str | None) -> tuple:
    """Compared as numbers: 0.10.0 is newer than 0.9.3."""
    parts = []
    for piece in (text or "0").split("."):
        digits = "".join(ch for ch in piece if ch.isdigit())
        parts.append(int(digits) if digits else 0)
    return tuple(parts)


def plural(n: int) -> int:
    """Polish number forms: 0 = one („1 zmiana”), 1 = few („3 zmiany”), 2 = many („5 zmian”)."""
    return 0 if n == 1 else 1 if 2 <= n % 10 <= 4 and not 12 <= n % 100 <= 14 else 2


def count_pl(n: int, one: str, few: str, many: str) -> str:
    return f"{n} {(one, few, many)[plural(n)]}"


def has_webview2() -> bool:
    """Registered for the whole machine (either registry view) or for this user."""
    places = [(winreg.HKEY_LOCAL_MACHINE, rf"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{WEBVIEW2}"),
              (winreg.HKEY_LOCAL_MACHINE, rf"SOFTWARE\Microsoft\EdgeUpdate\Clients\{WEBVIEW2}"),
              (winreg.HKEY_CURRENT_USER, rf"Software\Microsoft\EdgeUpdate\Clients\{WEBVIEW2}")]
    for root, path in places:
        try:
            with winreg.OpenKey(root, path) as key:
                version = winreg.QueryValueEx(key, "pv")[0]
                if version and version != "0.0.0.0":
                    return True
        except OSError:
            continue
    return False


def heirloom_pids() -> list | None:
    """The running heirloom.exe processes (tasklist as CSV: "heirloom.exe","1234",…); None when tasklist failed."""
    result = run("tasklist", "/FI", f"IMAGENAME eq {EXE_NAME}", "/NH", "/FO", "CSV")
    if result.returncode != 0:
        return None
    return [int(row[1]) for row in csv.reader(result.stdout.splitlines())
            if len(row) > 1 and row[0].lower() == EXE_NAME and row[1].isdigit()]


def app_running() -> bool:
    return bool(heirloom_pids())


# ---------- a running Heirloom: what it would lose, and closing it ----------

def said_by(pid: int) -> dict | None:
    """What Heirloom with this pid wrote about itself, or None: an older Heirloom (0.4.0 writes nothing) or a file
    that can't be read."""
    if RUNNING is None:
        return None
    try:
        state = json.loads((RUNNING / f"{pid}.json").read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    return state if isinstance(state, dict) and state.get("pid") == pid else None


def forget_gone(pids):
    """Files left by a Heirloom that did not exit normally (a crash, Task Manager): no heirloom.exe has their pid. Only
    older than a minute - one that has just started may not have been in the process list yet."""
    if RUNNING is None or not RUNNING.is_dir():
        return
    for path in RUNNING.glob("*.json"):
        try:
            if path.stem.isdigit() and int(path.stem) not in pids and time.time() - path.stat().st_mtime > 60:
                path.unlink()
        except OSError:
            pass


def look() -> dict:
    """Is Heirloom running, and would closing it now lose work? kind: "none", "unsaved" (changes, or an open section's
    draft), "unknown" (it doesn't say - Heirloom 0.4.0) or "clean". Never raises: a failed look is "none" (and
    copy_files still won't touch a running exe)."""
    try:
        pids = heirloom_pids()
    except Exception:
        pids = None
    if pids is None:   # (tasklist failed: no file is taken for a leftover - its Heirloom may well be running)
        pids = []
    else:
        forget_gone(pids)
    said = [s for s in (said_by(pid) for pid in pids) if s]
    unsaved = sum(int(s.get("unsavedChanges") or 0) for s in said)
    draft = any(bool(s.get("draft")) for s in said)
    archives = sorted({str(s["archive"]) for s in said if s.get("archive") and (s.get("unsavedChanges") or s.get("draft"))})
    kind = ("none" if not pids else "unsaved" if unsaved or draft else "unknown" if len(said) < len(pids) else "clean")
    return {"kind": kind, "pids": pids, "unsaved": unsaved, "draft": draft, "archives": archives}


WNDENUMPROC = getattr(ctypes, "WINFUNCTYPE", ctypes.CFUNCTYPE)(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
TAO_TARGET = "Tao Thread Event Target"


def user32():
    u = ctypes.windll.user32
    u.EnumWindows.argtypes = [WNDENUMPROC, wintypes.LPARAM]
    u.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    u.GetWindow.argtypes, u.GetWindow.restype = [wintypes.HWND, wintypes.UINT], wintypes.HWND
    u.GetWindowLongW.argtypes = [wintypes.HWND, ctypes.c_int]
    u.GetClassNameW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
    for name in ("IsWindowVisible", "IsIconic", "SetForegroundWindow"):
        getattr(u, name).argtypes = [wintypes.HWND]
    u.ShowWindow.argtypes = [wintypes.HWND, ctypes.c_int]
    u.PostMessageW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
    return u


def app_windows(pids) -> list:
    """Heirloom's own windows: visible, top-level, not owned, not tool windows. Never the invisible one its event loop
    runs through (TAO_TARGET): WM_CLOSE there destroys it and leaves Heirloom hung."""
    u, found = user32(), []

    def each(hwnd, _):
        pid = wintypes.DWORD()
        u.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
        if (pid.value in pids and u.IsWindowVisible(hwnd) and not u.GetWindow(hwnd, 4)   # GW_OWNER
                and not u.GetWindowLongW(hwnd, -20) & 0x80):                              # GWL_EXSTYLE, WS_EX_TOOLWINDOW
            name = ctypes.create_unicode_buffer(64)
            u.GetClassNameW(hwnd, name, 64)
            if name.value != TAO_TARGET:
                found.append(hwnd)
        return True
    u.EnumWindows(WNDENUMPROC(each), 0)
    return found


def ask_to_close(pids) -> bool:
    """What a click on its ✕ does (WM_CLOSE to its window only): it closes, or asks about unsaved changes itself."""
    windows = app_windows(pids)
    for hwnd in windows:
        user32().PostMessageW(hwnd, 0x0010, 0, 0)   # WM_CLOSE
    return bool(windows)


def bring_forward(pids):
    """Heirloom's window to the front (restored if minimised), so its changes can be saved."""
    u = user32()
    for hwnd in app_windows(pids):
        if u.IsIconic(hwnd):
            u.ShowWindow(hwnd, 9)   # SW_RESTORE
        u.SetForegroundWindow(hwnd)


def force_close(pids) -> bool:
    """„Zamknij mimo to”, after asking once more: ended at once, unsaved work lost (its WebView2 helpers end by
    themselves). Never /T: this installer can be Heirloom's child (--update, or opened from Heirloom), and the tree
    would take it along. True when it is gone within CLOSE_WAIT."""
    for pid in pids:
        run("taskkill", "/F", "/PID", str(pid))
    start = time.monotonic()
    while set(pids) & set(heirloom_pids() or []):
        if time.monotonic() - start > CLOSE_WAIT:
            return False
        time.sleep(0.3)
    for pid in pids:   # (Heirloom had no chance to remove them itself)
        if RUNNING is not None:
            (RUNNING / f"{pid}.json").unlink(missing_ok=True)
    return True


# ---------- steps ----------

class StillRunning(Exception):
    """Heirloom did not close within CLOSE_WAIT seconds, or is running again; nothing was changed."""


def close_app(log, each):
    """Waits until heirloom.exe is gone: an update (Heirloom has asked about unsaved changes and is exiting), or a
    Heirloom closed on the "running" page a moment ago."""
    if not app_running():
        return
    log("Czekam, aż Heirloom się zamknie…")
    start = time.monotonic()
    while time.monotonic() - start < CLOSE_WAIT:
        time.sleep(0.5)
        each((time.monotonic() - start) / CLOSE_WAIT)
        if not app_running():
            log("Heirloom jest zamknięty.")
            return
    raise StillRunning()


def exe_in_use() -> bool:
    """A running heirloom.exe can't be opened for writing (Windows keeps it mapped) - whoever started it. Also true
    for a moment after it exits (antivirus, WebView2 closing)."""
    try:
        with open(INSTALL_DIR / EXE_NAME, "r+b"):
            return False
    except FileNotFoundError:
        return False
    except OSError:
        return True


def until_free(log):
    """Checked right before the first file moves: no heirloom.exe running (StillRunning: back to the "running" page),
    and its exe not held (by a Heirloom the process list missed, or for a moment after it exited)."""
    for attempt in range(20):
        if app_running():
            raise StillRunning()
        if not exe_in_use():
            return
        if attempt == 0:
            log("Czekam, aż Windows zwolni plik heirloom.exe…")
        time.sleep(0.5)
    raise OSError(f"Plik {INSTALL_DIR / EXE_NAME} jest wciąż zajęty przez inny program (może antywirusowy).")


def move(source: Path, target: Path):
    """os.replace, tried again for a few seconds: right after Heirloom exits its exe can stay locked for a moment
    (antivirus, WebView2 closing)."""
    for attempt in range(10):
        try:
            os.replace(source, target)
            return
        except PermissionError:
            if attempt == 9:
                raise
            time.sleep(0.5)


class Copied:
    """What copy_files changed, so put_back can undo it: the files written, and the old files moved to BACKUP."""

    def __init__(self):
        self.written, self.moved, self.committed = [], [], False


def copy_files(log, each, copied: Copied):
    """each(fraction_done, name) for every megabyte copied (the progress bar)."""
    until_free(log)
    log("Kopiuję program…")
    if BACKUP.exists():   # left by an install that was cut off
        shutil.rmtree(BACKUP)
    INSTALL_DIR.mkdir(parents=True, exist_ok=True)
    uninstaller = INSTALL_DIR / UNINSTALLER
    copy_self = getattr(sys, "frozen", False) and Path(sys.executable).resolve() != uninstaller.resolve()
    with zipfile.ZipFile(payload()) as z:
        members = [m for m in z.infolist() if not m.is_dir()]
        # The files about to be replaced step aside first, so a failure further on can put them back.
        for name in [m.filename for m in members] + ([UNINSTALLER] if copy_self else []):
            old = INSTALL_DIR / name
            if old.is_file():
                (BACKUP / name).parent.mkdir(parents=True, exist_ok=True)
                move(old, BACKUP / name)
                copied.moved.append(name)
        total = sum(m.file_size for m in members) or 1
        done = 0
        for member in members:
            target = INSTALL_DIR / member.filename
            target.parent.mkdir(parents=True, exist_ok=True)
            copied.written.append(target)
            each(done / total, member.filename)
            with z.open(member) as source, open(target, "wb") as out:
                while True:
                    chunk = source.read(1 << 20)
                    if not chunk:
                        break
                    out.write(chunk)
                    done += len(chunk)
                    each(done / total, member.filename)
    if copy_self:
        copied.written.append(uninstaller)
        shutil.copy2(sys.executable, uninstaller)


def put_back(copied: Copied, log) -> str | None:
    """A step failed: the files this install wrote go, the ones it replaced come back (shortcuts too, for a first
    install). "restored" (the old version is in place), "removed" (it was a first install, nothing is left), "partial"
    or None (nothing had been changed yet)."""
    if copied.committed or not (copied.written or copied.moved):
        return None
    log("Przywracam poprzedni stan…")
    ok = True
    for path in reversed(copied.written):
        try:
            path.unlink(missing_ok=True)
        except OSError:
            ok = False
    for name in copied.moved:
        try:
            os.replace(BACKUP / name, INSTALL_DIR / name)
        except OSError:
            ok = False
    if not ok:
        log(f"Nie wszystko udało się przywrócić. Poprzednie pliki są w {BACKUP}.")
        return "partial"
    shutil.rmtree(BACKUP, ignore_errors=True)
    if copied.moved:
        log("Poprzednia wersja jest z powrotem na miejscu.")
        return "restored"
    for folder in (START_MENU, DESKTOP):
        if folder:
            (folder / SHORTCUT).unlink(missing_ok=True)
    try:
        INSTALL_DIR.rmdir()   # (only if empty)
    except OSError:
        pass
    log("Skopiowane pliki usunięte.")
    return "removed"


def shortcuts(log):
    """The paths go to PowerShell as environment variables, never inside its command text (an apostrophe in a user
    name would break the quoting)."""
    log("Dodaję skróty w menu Start i na pulpicie…")
    target = INSTALL_DIR / EXE_NAME
    made = 0
    for folder in (START_MENU, DESKTOP):
        if not folder:
            continue
        env = {**os.environ, "HL_LINK": str(folder / SHORTCUT), "HL_TARGET": str(target), "HL_DIR": str(INSTALL_DIR)}
        result = run("powershell", "-NoProfile", "-NonInteractive", "-Command",
                     "$s = (New-Object -ComObject WScript.Shell).CreateShortcut($env:HL_LINK); "
                     "$s.TargetPath = $env:HL_TARGET; $s.WorkingDirectory = $env:HL_DIR; "
                     "$s.IconLocation = $env:HL_TARGET + ',0'; $s.Description = 'Archiwum rodzinne'; $s.Save()",
                     env=env)
        if result.returncode == 0 and (folder / SHORTCUT).exists():
            made += 1
        else:
            log(f"Nie udało się dodać skrótu w {folder} — Heirloom znajdziesz w {INSTALL_DIR}.")
    if made:
        log("Dodano skróty w menu Start i na pulpicie.")


def uninstall_entry(log):
    log("Rejestruję Heirloom w „Aplikacjach i funkcjach”…")
    size_kb = sum(f.stat().st_size for f in INSTALL_DIR.rglob("*") if f.is_file()) // 1024
    with winreg.CreateKeyEx(winreg.HKEY_CURRENT_USER, UNINSTALL_KEY, 0, winreg.KEY_WRITE) as key:
        for name, value in {
            "DisplayName": APP,
            "DisplayVersion": VERSION,
            "Publisher": "Husarp",
            "DisplayIcon": str(INSTALL_DIR / EXE_NAME),
            "InstallLocation": str(INSTALL_DIR),
            "UninstallString": f'"{INSTALL_DIR / UNINSTALLER}" --uninstall',
        }.items():
            winreg.SetValueEx(key, name, 0, winreg.REG_SZ, value)
        for name, value in {"NoModify": 1, "NoRepair": 1, "EstimatedSize": size_kb}.items():
            winreg.SetValueEx(key, name, 0, winreg.REG_DWORD, value)


# Each step's share of the bar. Install: closing Heirloom, copying, shortcuts, registering, finishing.
INSTALL_WEIGHTS = (6, 70, 16, 4, 4)
# Uninstall: closing Heirloom, shortcuts, registry entry, own data (the Recycle Bin can take a while), finishing.
UNINSTALL_WEIGHTS = (10, 15, 10, 60, 5)


def install(log, at, file, copied: Copied):
    """at(step, fraction=0.0): which of INSTALL_WEIGHTS is running (step 0, closing Heirloom, is done by work)."""
    at(1)
    copy_files(log, lambda fraction, name: (at(1, fraction), file(name)), copied)
    at(2)
    shortcuts(log)
    at(3)
    uninstall_entry(log)
    copied.committed = True   # (the new version is in place: nothing to put back any more)
    shutil.rmtree(BACKUP, ignore_errors=True)
    at(4)
    log(f"Heirloom {VERSION} jest zainstalowany.")


class SHFILEOPSTRUCTW(ctypes.Structure):
    _fields_ = [("hwnd", wintypes.HWND), ("wFunc", wintypes.UINT), ("pFrom", wintypes.LPCWSTR), ("pTo", wintypes.LPCWSTR),
                ("fFlags", ctypes.c_ushort), ("fAnyOperationsAborted", wintypes.BOOL), ("hNameMappings", ctypes.c_void_p),
                ("lpszProgressTitle", wintypes.LPCWSTR)]


def to_recycle_bin(path: Path) -> bool:
    """Moves a folder to the Recycle Bin. A folder Windows can't put there (too big, the bin switched off) is not
    deleted silently: Windows asks first (FOF_WANTNUKEWARNING)."""
    FO_DELETE, FOF_SILENT, FOF_NOCONFIRMATION, FOF_ALLOWUNDO, FOF_WANTNUKEWARNING = 3, 0x4, 0x10, 0x40, 0x4000
    # The list of paths ends with two nulls.
    names = ctypes.create_unicode_buffer(str(path) + "\0\0")
    op = SHFILEOPSTRUCTW(None, FO_DELETE, ctypes.cast(names, wintypes.LPCWSTR), None,
                         FOF_SILENT | FOF_NOCONFIRMATION | FOF_ALLOWUNDO | FOF_WANTNUKEWARNING, False, None, None)
    result = ctypes.windll.shell32.SHFileOperationW(ctypes.byref(op))
    return result == 0 and not op.fAnyOperationsAborted and not path.exists()


def uninstall(log, remove_own_data: bool, at) -> list:
    """Returns the folders of the program's own data that stayed on the disk (not taken by the Recycle Bin)."""
    at(1)
    log("Usuwam skróty…")
    for folder in (START_MENU, DESKTOP):
        if folder:
            (folder / SHORTCUT).unlink(missing_ok=True)
    at(2)
    log("Usuwam wpis z „Aplikacji i funkcji”…")
    try:
        winreg.DeleteKey(winreg.HKEY_CURRENT_USER, UNINSTALL_KEY)
    except OSError:
        pass
    at(3)
    left = []
    if remove_own_data:
        log("Przenoszę ustawienia programu i miniatury do Kosza…")
        for folder in OWN_DATA:
            if folder.exists():
                if to_recycle_bin(folder):
                    log(f"Do Kosza: {folder}")
                else:
                    log(f"Zostaje na dysku (nie trafiło do Kosza): {folder}")
                    left.append(folder)
    else:
        log("Ustawienia programu zostają — po ponownej instalacji wszystko będzie jak przedtem.")
    at(4)
    log("Archiwa rodzinne zostają nietknięte.")
    log("Heirloom jest odinstalowany. Folder programu zniknie po zamknięciu tego okna.")
    return left


def remove_program_folder(folder: Path):
    """The uninstaller runs FROM the program folder, so it goes a moment after we quit: our two files, then the
    folder itself only if it is empty (rmdir without /s never takes anything else with it)."""
    ping = SYSTEM32 / "PING.EXE"
    inner = (f'"{ping}" 127.0.0.1 -n 3 >nul & del /f /q "{folder / EXE_NAME}" & del /f /q "{folder / UNINSTALLER}" '
             f'& rmdir "{folder}"')
    # The whole command in one more pair of quotes: cmd /c drops the first and the last quote of what follows.
    subprocess.Popen(f'"{tool("cmd")}" /d /c "{inner}"', creationflags=NO_WINDOW, cwd=str(SYSTEM32))


def launch_app():
    subprocess.Popen([tool("explorer.exe"), str(INSTALL_DIR / EXE_NAME)], creationflags=NO_WINDOW)


def overall(weights, step: int, fraction: float = 0.0) -> float:
    """How far the bar is (0-1) at `fraction` of step `step` (0-based; len(weights) = all done)."""
    step = max(0, min(step, len(weights)))
    part = weights[step] * min(max(fraction, 0.0), 1.0) if step < len(weights) else 0
    return (sum(weights[:step]) + part) / sum(weights)


def work(mode: str, post, remove_own_data: bool = False):
    """The worker thread: installs or uninstalls and reports through post(kind, value) - never touches Tk.
    Kinds: "log" (a step), "file" (the file being copied), "progress" ((value, end of this step), 0-1), then
    "done" (("installed" | "uninstalled", folders left on the disk)) or "failed" (("running" | "error", message,
    what put_back did))."""
    weights = UNINSTALL_WEIGHTS if mode == "uninstall" else INSTALL_WEIGHTS

    def at(step, fraction=0.0):
        post("progress", (overall(weights, step, fraction), overall(weights, step + 1)))

    def log(text):
        post("log", text)

    copied = Copied()
    try:
        at(0)
        close_app(log, lambda fraction: at(0, fraction))
        if mode == "uninstall":
            left = uninstall(log, remove_own_data, at)
            at(len(weights))
            post("done", ("uninstalled", left))
            return
        install(log, at, lambda name: post("file", name), copied)
        if mode == "update":   # Heirloom closed itself for the update: bring it back
            log("Uruchamiam Heirloom…")
            try:
                launch_app()
            except OSError as e:
                log(f"Nie udało się uruchomić Heirloom ({e}). Otwórz go z menu Start.")
        at(len(weights))
        post("done", ("installed", []))
    except StillRunning:
        log(f"Heirloom wciąż jest otwarty po {CLOSE_WAIT} s. Nic nie zostało zmienione.")
        post("failed", ("running", "", None))   # (nothing was changed: back to the "running" page)
    except Exception as e:      # show it rather than vanish
        log(f"Coś poszło nie tak: {e}")
        undone = None
        if mode != "uninstall":
            try:
                undone = put_back(copied, log)
            except Exception as again:
                log(f"Przywracanie też się nie udało: {again}")
                undone = "partial"
        post("failed", ("error", str(e) or type(e).__name__, undone))


def mode_of(argv) -> str:
    """"uninstall" (from Apps & features), "update" (started by Heirloom itself: no questions) or "install"."""
    args = {a.lower() for a in argv[1:]}
    return "uninstall" if "--uninstall" in args else "update" if "--update" in args else "install"


def ease(shown: float, target: float, end: float) -> float:
    """The bar's next frame: glide up to `target`; while a step reports nothing, creep towards (never past) 90% of the
    way to the end of that step, so a long wait doesn't look frozen. Never goes back."""
    if shown < target - 0.0005:
        return min(target, shown + max((target - shown) * 0.22, 0.002))
    goal = target + (end - target) * 0.9
    return shown + (goal - shown) * 0.004 if goal > shown else shown


# ---------- look: Heirloom's light theme (src/styles/tokens.css) ----------

BG, SURFACE, SURFACE2, HEADER = "#f7f4ee", "#ffffff", "#efebe3", "#efeae1"
BORDER, LINE, TRACK = "#dfd8cc", "#857c6f", "#e4ddd1"
TEXT, TEXT2, TEXT3 = "#1f1c18", "#554e45", "#6b6256"
ACCENT, ACCENT_SOFT = "#2f5d50", "#e1eae5"
ERR, ERR_SOFT, WARN, WARN_SOFT = "#a3392b", "#f6e3df", "#7a5d0e", "#f4ecd3"
# The app's Newsreader and IBM Plex Sans come as web fonts only, which Windows can't load: Georgia and Segoe UI.
FAMILIES = {"serif": ("Newsreader", "Georgia", "Cambria", "Noto Serif", "DejaVu Serif"),
            "sans": ("IBM Plex Sans", "Segoe UI", "Noto Sans", "DejaVu Sans"),
            "mono": ("Cascadia Mono", "Consolas", "DejaVu Sans Mono")}
LOGOS = {44: "Square44x44Logo.png", 64: "64x64.png", 71: "Square71x71Logo.png", 89: "Square89x89Logo.png"}


def asset(name: str) -> Path:
    """The logo pictures: bundled under setup\\ (scripts/build.ps1), or straight from src-tauri\\icons."""
    if hasattr(sys, "_MEIPASS"):
        return Path(sys._MEIPASS) / "setup" / name
    return Path(__file__).resolve().parents[1] / "src-tauri" / "icons" / name


def _rgb(colour: str):
    return tuple(int(colour[i:i + 2], 16) for i in (1, 3, 5))


def mix(a: str, b: str, t: float) -> str:
    return "#%02x%02x%02x" % tuple(round(x + (y - x) * t) for x, y in zip(_rgb(a), _rgb(b)))


def rrect(x0, y0, x1, y1, r):
    """Signed distance to a rounded rectangle (< 0 inside)."""
    cx, cy, hx, hy = (x0 + x1) / 2, (y0 + y1) / 2, (x1 - x0) / 2 - r, (y1 - y0) / 2 - r

    def d(x, y):
        qx, qy = abs(x - cx) - hx, abs(y - cy) - hy
        return math.hypot(max(qx, 0), max(qy, 0)) + min(max(qx, qy), 0) - r
    return d


def circle(cx, cy, r):
    return lambda x, y: math.hypot(x - cx, y - cy) - r


def strokes(points, width):
    """A line through `points`, `width` thick, with round ends."""
    segs = list(zip(points, points[1:]))

    def d(x, y):
        best = 1e9
        for (ax, ay), (bx, by) in segs:
            vx, vy = bx - ax, by - ay
            t = max(0.0, min(1.0, ((x - ax) * vx + (y - ay) * vy) / ((vx * vx + vy * vy) or 1)))
            best = min(best, math.hypot(x - ax - vx * t, y - ay - vy * t))
        return best - width / 2
    return d


_SUB = [(i + 0.5) / 4 - 0.5 for i in range(4)]


def raster(w: int, h: int, bg: str, layers, x0: int = 0) -> list:
    """A w x h picture as rows of "#rrggbb": each (colour, distance) layer painted over `bg` in order, with smooth
    edges. Tk draws curves without antialiasing, so the round things (buttons, the tick box, the bar, the badges) are
    pictures made here. x0: start at that column (a slice)."""
    base = _rgb(bg)
    layers = [(_rgb(c), d) for c, d in layers]
    rows = []
    for y in range(h):
        row = []
        for x in range(x0, x0 + w):
            px, py = x + 0.5, y + 0.5
            r, g, b = base
            for (cr, cg, cb), d in layers:
                dist = d(px, py)
                if dist >= 0.71:
                    continue
                a = 1.0 if dist <= -0.71 else sum(d(px + sx, py + sy) < 0 for sx in _SUB for sy in _SUB) / 16
                r, g, b = r + (cr - r) * a, g + (cg - g) * a, b + (cb - b) * a
            row.append("#%02x%02x%02x" % (round(r), round(g), round(b)))
        rows.append(row)
    return rows


def bar_rows(w: int, h: int, filled: int, fill: str, bg: str) -> list:
    """The progress bar: a rounded track with `filled` pixels of it in `fill`. Only the columns around the round ends
    are worked out by raster(); between them every row is one colour (it is redrawn while it moves)."""
    layers = [(TRACK, rrect(0, 0, w, h, h / 2))] + ([(fill, rrect(0, 0, filled, h, h / 2))] if filled else [])
    k = math.ceil(h / 2) + 1
    edges = set(range(0, k)) | set(range(w - k, w)) | (set(range(filled - k, filled + k)) if filled else set())
    plain = {x: (fill if x < filled else TRACK) for x in range(w) if x not in edges}
    rows = [[] for _ in range(h)]
    x = 0
    while x < w:
        start = x
        if x in plain:
            while x < w and x in plain and plain[x] == plain[start]:
                x += 1
            for row in rows:
                row += [plain[start]] * (x - start)
        else:
            while x < w and x not in plain:
                x += 1
            for row, part in zip(rows, raster(x - start, h, bg, layers, x0=start)):
                row += part
    return rows


def photo(rows) -> tk.PhotoImage:
    image = tk.PhotoImage(width=len(rows[0]), height=len(rows))
    image.put(" ".join("{" + " ".join(row) + "}" for row in rows))
    return image


# ---------- window ----------

class Button(tk.Label):
    """A button like the app's (.btn): primary (accent), danger (red) or secondary (white with a line)."""

    def __init__(self, ui, parent, text, command, kind="secondary"):
        font = ui.font("semi", 13)
        w, h, r = max(ui.px(96), font.measure(text) + ui.px(32)), ui.px(36), ui.px(6)
        bg = parent["bg"]
        if kind == "secondary":
            ring = ui.px(1.5)

            def look(fill):
                return [(LINE, rrect(0, 0, w, h, r)), (fill, rrect(ring, ring, w - ring, h - ring, r - ring))]
            looks = {"normal": look(SURFACE), "hover": look(SURFACE2), "disabled": look(mix(SURFACE, BG, 0.5))}
            self.fg = TEXT
        else:
            fill = ERR if kind == "danger" else ACCENT
            looks = {state: [(c, rrect(0, 0, w, h, r))] for state, c in
                     (("normal", fill), ("hover", mix(fill, TEXT, 0.12)), ("disabled", mix(fill, BG, 0.55)))}
            self.fg = "#ffffff"
        self.images = {state: photo(raster(w, h, bg, layers)) for state, layers in looks.items()}
        super().__init__(parent, image=self.images["normal"], text=text, compound="center", font=font, fg=self.fg,
                         bg=bg, bd=0, padx=0, pady=0, highlightthickness=0, cursor="hand2")
        self.command, self.enabled = command, True
        self.bind("<Enter>", lambda e: self.enabled and self.configure(image=self.images["hover"]))
        self.bind("<Leave>", lambda e: self.enabled and self.configure(image=self.images["normal"]))
        self.bind("<ButtonRelease-1>", lambda e: self.invoke())

    def invoke(self):
        if self.enabled:
            self.command()

    def enable(self, on: bool):
        self.enabled = on
        self.configure(image=self.images["normal" if on else "disabled"],
                       fg=self.fg if on else mix(self.fg, BG, 0.45), cursor="hand2" if on else "arrow")


class Check(tk.Frame):
    """A tick box like the app's: accent green with a white tick when on."""

    def __init__(self, ui, parent, text, variable, wrap):
        super().__init__(parent, bg=parent["bg"])
        n, bg, ring = ui.px(18), parent["bg"], ui.px(1.5)
        box = rrect(0, 0, n, n, ui.px(4))
        tick = strokes([(n * 0.26, n * 0.52), (n * 0.43, n * 0.68), (n * 0.74, n * 0.33)], ui.px(2.2))
        self.images = {True: photo(raster(n, n, bg, [(ACCENT, box), ("#ffffff", tick)])),
                       False: photo(raster(n, n, bg, [(LINE, box), (SURFACE, rrect(ring, ring, n - ring, n - ring,
                                                                                    ui.px(4) - ring))]))}
        self.variable = variable
        self.box = tk.Label(self, bg=bg, bd=0, cursor="hand2")
        self.box.pack(side="left", anchor="n", pady=(ui.px(1), 0))
        self.text = tk.Label(self, text=text, bg=bg, fg=TEXT, font=ui.font("sans", 13), cursor="hand2",
                             justify="left", anchor="w", wraplength=wrap)
        self.text.pack(side="left", padx=(ui.px(10), 0))
        for widget in (self, self.box, self.text):
            widget.bind("<ButtonRelease-1>", lambda e: self.toggle())
        self.show()

    def toggle(self):
        self.variable.set(not self.variable.get())
        self.show()

    def show(self):
        self.box.configure(image=self.images[bool(self.variable.get())])


class SetupWindow(tk.Tk):
    W, H = 600, 440   # (at 100% - everything is scaled by px(); the height grows when a page needs more)

    def __init__(self, mode: str):
        super().__init__()
        self.withdraw()
        self.mode = mode
        self.uninstalling = mode == "uninstall"
        self.s = max(1.0, float(self.tk.call("tk", "scaling")) * 72 / 96)   # Windows display scaling
        self.current = installed_version()
        # A real update (an older version is installed), not a first install, a reinstall or a downgrade.
        self.upgrade = mode == "update" or (self.current is not None and version_key(self.current) < version_key(VERSION))
        self.events = queue.Queue()  # from the worker threads; read on the Tk thread only (_drain)
        self.page = None
        self.busy = False            # working: the window can't be closed
        self.done = False
        self.outcome = None
        self.was_running = False     # Heirloom was open and closed for this install: started again at the end
        self.start_old = False       # a failed update put the old version back: start it when the window closes
        # The "running" page: what look() found, „Czekaj” (or OK) clicked, „Zamknij mimo to” being confirmed, the
        # force close under way ("closing") or failed ("stuck"), the pids already asked to close, and which watcher
        # thread's news counts (a new page stops the old one).
        self.found = None
        self.waiting = self.confirming = self.closing = self.stuck = False
        self.asked = set()
        self.watch = 0
        self.remove_own_data = tk.BooleanVar(value=False)
        self.run_app = tk.BooleanVar(value=True)
        self.shown, self.target, self.end = 0.0, 0.0, 0.0   # the bar: drawn / reported / end of this step
        self.bar_colour = ACCENT
        self.bar_px = None
        self.details_open = False
        self.lines = []              # the log, kept so the details box can be (re)built at any time
        self.last_file = None
        self._keep = []              # PhotoImages of the page shown (Tk drops an image nobody refers to)
        self._fonts = {}
        families = set(tkfont.families(self))
        pick = {kind: next((f for f in names if f in families), names[-1]) for kind, names in FAMILIES.items()}
        self.families = {"serif": (pick["serif"], "normal"), "sans": (pick["sans"], "normal"),
                         "semi": ("Segoe UI Semibold", "normal") if pick["sans"] == "Segoe UI" and
                         "Segoe UI Semibold" in families else (pick["sans"], "bold"),
                         "mono": (pick["mono"], "normal")}

        self.title("Odinstaluj Heirloom" if self.uninstalling else
                   "Heirloom — aktualizacja" if self.upgrade else "Heirloom — instalacja")
        self.configure(bg=BG)
        self.resizable(False, False)
        w, h = self.px(self.W), self.px(self.H)
        self.geometry(f"{w}x{h}+{(self.winfo_screenwidth() - w) // 2}+{(self.winfo_screenheight() - h) // 3}")
        self._icon()

        header = tk.Frame(self, bg=HEADER, height=self.px(76))
        header.pack(fill="x")
        header.pack_propagate(False)
        self.logo = self._logo()
        if self.logo:
            tk.Label(header, image=self.logo, bg=HEADER, bd=0).pack(side="left", padx=(self.px(26), self.px(14)))
        words = tk.Frame(header, bg=HEADER)
        words.pack(side="left", padx=(0 if self.logo else self.px(28), 0))
        tk.Label(words, text=APP, font=self.font("serif", 23), fg=TEXT, bg=HEADER).pack(anchor="w")
        tk.Label(words, text="Archiwum rodzinne", font=self.font("sans", 12), fg=TEXT3, bg=HEADER).pack(anchor="w")
        tk.Frame(self, bg=BORDER, height=self.px(1)).pack(fill="x")
        self.footer = tk.Frame(self, bg=BG, height=self.px(68))
        self.footer.pack(side="bottom", fill="x")
        self.footer.pack_propagate(False)
        tk.Frame(self, bg=BORDER, height=self.px(1)).pack(side="bottom", fill="x")
        self.body = tk.Frame(self, bg=BG, padx=self.px(28), pady=self.px(22))
        self.body.pack(fill="both", expand=True)

        self.protocol("WM_DELETE_WINDOW", self._close)
        self.bind("<Return>", lambda e: self.primary and self.primary.invoke())
        self.bind("<Escape>", lambda e: self.secondary and self.secondary.invoke())
        self.primary = self.secondary = None
        if mode == "update":   # Heirloom has closed itself (after asking about unsaved changes): no questions
            self._progress()
        else:
            self.show("welcome")
        self.deiconify()
        self.lift()
        self.focus_force()
        self.after(30, self._drain)

    # ----- helpers -----

    def px(self, n: float) -> int:
        return int(round(n * self.s))

    def font(self, kind: str, size: int) -> tkfont.Font:
        key = (kind, size)
        if key not in self._fonts:
            family, weight = self.families[kind]
            self._fonts[key] = tkfont.Font(self, family=family, size=-self.px(size), weight=weight)
        return self._fonts[key]

    def keep(self, image):
        self._keep.append(image)
        return image

    def _icon(self):
        try:
            if sys.platform == "win32":
                frozen = Path(getattr(sys, "_MEIPASS", "")) / "heirloom.ico"
                self.iconbitmap(str(frozen if hasattr(sys, "_MEIPASS") else asset("icon.ico")))
            else:
                self.iconphoto(True, tk.PhotoImage(file=str(asset(LOGOS[64]))))
        except (tk.TclError, OSError):
            pass

    def _logo(self):
        best = min(LOGOS, key=lambda n: abs(n - 44 * self.s))
        try:
            return tk.PhotoImage(file=str(asset(LOGOS[best])))
        except (tk.TclError, OSError):
            return None

    def _fit(self):
        """Grows the window when the page needs more room (text is never cut off), back to the usual size after."""
        self.update_idletasks()
        need = max(self.px(self.H), self.winfo_reqheight())
        if need != self.winfo_height():
            self.geometry(f"{self.px(self.W)}x{need}")

    def label(self, parent, text, kind="sans", size=13, fg=TEXT, **kw):
        return tk.Label(parent, text=text, font=self.font(kind, size), fg=fg, bg=parent["bg"], justify="left",
                        anchor="w", **kw)

    def wrap(self, less=0):
        return self.px(self.W - 56 - less)

    def badge(self, parent, colour, soft, kind):
        """A round tinted sign: a tick (done) or "!" (look here)."""
        n = self.px(44)
        layers = [(soft, circle(n / 2, n / 2, n / 2))]
        if kind == "tick":
            layers.append((colour, strokes([(n * .31, n * .52), (n * .45, n * .65), (n * .70, n * .37)], self.px(3.2))))
        else:
            layers += [(colour, strokes([(n / 2, n * .28), (n / 2, n * .55)], self.px(3.4))),
                       (colour, circle(n / 2, n * .71, self.px(2.2)))]
        return tk.Label(parent, image=self.keep(photo(raster(n, n, parent["bg"], layers))), bg=parent["bg"], bd=0)

    def chip(self, parent, text, fill, fg):
        font = self.font("semi", 13)
        w, h = font.measure(text) + self.px(22), self.px(26)
        image = self.keep(photo(raster(w, h, parent["bg"], [(fill, rrect(0, 0, w, h, h / 2))])))
        return tk.Label(parent, image=image, text=text, compound="center", font=font, fg=fg, bg=parent["bg"], bd=0)

    def note(self, parent, text, fg, bg):
        """A tinted box for a warning in a page."""
        box = tk.Frame(parent, bg=bg, padx=self.px(14), pady=self.px(10))
        self.label(box, text, size=13, fg=fg, wraplength=self.wrap(28)).pack(anchor="w")
        return box

    def buttons(self, *specs):
        """Footer buttons, right to left: (text, command, kind). Enter = the first one, Esc = a secondary one."""
        for child in self.footer.winfo_children():
            child.destroy()
        self.primary = self.secondary = None
        for i, (text, command, kind) in enumerate(specs):
            button = Button(self, self.footer, text, command, kind)
            button.pack(side="right", padx=(0, self.px(28) if i == 0 else self.px(10)))
            if i == 0:
                self.primary = button
            elif kind == "secondary":
                self.secondary = button
        return self.primary

    # ----- pages -----

    def show(self, page: str):
        self.page = page
        self.watch += 1   # (stops the "running" page's watcher; that page starts a new one)
        for child in self.body.winfo_children():
            child.destroy()
        self._keep = []
        getattr(self, "_page_" + page)()
        self._fit()

    def _go(self):
        """Welcome answered (or „Spróbuj ponownie”, or Heirloom still open after CLOSE_WAIT): is Heirloom open, and
        with unsaved work? (asked off the Tk thread) - then on."""
        if self.primary:
            self.primary.enable(False)
        threading.Thread(target=lambda: self.events.put(("running", look())), daemon=True).start()

    def _on_running(self, found: dict):
        if found["kind"] == "none":
            self._progress()
            return
        self.found, self.asked = found, set()
        self.waiting = self.confirming = self.closing = self.stuck = False
        self.was_running = True
        self.show("running")

    def _watch(self):
        """Looks at Heirloom every WATCH_EVERY s while the "running" page is shown (a thread: tasklist takes a moment)."""
        self.watch += 1
        mine = self.watch

        def loop():
            while self.watch == mine:
                time.sleep(WATCH_EVERY)
                if self.watch == mine:
                    self.events.put(("look", (mine, look())))
        threading.Thread(target=loop, daemon=True).start()

    @staticmethod
    def _what(found):
        """What the "running" page shows of look()'s answer (a new answer redraws it only when this differs)."""
        return found["kind"], found["unsaved"], found["draft"], tuple(found["archives"])

    def _on_look(self, news):
        mine, found = news
        if mine != self.watch or self.page != "running":
            return
        if found["kind"] == "none":   # closed (saved and closed by hand, closed by OK, or ended): on with the install
            self._progress()
            return
        before, self.found = self.found, found
        if found["kind"] == "unsaved":   # (asked before, but kept open with new changes: asked again once saved)
            self.asked -= set(found["pids"])
        if self.waiting and found["kind"] == "clean" and set(found["pids"]) - self.asked:
            # Saved in Heirloom after „Czekaj” (or nothing to save after OK): closed as its ✕ would.
            self._ask(found["pids"])
        if self._what(found) != self._what(before):
            self.show("running")
        elif self.r_status_text and (self.waiting or self.closing) and not (self.confirming or self.stuck):
            self.r_dots = (self.r_dots + 1) % 4
            self.r_status.configure(text=self.r_status_text + "." * self.r_dots)

    def _ask(self, pids):
        """OK (nothing unsaved, or an older Heirloom that asks itself), or saved after „Czekaj”: closed the way its ✕
        does. An older Heirloom may ask about its changes, so its window comes to the front."""
        self.asked |= set(pids)
        front = self.found["kind"] == "unknown"

        def ask():
            try:
                ask_to_close(pids)
                if front:
                    bring_forward(pids)
            except Exception:
                pass
        threading.Thread(target=ask, daemon=True).start()

    def _ok(self):
        self.waiting = True
        self._ask(self.found["pids"])
        self.show("running")

    def _wait(self):
        """„Czekaj”: Heirloom to the front, to save there; the watcher goes on by itself."""
        self.waiting = True
        pids = self.found["pids"]
        threading.Thread(target=lambda: self._quietly(bring_forward, pids), daemon=True).start()
        self.show("running")

    @staticmethod
    def _quietly(action, *args):
        try:
            action(*args)
        except Exception:
            pass

    def _confirm(self, on: bool):
        self.confirming = on
        self.show("running")

    def _force(self):
        """„Zamknij bez zapisu”, confirmed: taskkill /F. The watcher sees it gone and the install goes on."""
        self.confirming, self.closing, self.stuck = False, True, False
        pids = self.found["pids"]

        def kill():
            try:
                gone = force_close(pids)
            except Exception:
                gone = False
            if not gone:
                self.events.put(("stuck", None))
        threading.Thread(target=kill, daemon=True).start()
        self.show("running")

    def _on_stuck(self, _):
        if self.page == "running":
            self.closing, self.stuck = False, True
            self.show("running")

    def _page_welcome(self):
        b = self.body
        newer = self.current is not None and version_key(self.current) > version_key(VERSION)
        chips = None
        if INSTALL_DIR is None:
            head = "Nie da się zainstalować Heirloom"
            points = ["Windows nie podał folderu na programy tego konta (AppData\\Local)."]
        elif self.uninstalling:
            head = "Odinstaluj Heirloom"
            points = ["Usuwa program, skróty i wpis w „Aplikacjach i funkcjach”.",
                      "Archiwa rodzinne (Twoje foldery z plikiem .ged i zdjęciami) zostają zawsze.",
                      "Ustawienia programu i miniatury zostają, chyba że zaznaczysz pole poniżej."]
        elif self.current == VERSION:
            head = f"Heirloom {VERSION} jest już zainstalowany"
            points = ["To ta sama wersja, nie aktualizacja. Ponowna instalacja niczego nie psuje i naprawia "
                      "uszkodzony program.",
                      "Archiwa rodzinne i ustawienia zostają bez zmian."]
        elif newer:
            head, chips = "Zainstalowana jest nowsza wersja", (WARN_SOFT, WARN)
            points = [f"Ten instalator zamieni Heirloom {self.current} na starszą wersję {VERSION}.",
                      "Archiwa rodzinne i ustawienia zostają bez zmian."]
        elif self.current:
            head, chips = "Aktualizuj Heirloom", (ACCENT_SOFT, ACCENT)
            points = [f"Nowa wersja zastąpi program w {INSTALL_DIR}.",
                      "Archiwa rodzinne i ustawienia zostają bez zmian."]
        else:
            head = f"Zainstaluj Heirloom {VERSION}"
            points = [f"Program trafi do {INSTALL_DIR} — tylko dla Twojego konta, bez uprawnień administratora.",
                      "W menu Start i na pulpicie pojawi się skrót.",
                      "Archiwa rodzinne to Twoje własne foldery — instalator nigdy ich nie dotyka."]
        self.label(b, head, "serif", 22).pack(anchor="w")
        if chips:
            row = tk.Frame(b, bg=BG)
            row.pack(anchor="w", pady=(self.px(10), 0))
            self.chip(row, self.current, SURFACE2, TEXT2).pack(side="left")
            self.label(row, "→", "semi", 15, TEXT3).pack(side="left", padx=self.px(8))
            self.chip(row, VERSION, chips[0], chips[1]).pack(side="left")
        listing = tk.Frame(b, bg=BG)
        listing.pack(anchor="w", fill="x", pady=(self.px(14), 0))
        n = self.px(6)
        dot = self.keep(photo(raster(n, n, BG, [(ACCENT, circle(n / 2, n / 2, n / 2))])))
        for point in points:
            row = tk.Frame(listing, bg=BG)
            row.pack(anchor="w", fill="x", pady=self.px(3))
            tk.Label(row, image=dot, bg=BG, bd=0).pack(side="left", anchor="n", pady=(self.px(8), 0))
            self.label(row, point, fg=TEXT2, wraplength=self.wrap(18)).pack(side="left", padx=(self.px(12), 0))
        if INSTALL_DIR is None:
            self.buttons(("Zamknij", self._close, "primary"))
            return
        if not self.uninstalling and not has_webview2():
            self.note(b, "Brakuje składnika Microsoft Edge WebView2, którego potrzebuje okno programu. Zainstaluj go "
                         "z witryny Microsoftu („WebView2 Runtime”), a potem uruchom Heirloom.",
                      WARN, WARN_SOFT).pack(fill="x", pady=(self.px(16), 0))
        if self.uninstalling:
            Check(self, b, "Usuń też moje dane (ustawienia programu i miniatury trafią do Kosza)",
                  self.remove_own_data, self.wrap(28)).pack(anchor="w", pady=(self.px(18), 0))
        go = ("Odinstaluj" if self.uninstalling else "Zainstaluj ponownie" if self.current == VERSION
              else "Zainstaluj starszą wersję" if newer else "Aktualizuj" if self.current else "Zainstaluj")
        self.buttons((go, self._go, "danger" if self.uninstalling else "primary"), ("Anuluj", self._close, "secondary"))

    def _page_running(self):
        """APP-STANDARDS section 5: say it plainly. Nothing unsaved (or an older Heirloom that asks itself): OK closes it
        the way its own ✕ does. Unsaved work: „Czekaj” / „Zamknij mimo to” (asked once more) / „Anuluj …”."""
        f = self.found
        verb = ("odinstalować" if self.uninstalling else "zaktualizować" if self.upgrade
                else "zainstalować ponownie" if self.current == VERSION else "zainstalować")
        then = "" if self.uninstalling else ", a potem uruchomiony ponownie"
        cancel = ("Anuluj odinstalowanie" if self.uninstalling else "Anuluj aktualizację" if self.upgrade
                  else "Anuluj instalację")
        where = ("w otwartym archiwum" if not f["archives"] else f"w archiwum „{f['archives'][0]}”"
                 if len(f["archives"]) == 1 else "w archiwach " + ", ".join(f"„{a}”" for a in f["archives"]))
        draft = "zmiany w otwartej sekcji, niezatwierdzone przyciskiem „Gotowe”"
        hint_stuck = "Gdyby Heirloom nie odpowiadał, „Zamknij mimo to” zamknie go od razu."
        if f["kind"] == "unsaved":
            n = f["unsaved"]
            changes = count_pl(n, "niezapisana zmiana", "niezapisane zmiany", "niezapisanych zmian")
            title = "Heirloom ma niezapisane zmiany"
            if n:
                said = (f"{where[0].upper() + where[1:]} {'są' if plural(n) == 1 else 'jest'} {changes}"
                        + (f", a do tego {draft}." if f["draft"] else "."))
                lose = (f"{'Przepadną' if f['draft'] or plural(n) == 1 else 'Przepadnie'} {changes} {where}"
                        + (f" i {draft}." if f["draft"] else "."))
            else:
                said = f"{where[0].upper() + where[1:]} są {draft}."
                lose = f"Przepadną {draft}."
            todo = ("" if self.waiting else "Żeby ich nie stracić, kliknij „Czekaj”, przejdź do Heirloom i "
                    + ("zatwierdź sekcję, a potem " if f["draft"] else "") + "zapisz zmiany. Instalator poczeka, "
                    "zamknie Heirloom i sam ruszy dalej.")
            waiting = "Czekam, aż zapiszesz zmiany albo zamkniesz Heirloom"
            hint = "Gdy w Heirloom nic nie będzie czekało na zapis, instalator sam go zamknie i ruszy dalej."
            ask = "Zamknąć Heirloom bez zapisu?"
        elif f["kind"] == "unknown":
            title = "Heirloom jest uruchomiony"
            said = f"Zostanie zamknięty, żeby go {verb}{then}."
            todo = ("" if self.waiting else "Ta wersja Heirloom nie mówi instalatorowi, czy ma niezapisane zmiany — "
                    "jeśli ma, po „OK” zapyta o nie w swoim oknie.")
            waiting = "Czekam, aż Heirloom się zamknie"
            hint = "Jeśli pyta o niezapisane zmiany, odpowiedz w jego oknie. " + hint_stuck
            ask, lose = "Zamknąć Heirloom od razu?", "Jeśli ma niezapisane zmiany, przepadną."
        else:
            title = "Heirloom jest uruchomiony"
            said, todo = f"Zostanie zamknięty, żeby go {verb}{then}.", "Nie ma niezapisanych zmian, więc nic nie przepadnie."
            waiting, hint = "Zamykam Heirloom", hint_stuck
            ask, lose = "Zamknąć Heirloom od razu?", "Nie ma niezapisanych zmian, więc nic nie przepadnie."
        card = tk.Frame(self.body, bg=SURFACE, highlightthickness=self.px(1), highlightbackground=BORDER,
                        padx=self.px(20), pady=self.px(20))
        card.pack(fill="x", pady=(self.px(10), 0))
        self.badge(card, WARN, WARN_SOFT, "!").pack(side="left", anchor="n")
        words = tk.Frame(card, bg=SURFACE)
        words.pack(side="left", fill="x", expand=True, padx=(self.px(16), 0))
        self.label(words, title, "serif", 19).pack(anchor="w")
        self.label(words, said, wraplength=self.wrap(100)).pack(anchor="w", pady=(self.px(8), 0))
        if todo:
            self.label(words, todo, fg=TEXT2, wraplength=self.wrap(100)).pack(anchor="w", pady=(self.px(6), 0))
        self.r_dots, self.r_status_text = 0, ""
        self.r_status = self.label(self.body, "", "semi", 13, ACCENT, wraplength=self.wrap())
        r_hint = self.label(self.body, "", size=12, fg=TEXT3, wraplength=self.wrap())
        if self.confirming:
            box = tk.Frame(self.body, bg=ERR_SOFT, padx=self.px(14), pady=self.px(10))
            box.pack(fill="x", pady=(self.px(14), 0))
            self.label(box, ask, "semi", 13, ERR, wraplength=self.wrap(28)).pack(anchor="w")
            self.label(box, lose, size=13, fg=TEXT, wraplength=self.wrap(28)).pack(anchor="w", pady=(self.px(2), 0))
            self.buttons(("Zamknij bez zapisu" if f["kind"] == "unsaved" else "Zamknij od razu", self._force, "danger"),
                         ("Wróć", lambda: self._confirm(False), "secondary"))
            self.primary = None   # (Enter never closes it by force: that takes a click)
        elif self.closing or self.stuck:
            self.r_status_text = "Zamykam Heirloom" if self.closing else "Nie udało się zamknąć Heirloom"
            self.r_status.configure(text=self.r_status_text, fg=ACCENT if self.closing else ERR)
            if self.stuck:
                r_hint.configure(text="Zamknij go w Menedżerze zadań — instalator ruszy wtedy sam.")
            self.buttons(("Czekam…", None, "primary"), (cancel, self._close, "secondary"))
            self.primary.enable(False)
            if self.closing:
                self.secondary.enable(False)
        elif self.waiting:
            closing = f["kind"] == "clean" and set(f["pids"]) <= self.asked
            self.r_status_text = "Zamykam Heirloom" if closing else waiting
            self.r_status.configure(text=self.r_status_text)
            r_hint.configure(text=hint_stuck if closing else hint)
            self.buttons(("Czekam…", None, "primary"), ("Zamknij mimo to", lambda: self._confirm(True), "danger"),
                         (cancel, self._close, "secondary"))
            self.primary.enable(False)
        elif f["kind"] == "unsaved":
            self.buttons(("Czekaj", self._wait, "primary"), ("Zamknij mimo to", lambda: self._confirm(True), "danger"),
                         (cancel, self._close, "secondary"))
        else:
            self.buttons(("OK", self._ok, "primary"), (cancel, self._close, "secondary"))
        if self.r_status_text:
            self.r_status.pack(anchor="w", pady=(self.px(18), 0))
        if r_hint["text"]:
            r_hint.pack(anchor="w", pady=(self.px(4), 0))
        self._watch()   # (whatever is shown: Heirloom gone = on with the install)

    def _progress(self):
        self.show("progress")

    def _page_progress(self):
        b = self.body
        head = ("Odinstalowuję Heirloom" if self.uninstalling else
                f"Aktualizuję Heirloom do wersji {VERSION}" if self.upgrade else f"Instaluję Heirloom {VERSION}")
        self.p_head = self.label(b, head, "serif", 22)
        self.p_head.pack(anchor="w")
        row = tk.Frame(b, bg=BG)
        row.pack(fill="x", pady=(self.px(18), self.px(8)))
        self.p_pct = self.label(row, "0%", "semi", 13, TEXT3)
        self.p_pct.pack(side="right")
        self.p_status = self.label(row, "Zaczynam…", wraplength=self.wrap(60))
        self.p_status.pack(side="left")
        self.bar_w, self.bar_h = self.px(self.W - 56), self.px(8)
        self.p_bar = tk.Label(b, bg=BG, bd=0)
        self.p_bar.pack(anchor="w")
        self.p_file = self.label(b, " ", size=12, fg=TEXT3, wraplength=self.wrap())
        self.p_file.pack(fill="x", pady=(self.px(8), 0))
        self.p_note = self.label(b, "", size=13, fg=TEXT2, wraplength=self.wrap())
        self.p_toggle = self.label(b, "", "semi", 12, TEXT3, cursor="hand2")
        self.p_toggle.pack(anchor="w", pady=(self.px(10), 0))
        self.p_toggle.bind("<ButtonRelease-1>", lambda e: self._details(not self.details_open))
        self.p_toggle.bind("<Enter>", lambda e: self.p_toggle.configure(fg=ACCENT))
        self.p_toggle.bind("<Leave>", lambda e: self.p_toggle.configure(fg=TEXT3))
        self.p_details = tk.Text(b, height=7, bg=SURFACE, fg=TEXT2, font=self.font("mono", 12), relief="flat",
                                 bd=0, highlightthickness=self.px(1), highlightbackground=BORDER,
                                 highlightcolor=BORDER, padx=self.px(10), pady=self.px(8), wrap="word",
                                 insertbackground=SURFACE, selectbackground=ACCENT_SOFT, selectforeground=TEXT)
        self.p_details.insert("end", NEWLINE.join(self.lines))
        self.p_details.configure(state="disabled")
        self._details(self.details_open, fit=False)
        self._draw_bar(force=True)
        for child in self.footer.winfo_children():
            child.destroy()
        self.primary = self.secondary = None
        note = ("" if self.uninstalling else "Potem Heirloom uruchomi się sam." if self.mode == "update"
                else "To potrwa chwilę.")
        self.p_footnote = self.label(self.footer, note, size=12, fg=TEXT3)
        self.p_footnote.pack(side="left", padx=self.px(28))
        self.busy = True
        # Not a daemon: if the window goes away mid-install (a Tk error, Windows closing it), the process still waits
        # for the install to finish - or to put the old version back - instead of dying half-way.
        threading.Thread(target=work, args=(self.mode, self._post, self.remove_own_data.get())).start()

    def _post(self, kind, value):   # (worker thread: only the queue)
        self.events.put((kind, value))

    def _details(self, show: bool, fit=True):
        self.details_open = show
        self.p_toggle.configure(text=("▾  Ukryj szczegóły" if show else "▸  Pokaż szczegóły"))
        if show:
            self.p_details.pack(fill="x", pady=(self.px(8), 0))   # (its own height: whole lines, none cut off)
            self.p_details.see("end")
        else:
            self.p_details.pack_forget()
        if fit:
            self._fit()

    def _page_finish(self):
        b = self.body
        kind, left = self.outcome
        self.badge(b, ACCENT, ACCENT_SOFT, "tick").pack(anchor="w", pady=(self.px(4), 0))
        if kind == "uninstalled":
            title = "Heirloom jest odinstalowany"
            if not self.remove_own_data.get():
                text = "Archiwa rodzinne zostały nietknięte, ustawienia programu też — po ponownej instalacji " \
                       "wszystko będzie jak przedtem."
            elif left:
                text = ("Archiwa rodzinne zostały nietknięte. Część ustawień programu nie trafiła do Kosza i została "
                        "na dysku:" + NEWLINE + NEWLINE.join(str(p) for p in left))
            else:
                text = "Archiwa rodzinne zostały nietknięte. Ustawienia programu i miniatury są w Koszu."
            text += NEWLINE + "Folder programu zniknie po zamknięciu tego okna."
        else:
            title = f"Heirloom {VERSION} jest zainstalowany"
            text = ("Archiwa rodzinne i ustawienia zostały bez zmian." if self.current else
                    "Znajdziesz go w menu Start i na pulpicie. Przy pierwszym uruchomieniu założysz nowe archiwum "
                    "rodzinne albo otworzysz istniejące.")
        self.label(b, title, "serif", 22).pack(anchor="w", pady=(self.px(14), 0))
        self.label(b, text, fg=TEXT2, wraplength=self.wrap()).pack(anchor="w", pady=(self.px(6), 0))
        if kind == "installed":
            Check(self, b, "Uruchom Heirloom", self.run_app, self.wrap(28)).pack(anchor="w", pady=(self.px(20), 0))
            self.buttons(("Zakończ", self._close, "primary"))
        else:
            self.buttons(("Zamknij", self._close, "primary"))

    # ----- the worker's news (Tk thread) -----

    def _drain(self):
        try:
            self._take_news()
        finally:   # (an error here must not stop the window from following the install)
            self.after(30, self._drain)

    def _take_news(self):
        news, new_lines = [], []
        try:
            while True:
                kind, value = self.events.get_nowait()
                if kind == "progress":
                    self.target, self.end = max(self.target, value[0]), max(self.end, value[1])
                elif kind == "log":
                    new_lines.append(value)
                    if self.page == "progress":
                        self.p_status.configure(text=value)
                        self.p_file.configure(text=" ")
                elif kind == "file":
                    if value != self.last_file:
                        self.last_file = value
                        new_lines.append("  " + value)
                    if self.page == "progress":
                        self.p_file.configure(text=value if len(value) < 72 else "…" + value[-70:])
                else:
                    news.append((kind, value))
        except queue.Empty:
            pass
        if new_lines:
            self.lines += new_lines
            if self.page == "progress":
                self.p_details.configure(state="normal")
                self.p_details.insert("end", (NEWLINE if self.p_details.index("end-1c") != "1.0" else "")
                                      + NEWLINE.join(new_lines))
                self.p_details.see("end")
                self.p_details.configure(state="disabled")
        for kind, value in news:
            getattr(self, "_on_" + kind)(value)
        if self.page == "progress" and self.busy:
            self.shown = ease(self.shown, self.target, self.end)
            self._draw_bar()

    def _draw_bar(self, force=False):
        filled = 0 if self.shown <= 0 else max(self.bar_h, round(self.bar_w * self.shown))
        self.p_pct.configure(text=f"{int(self.shown * 100 + 1e-6)}%")
        if filled == self.bar_px and not force:
            return
        self.bar_px = filled
        self.bar_image = photo(bar_rows(self.bar_w, self.bar_h, filled, self.bar_colour, BG))
        self.p_bar.configure(image=self.bar_image)

    def _on_done(self, outcome):
        self.busy, self.done, self.outcome = False, True, outcome
        self.shown = self.target = self.end = 1.0
        self._draw_bar()
        if self.mode == "update":   # (work() has started Heirloom again)
            self.p_head.configure(text=f"Heirloom {VERSION} jest zainstalowany")
            self.p_status.configure(text="Heirloom uruchamia się ponownie. To okno zamknie się samo.")
            self.p_file.configure(text=" ")
            self.p_footnote.configure(text="")
            self.after(2000, self._close)
        else:
            self.after(450, lambda: self.show("finish"))

    def _on_failed(self, failure):
        """Stays on screen, with the details open, until „Spróbuj ponownie” or „Zamknij”."""
        kind, error, undone = failure
        self.busy = False
        if kind == "running":   # (nothing was changed) the "running" page again, with what Heirloom says now
            self._go()
            return
        self.shown = self.end = self.target   # (the bar stays where the work stopped)
        self.p_footnote.configure(text="")
        self.bar_colour = ERR
        self.p_head.configure(text="Odinstalowanie nie powiodło się" if self.uninstalling else
                              "Aktualizacja nie powiodła się" if self.upgrade else
                              "Instalacja nie powiodła się")
        self.p_status.configure(text="Coś poszło nie tak:", fg=ERR)
        self.p_file.configure(text=error, fg=TEXT)
        # The old version is whole (put back, or never touched): if this install closed it (an update, or OK on
        # „Heirloom jest uruchomiony”), it starts again when the window closes - never left off.
        self.start_old = (not self.uninstalling and undone in ("restored", None)
                          and (self.mode == "update" or self.was_running)
                          and INSTALL_DIR is not None and (INSTALL_DIR / EXE_NAME).exists())
        if undone == "restored":
            self.p_note.configure(text=f"Poprzednia wersja{f' ({self.current})' if self.current else ''} jest z "
                                       "powrotem na miejscu" + (" i uruchomi się po zamknięciu tego okna."
                                                                if self.start_old else "."))
        elif undone is None and not self.uninstalling:
            self.p_note.configure(text="Nic nie zostało zmienione" + (" — Heirloom uruchomi się po zamknięciu "
                                                                      "tego okna." if self.start_old else "."))
        elif undone == "removed":
            self.p_note.configure(text="Nic nie zostało zainstalowane.")
        elif undone == "partial":
            self.p_note.configure(text="Nie wszystko udało się przywrócić — szczegóły poniżej. "
                                       "„Spróbuj ponownie” zainstaluje program jeszcze raz.")
        if self.p_note["text"]:
            self.p_note.pack(fill="x", pady=(self.px(6), 0), before=self.p_toggle)
        self._draw_bar(force=True)
        self.p_details.configure(height=5)   # (the error above takes the room of the other lines)
        self._details(True)
        self.buttons(("Spróbuj ponownie", self._retry, "primary"), ("Zamknij", self._close, "secondary"))

    def _retry(self):
        self.lines.append("— jeszcze raz —")
        self.shown = self.target = self.end = 0.0
        self.bar_colour, self.bar_px, self.start_old, self.last_file = ACCENT, None, False, None
        self._go()

    def _close(self):
        if self.busy:   # (don't quit halfway)
            return
        self.destroy()
        if self.uninstalling and self.done:
            # Only the folder this uninstaller runs from, and only when it is the program folder.
            here = Path(sys.executable).resolve().parent if getattr(sys, "frozen", False) else None
            if INSTALL_DIR is not None and here is not None and here == INSTALL_DIR.resolve():
                remove_program_folder(INSTALL_DIR)
        elif self.mode != "update" and self.done and self.run_app.get():
            launch_app()
        elif self.start_old:
            launch_app()


def dpi_aware():
    """Sharp text at 125 / 150 %: said before the window exists (sizes then follow via SetupWindow.px)."""
    try:
        ctypes.windll.shcore.SetProcessDpiAwareness(1)
    except Exception:
        pass


def main():
    dpi_aware()
    SetupWindow(mode_of(sys.argv)).mainloop()


if __name__ == "__main__":
    main()
