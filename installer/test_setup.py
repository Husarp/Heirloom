"""Tests for the parts of setup.py that can run off Windows: the set file type's registry entries, written and
removed against an in-memory stand-in for winreg. Run from the project folder: python -m unittest installer/test_setup.py
(on Windows too; Tk, winreg and the shell calls are stood in for, so nothing on the computer is touched)."""
import ctypes
import sys
import types
import unittest
from pathlib import PureWindowsPath


class FakeRegistry(types.ModuleType):
    """The few winreg calls setup.py makes, over a dict: {lowercased key path: {value name: (type, value)}}."""
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE = "HKCU", "HKLM"
    KEY_READ, KEY_WRITE, KEY_SET_VALUE, REG_SZ, REG_DWORD = 1, 2, 4, 1, 4

    def __init__(self):
        super().__init__("winreg")
        self.keys = {}

    class Key:
        def __init__(self, path):
            self.path = path

        def __enter__(self):
            return self

        def __exit__(self, *exc):
            return False

    def _path(self, root, path):
        return f"{root}\\{path}".lower()

    def CreateKeyEx(self, root, path, reserved=0, access=0):
        full = self._path(root, path)
        parts = full.split("\\")
        for i in range(2, len(parts) + 1):   # (the parents are made too, as in the real registry)
            self.keys.setdefault("\\".join(parts[:i]), {})
        return self.Key(full)

    def OpenKey(self, root, path, reserved=0, access=0):
        full = self._path(root, path)
        if full not in self.keys:
            raise FileNotFoundError(full)
        return self.Key(full)

    def SetValueEx(self, key, name, reserved, kind, value):
        self.keys[key.path][name.lower()] = (kind, value)

    def QueryValueEx(self, key, name):
        try:
            kind, value = self.keys[key.path][name.lower()]
        except KeyError:
            raise FileNotFoundError(name) from None
        return value, kind

    def DeleteValue(self, key, name):
        if self.keys[key.path].pop(name.lower(), None) is None:
            raise FileNotFoundError(name)

    def _subkeys(self, full):
        return sorted({k[len(full) + 1:].split("\\")[0] for k in self.keys if k.startswith(full + "\\")})

    def EnumKey(self, key, index):
        subkeys = self._subkeys(key.path)
        if index >= len(subkeys):
            raise OSError("no more")
        return subkeys[index]

    def QueryInfoKey(self, key):
        return len(self._subkeys(key.path)), len(self.keys[key.path]), 0

    def DeleteKey(self, root, path):
        full = self._path(root, path)
        if full not in self.keys:
            raise FileNotFoundError(full)
        if self._subkeys(full):
            raise PermissionError("a key with subkeys")   # (as the real DeleteKey)
        del self.keys[full]

    def values(self, path):
        return {name: value for name, (_, value) in self.keys.get(self._path("HKCU", path), {}).items()}


class Anything(types.ModuleType):
    """Stands in for tkinter, which setup.py only needs for its window: any name is a class."""
    def __getattr__(self, name):
        return type(name, (), {})


class FakeShell:
    def __init__(self):
        self.notified = 0

    def SHChangeNotify(self, *args):
        self.notified += 1

    def SHGetKnownFolderPath(self, *args):
        return 1   # (no folders off Windows: INSTALL_DIR is None, and the tests pass their own)


registry, shell = FakeRegistry(), FakeShell()
sys.modules["winreg"] = registry
sys.modules["tkinter"] = Anything("tkinter")
sys.modules["tkinter.font"] = Anything("tkinter.font")
sys.modules["version"] = types.SimpleNamespace(VERSION="9.9.9")
ctypes.windll = types.SimpleNamespace(shell32=shell, ole32=types.SimpleNamespace(CoTaskMemFree=lambda *a: None))
sys.path.insert(0, str(__import__("pathlib").Path(__file__).parent))
import setup  # noqa: E402

