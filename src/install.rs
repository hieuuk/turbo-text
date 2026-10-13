//! Per-user install and uninstall (no admin): copies the exe to `%LOCALAPPDATA%\Programs\TurboText`,
//! registers the "Open with" associations against that copy, and adds an Apps & features entry.
//! Settings and the usage log in `%APPDATA%\turbo-text` are never touched.

use crate::assoc;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use winreg::RegKey;
use winreg::enums::*;

const UNINSTALL: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\TurboText";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn install_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map_or_else(std::env::temp_dir, PathBuf::from);
    base.join("Programs").join("TurboText")
}

fn installed_exe() -> PathBuf {
    install_dir().join("turbo-text.exe")
}

/// Copies this exe into the install folder (unless it already runs from there), registers the
/// associations for `exts` and the uninstall entry. Returns the installed exe path.
pub fn install(exts: &[String]) -> Result<PathBuf, String> {
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let dest = installed_exe();
    std::fs::create_dir_all(install_dir()).map_err(|e| e.to_string())?;
    if !same_file(&me, &dest) {
        std::fs::copy(&me, &dest).map_err(|e| {
            format!(
                "cannot copy to {} (is TurboText running from there?): {e}",
                dest.display()
            )
        })?;
    }
    let exe = dest.to_string_lossy().into_owned();
    assoc::associate_in(assoc::CLASSES, exts, &exe)?;
    write_uninstall_entry(UNINSTALL, &exe).map_err(|e| e.to_string())?;
    Ok(dest)
}

/// Removes the associations for `exts`, the uninstall entry and (after this process exits) the
/// installed exe and folder.
pub fn uninstall(exts: &[String]) -> Result<(), String> {
    assoc::unassociate_in(assoc::CLASSES, exts)?;
    assoc::remove_prog_in(assoc::CLASSES);
    remove_uninstall_entry(UNINSTALL);
    let exe = installed_exe();
    if exe.exists() {
        // a running exe cannot delete itself: a hidden cmd waits a moment, then removes it
        let _ = std::process::Command::new("cmd")
            .raw_arg(delete_command(&install_dir()))
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }
    Ok(())
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// `cmd` arguments that delete the install folder's files once the caller has exited.
fn delete_command(dir: &Path) -> String {
    let exe = dir.join("turbo-text.exe");
    format!(
        "/C \"ping -n 3 127.0.0.1 >nul & del /f /q \"{}\" \"{}.prev\" & rmdir \"{}\"\"",
        exe.display(),
        exe.display(),
        dir.display()
    )
}

fn write_uninstall_entry(key: &str, exe: &str) -> std::io::Result<()> {
    let (k, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(key)?;
    let dir = Path::new(exe)
        .parent()
        .map_or_else(String::new, |p| p.display().to_string());
    k.set_value("DisplayName", &"TurboText")?;
    k.set_value("DisplayVersion", &env!("CARGO_PKG_VERSION"))?;
    k.set_value("Publisher", &"hieuuk")?;
    k.set_value("DisplayIcon", &exe)?;
    k.set_value("InstallLocation", &dir)?;
    k.set_value("UninstallString", &format!("\"{exe}\" --uninstall"))?;
    k.set_value("NoModify", &1u32)?;
    k.set_value("NoRepair", &1u32)
}

fn remove_uninstall_entry(key: &str) {
    let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(key);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delete_command_targets_only_the_install_folder() {
        let c = delete_command(Path::new("C:\\Users\\a\\Programs\\TurboText"));
        assert!(c.starts_with("/C \"") && c.ends_with('"'));
        assert!(c.contains("del /f /q \"C:\\Users\\a\\Programs\\TurboText\\turbo-text.exe\""));
        assert!(c.contains("rmdir \"C:\\Users\\a\\Programs\\TurboText\""));
    }

    /// Real registry round trip under a throwaway HKCU key, cleaned up after.
    #[test]
    fn uninstall_entry_round_trip() {
        let key = "Software\\TurboTextTest\\Uninstall";
        write_uninstall_entry(key, "C:\\x\\turbo-text.exe").unwrap();
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let k = hkcu.open_subkey(key).unwrap();
        assert_eq!(
            k.get_value::<String, _>("DisplayName").unwrap(),
            "TurboText"
        );
        assert_eq!(
            k.get_value::<String, _>("UninstallString").unwrap(),
            "\"C:\\x\\turbo-text.exe\" --uninstall"
        );
        remove_uninstall_entry(key);
        assert!(hkcu.open_subkey(key).is_err());
        let _ = hkcu.delete_subkey_all("Software\\TurboTextTest");
    }
}
