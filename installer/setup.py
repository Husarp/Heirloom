"""HeirloomSetup-X.Y.Z.exe - installs, updates and uninstalls Heirloom (APP-STANDARDS.md section 1).

Built by scripts/build.ps1, after the Reckless Driving installer: per-user into %LOCALAPPDATA%\\Programs\\Heirloom,
no administrator rights. Install or update is not a separate build: the exe reads DisplayVersion from its uninstall
key and, if Heirloom is already there, says Aktualizuj instead of Zainstaluj.

What it never touches: the family archives. They are ordinary folders the family chose (rodzina.ged, media/,
.heirloom/), anywhere on the disk, and neither an update nor an uninstall goes near them - nor near
%LOCALAPPDATA%\\Heirloom\\archives, where an archive in a read-only folder keeps its history and backup copies.
The program's own settings (%APPDATA%\\Heirloom: recent archives, theme), its thumbnails (%LOCALAPPDATA%\\Heirloom\\
cache) and the window's data (%LOCALAPPDATA%\\com.husarp.heirloom) stay too, unless the box is ticked while
uninstalling - and then they go to the Recycle Bin; Windows asks first if a folder can't go there.

Heirloom saves only when told to, so a running Heirloom is never closed by force: the family is asked to save and
close it first.
"""
import ctypes
import os
import shutil
import subprocess
import sys
import threading
import tkinter as tk
import uuid
import winreg
import zipfile
from ctypes import wintypes
from pathlib import Path
from tkinter import messagebox, ttk

from version import VERSION   # written by scripts/build.ps1 from Cargo.toml

APP = "Heirloom"
EXE_NAME = "heirloom.exe"
FOLDER = "Heirloom"
UNINSTALLER = f"Odinstaluj {APP}.exe"
SHORTCUT = f"{APP}.lnk"
NEWLINE = chr(10)   # written this way so the source survives being edited by scripts
NO_WINDOW = subprocess.CREATE_NO_WINDOW
UNINSTALL_KEY = rf"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{FOLDER}"
# The Microsoft Edge WebView2 runtime draws Heirloom's window (it comes with Windows 11 and current Windows 10).
WEBVIEW2 = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"


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
# The program's own data. Never the family archives, and never %LOCALAPPDATA%\Heirloom\archives (see the docstring).
OWN_DATA = [p for p in (ROAMING and ROAMING / "Heirloom", LOCAL and LOCAL / "Heirloom" / "cache", LOCAL and LOCAL / "com.husarp.heirloom") if p]

SYSTEM32 = Path(os.environ.get("SystemRoot", r"C:\Windows")) / "System32"
# Full paths: a user's PATH is not guaranteed to include System32.
TOOLS = {"tasklist": SYSTEM32 / "tasklist.exe",
         "powershell": SYSTEM32 / r"WindowsPowerShell\v1.0\powershell.exe",
         "cmd": SYSTEM32 / "cmd.exe",
         "explorer.exe": Path(os.environ.get("SystemRoot", r"C:\Windows")) / "explorer.exe"}


def tool(name: str) -> str:
    return str(TOOLS.get(name, name))