PROGRAMS = PureWindowsPath(r"C:\Users\Ala Nowak\AppData\Local\Programs\Heirloom")
CLASSES = r"Software\Classes"


class SetFileType(unittest.TestCase):
    def setUp(self):
        registry.keys.clear()
        self.log = []
        setup.INSTALL_DIR = PROGRAMS

    def test_the_entries(self):
        entries = {(path, name): value for path, name, value in setup.file_type_entries(PROGRAMS)}
        self.assertEqual(entries[(".heirloom-zestaw", "")], "Heirloom.Zestaw")
        self.assertEqual(entries[("Heirloom.Zestaw", "")], "Zestaw archiwów Heirloom")
        self.assertEqual(entries[(r"Heirloom.Zestaw\DefaultIcon", "")], str(PROGRAMS / "heirloom-zestaw.ico"))
        # The path in quotes: a user name with a space must not split it (src-tauri/src/lib.rs reads --open).
        self.assertEqual(entries[(r"Heirloom.Zestaw\shell\open\command", "")],
                         rf'"{PROGRAMS}\heirloom.exe" --open "%1"')
        self.assertFalse(any(path.lower().startswith((".ged", "folder", "directory")) for path, _ in entries),
                         "GEDCOM files and folders belong to other programs")

    def test_registered_then_removed(self):
        before = shell.notified
        setup.register_file_type(self.log.append)
        self.assertEqual(registry.values(rf"{CLASSES}\.heirloom-zestaw"), {"": "Heirloom.Zestaw"})
        self.assertEqual(registry.values(rf"{CLASSES}\Heirloom.Zestaw\shell\open\command"),
                         {"": rf'"{PROGRAMS}\heirloom.exe" --open "%1"'})
        self.assertEqual(shell.notified, before + 1, "Explorer is told, so the icon shows at once")
        setup.unregister_file_type()
        self.assertEqual([k for k in registry.keys if "heirloom" in k], [], "nothing of it is left")
        self.assertEqual(shell.notified, before + 2)

    def test_an_update_writes_it_again(self):
        setup.register_file_type(self.log.append)
        setup.INSTALL_DIR = PureWindowsPath(r"D:\Programy\Heirloom")
        setup.register_file_type(self.log.append)
        self.assertEqual(registry.values(rf"{CLASSES}\Heirloom.Zestaw\DefaultIcon"),
                         {"": r"D:\Programy\Heirloom\heirloom-zestaw.ico"})

    def test_another_programs_link_stays(self):
        setup.register_file_type(self.log.append)
        with registry.CreateKeyEx("HKCU", rf"{CLASSES}\.heirloom-zestaw") as key:
            registry.SetValueEx(key, "", 0, registry.REG_SZ, "Inny.Program")
        with registry.CreateKeyEx("HKCU", rf"{CLASSES}\.heirloom-zestaw\OpenWithProgids") as key:
            registry.SetValueEx(key, "Inny.Program", 0, registry.REG_SZ, "")
        setup.unregister_file_type()
        self.assertEqual(registry.values(rf"{CLASSES}\.heirloom-zestaw"), {"": "Inny.Program"})
        self.assertEqual(registry.values(rf"{CLASSES}\.heirloom-zestaw\OpenWithProgids"), {"inny.program": ""})
        self.assertNotIn(rf"hkcu\{CLASSES}\heirloom.zestaw".lower(), registry.keys)

    def test_removing_what_is_not_there(self):
        setup.unregister_file_type()   # (an uninstall of a version that never registered it)
        self.assertEqual(registry.keys, {})

    def test_a_failure_does_not_stop_the_install(self):
        def refuse(*args):
            raise PermissionError("Odmowa dostępu")
        create, registry.CreateKeyEx = registry.CreateKeyEx, refuse
        try:
            setup.register_file_type(self.log.append)
        finally:
            registry.CreateKeyEx = create
        self.assertIn("Odmowa dostępu", self.log[-1])


if __name__ == "__main__":
    unittest.main()
