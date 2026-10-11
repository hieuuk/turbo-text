import ctypes
import os
import sys
import winreg


def is_admin():
    try:
        return ctypes.windll.shell32.IsUserAnAdmin()
    except Exception:
        return False


def run_as_admin(args):
    ctypes.windll.shell32.ShellExecuteW(
        None, "runas", sys.executable, " ".join(args), None, 1
    )


def get_exe_path():
    if getattr(sys, "frozen", False):
        return sys.executable
    return os.path.abspath(sys.argv[0])


def set_default_for(ext):
    """Set TurboText as the default handler for a file extension on Windows.

    Must be run as admin. ext should be like '.md' or '.txt'.
    """
    if not is_admin():
        script = get_exe_path()
        run_as_admin([script, "--set-default", ext])
        return False

    prog_id = f"TurboText{ext.replace('.', '').upper()}"
    exe = get_exe_path()
    if not getattr(sys, "frozen", False):
        command = f'"{sys.executable}" "{exe}" "%1"'
    else:
        command = f'"{exe}" "%1"'

    # Register the ProgID
    with winreg.CreateKey(winreg.HKEY_CLASSES_ROOT, prog_id) as key:
        winreg.SetValue(key, "", winreg.REG_SZ, f"TurboText {ext} File")

    with winreg.CreateKey(
        winreg.HKEY_CLASSES_ROOT, f"{prog_id}\\shell\\open\\command"
    ) as key:
        winreg.SetValue(key, "", winreg.REG_SZ, command)

    # Associate extension with ProgID
    with winreg.CreateKey(winreg.HKEY_CLASSES_ROOT, ext) as key:
        winreg.SetValue(key, "", winreg.REG_SZ, prog_id)

    # Notify Windows of the change
    from ctypes import windll

    SHCNE_ASSOCCHANGED = 0x08000000
    SHCNF_IDLIST = 0x0000
    windll.shell32.SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None)
    return True


def add_context_menu():
    """Add 'Open with TurboText' to the Windows right-click context menu for all files."""
    if not is_admin():
        script = get_exe_path()
        run_as_admin([script, "--add-context-menu"])
        return False

    exe = get_exe_path()
    if not getattr(sys, "frozen", False):
        command = f'"{sys.executable}" "{exe}" "%1"'
    else:
        command = f'"{exe}" "%1"'

    key_path = r"*\shell\TurboText"
    with winreg.CreateKey(winreg.HKEY_CLASSES_ROOT, key_path) as key:
        winreg.SetValueEx(key, "", 0, winreg.REG_SZ, "Open with TurboText")
        winreg.SetValueEx(key, "Icon", 0, winreg.REG_SZ, exe)

    with winreg.CreateKey(winreg.HKEY_CLASSES_ROOT, key_path + r"\command") as key:
        winreg.SetValueEx(key, "", 0, winreg.REG_SZ, command)

    SHCNE_ASSOCCHANGED = 0x08000000
    SHCNF_IDLIST = 0x0000
    ctypes.windll.shell32.SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None)
    return True


def remove_context_menu():
    """Remove 'Open with TurboText' from the Windows right-click context menu."""
    if not is_admin():
        script = get_exe_path()
        run_as_admin([script, "--remove-context-menu"])
        return False

    try:
        winreg.DeleteKey(winreg.HKEY_CLASSES_ROOT, r"*\shell\TurboText\command")
        winreg.DeleteKey(winreg.HKEY_CLASSES_ROOT, r"*\shell\TurboText")
    except FileNotFoundError:
        pass

    SHCNE_ASSOCCHANGED = 0x08000000
    SHCNF_IDLIST = 0x0000
    ctypes.windll.shell32.SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None)
    return True