def run(*args, env: dict | None = None) -> subprocess.CompletedProcess:
    return subprocess.run([tool(args[0]), *args[1:]], capture_output=True, text=True,
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


def app_running() -> bool:
    result = run("tasklist", "/FI", f"IMAGENAME eq {EXE_NAME}", "/NH", "/FO", "CSV")
    return EXE_NAME.lower() in result.stdout.lower()


# ---------- steps ----------

def copy_files(log):
    log("Kopiuję program…")
    INSTALL_DIR.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(payload()) as z:
        z.extractall(INSTALL_DIR)
    uninstaller = INSTALL_DIR / UNINSTALLER
    if getattr(sys, "frozen", False) and Path(sys.executable).resolve() != uninstaller.resolve():
        shutil.copy2(sys.executable, uninstaller)


def shortcuts(log):
    """The paths go to PowerShell as environment variables, never inside its command text (an apostrophe in a user
    name would break the quoting)."""
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


def install(log):
    copy_files(log)
    shortcuts(log)
    uninstall_entry(log)
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


def uninstall(log, remove_own_data: bool):
    log("Usuwam skróty…")
    for folder in (START_MENU, DESKTOP):
        if folder:
            (folder / SHORTCUT).unlink(missing_ok=True)
    try:
        winreg.DeleteKey(winreg.HKEY_CURRENT_USER, UNINSTALL_KEY)
    except OSError:
        pass
    if remove_own_data:
        for folder in OWN_DATA:
            if folder.exists():
                if to_recycle_bin(folder):
                    log(f"Do Kosza: {folder}")
                else:
                    log(f"Zostaje na dysku (nie trafiło do Kosza): {folder}")
    else:
        log("Ustawienia programu zostają — po ponownej instalacji wszystko będzie jak przedtem.")
    log("Archiwa rodzinne zostają nietknięte.")
    log("Heirloom jest odinstalowany. Folder programu zniknie po zamknięciu tego okna.")


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


# ---------- window ----------

class SetupWindow(tk.Tk):
    def __init__(self, uninstalling: bool):
        super().__init__()
        self.uninstalling = uninstalling
        self.title(f"Odinstaluj {APP}" if uninstalling else f"{APP} — instalacja")
        self.geometry("560x380")
        self.resizable(False, False)
        icon = Path(getattr(sys, "_MEIPASS", ".")) / "heirloom.ico"
        if icon.exists():
            self.iconbitmap(str(icon))
        box = ttk.Frame(self, padding=18)
        box.pack(fill="both", expand=True)

        current = installed_version()
        newer_installed = current is not None and version_key(current) > version_key(VERSION)
        if INSTALL_DIR is None:
            intro = "Windows nie podał folderu na programy tego konta (AppData\\Local), więc nie da się tu zainstalować Heirloom."
        elif uninstalling:
            intro = ("To usunie Heirloom z tego komputera." + NEWLINE +
                     "Archiwa rodzinne (Twoje foldery z plikiem .ged i zdjęciami) zostają zawsze. "
                     "Ustawienia programu zostają, chyba że zaznaczysz pole.")
        elif current == VERSION:
            intro = (f"Heirloom {VERSION} jest już zainstalowany." + NEWLINE +
                     "Ponowna instalacja nie zmieni archiwów rodzinnych ani ustawień.")
        elif newer_installed:
            intro = (f"Zainstalowana jest nowsza wersja, Heirloom {current}. Ten program zamieni ją na starszą, {VERSION}." + NEWLINE +
                     "Archiwa rodzinne i ustawienia zostają bez zmian.")
        elif current:
            intro = (f"Zainstalowany jest Heirloom {current}. Ten program zaktualizuje go do wersji {VERSION}." + NEWLINE +
                     "Archiwa rodzinne i ustawienia zostają bez zmian.")
        else:
            intro = (f"To zainstaluje Heirloom {VERSION} tylko dla Twojego konta, bez uprawnień administratora." + NEWLINE +
                     f"Program trafi do {INSTALL_DIR}.")
        if not uninstalling and INSTALL_DIR is not None and not has_webview2():
            intro += (NEWLINE + NEWLINE + "Uwaga: brakuje składnika Microsoft Edge WebView2, którego potrzebuje okno "
                      "programu. Zainstaluj go z witryny Microsoftu („WebView2 Runtime”), a potem uruchom Heirloom.")
        ttk.Label(box, text=intro, wraplength=510, justify="left").pack(anchor="w")

        self.remove_own_data = tk.BooleanVar(value=False)
        if uninstalling and INSTALL_DIR is not None:
            ttk.Checkbutton(box, text="Usuń też ustawienia programu i miniatury (trafią do Kosza)",
                            variable=self.remove_own_data).pack(anchor="w", pady=(10, 0))

        self.log_box = tk.Text(box, height=8, width=66, state="disabled", relief="flat",
                               background="#F2F2F2", font=("Segoe UI", 9))
        self.log_box.pack(fill="both", expand=True, pady=12)

        self.run_app = tk.BooleanVar(value=True)
        self.done_note = ttk.Label(box, text="", foreground="#2F5D50", font=("Segoe UI", 9, "bold"))
        self.run_check = ttk.Checkbutton(box, text="Uruchom Heirloom teraz", variable=self.run_app)
        self.buttons = ttk.Frame(box)
        self.buttons.pack(fill="x")
        label = ("Odinstaluj" if uninstalling else "Zainstaluj ponownie" if current == VERSION
                 else "Zainstaluj starszą wersję" if newer_installed else "Aktualizuj" if current else "Zainstaluj")
        self.go = ttk.Button(self.buttons, text=label, command=self._start)
        self.go.pack(side="right")
        if INSTALL_DIR is None:
            self.go.configure(state="disabled")
        self.close = ttk.Button(self.buttons, text="Anuluj", command=self._close)
        self.close.pack(side="right", padx=8)
        self.protocol("WM_DELETE_WINDOW", self._close)
        self.done = False

    def log(self, text: str):
        self.after(0, self._append, text)

    def _append(self, text: str):
        self.log_box.configure(state="normal")
        self.log_box.insert("end", text + NEWLINE)
        self.log_box.see("end")
        self.log_box.configure(state="disabled")

    def _start(self):
        # A running Heirloom may hold unsaved changes: it is never closed by force.
        while app_running():
            if not messagebox.askretrycancel(
                    "Heirloom jest uruchomiony",
                    "Heirloom jest teraz otwarty. Jeśli masz niezapisane zmiany, zapisz je (Ctrl S), a potem zamknij "
                    "program i kliknij „Ponów”.", parent=self):
                return
        current = installed_version()
        # Installing the same version again is almost always an accident (an old installer kept from an update, a
        # second double-click), and an older version over a newer one too: both are confirmed outright; checked at
        # click time, not when the window opened.
        if not self.uninstalling and current == VERSION:
            if not messagebox.askyesno(
                    f"Heirloom {VERSION} jest już zainstalowany",
                    f"Masz już Heirloom {VERSION} — to ta sama wersja, nie aktualizacja." + NEWLINE + NEWLINE +
                    "Ponowna instalacja niczego nie psuje i nie zmienia archiwów ani ustawień. Zainstalować mimo to?",
                    parent=self, default="no"):
                return
        if not self.uninstalling and current and version_key(current) > version_key(VERSION):
            if not messagebox.askyesno(
                    "Starsza wersja",
                    f"Zainstalowany Heirloom {current} jest nowszy niż ten instalator ({VERSION})." + NEWLINE + NEWLINE +
                    "Zamienić go na starszą wersję?", parent=self, default="no"):
                return
        self.go.configure(state="disabled")
        self.close.configure(state="disabled")
        threading.Thread(target=self._work, daemon=True).start()

    def _work(self):
        try:
            if self.uninstalling:
                uninstall(self.log, self.remove_own_data.get())
                self.done = True
                self.after(0, lambda: self.close.configure(state="normal", text="Zamknij"))
            else:
                install(self.log)
                self.done = True
                self.after(0, self._finish_install)
        except Exception as e:      # show it rather than vanish
            self.log(f"Coś poszło nie tak: {e}")
            self.after(0, lambda: self.close.configure(state="normal", text="Zamknij"))

    def _finish_install(self):
        self.go.pack_forget()
        self.done_note.configure(text=f"\u2713  Heirloom {VERSION} jest gotowy.")
        self.done_note.pack(anchor="w", pady=(0, 2), before=self.buttons)
        self.run_check.pack(anchor="w", pady=(0, 8), before=self.buttons)
        self.close.configure(state="normal", text="Zakończ")

    def _close(self):
        if str(self.close.cget("state")) == "disabled":   # busy: don't quit halfway
            return
        self.destroy()
        if self.uninstalling and self.done:
            # Only the folder this uninstaller runs from, and only when it is the program folder.
            here = Path(sys.executable).resolve().parent if getattr(sys, "frozen", False) else None
            if INSTALL_DIR is not None and here is not None and here == INSTALL_DIR.resolve():
                remove_program_folder(INSTALL_DIR)
        elif not self.uninstalling and self.done and self.run_app.get():
            launch_app()


def main():
    SetupWindow("--uninstall" in sys.argv).mainloop()


if __name__ == "__main__":
    main()
