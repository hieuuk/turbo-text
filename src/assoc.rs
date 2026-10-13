//! Per-user (HKCU, no admin) file-association registration.
//! Registers the app as an "Open with" handler for each extension and only sets the
//! extension's default value when it has none. Windows keeps the real default-app choice
//! behind the user's confirmation, so `open_default_apps` opens that settings page.

use winreg::RegKey;
use winreg::enums::*;

pub const PROG_ID: &str = "TurboText.File";
pub const CLASSES: &str = "Software\\Classes";

pub fn normalize_ext(s: &str) -> String {
    let t = s.trim().trim_start_matches('.').to_ascii_lowercase();
    if t.is_empty()
        || !t
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return String::new();
    }
    format!(".{t}")
}

pub fn ext_key(base: &str, ext: &str) -> String {
    format!("{base}\\{ext}\\OpenWithProgids")
}

pub fn prog_key(base: &str) -> String {
    format!("{base}\\{PROG_ID}")
}

pub fn associate(exts: &[String]) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    associate_in(CLASSES, exts, &exe.to_string_lossy())
}

pub fn unassociate(exts: &[String]) -> Result<(), String> {
    unassociate_in(CLASSES, exts)
}

pub fn is_associated(ext: &str) -> bool {
    is_associated_in(CLASSES, ext)
}

pub fn open_default_apps() {
    let _ = std::process::Command::new("cmd")
        .args(["/c", "start", "", "ms-settings:defaultapps"])
        .spawn();
}

fn valid(ext: &str) -> Result<(), String> {
    if !ext.is_empty() && normalize_ext(ext) == ext {
        Ok(())
    } else {
        Err(format!("invalid extension: {ext:?}"))
    }
}

fn e(err: std::io::Error) -> String {
    err.to_string()
}

pub fn associate_in(base: &str, exts: &[String], exe: &str) -> Result<(), String> {
    for x in exts {
        valid(x)?;
    }
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let command = format!("\"{exe}\" \"%1\"");
    let (prog, _) = hkcu.create_subkey(prog_key(base)).map_err(e)?;
    prog.set_value("", &"TurboText File").map_err(e)?;
    let (cmd, _) = prog.create_subkey("shell\\open\\command").map_err(e)?;
    cmd.set_value("", &command).map_err(e)?;
    let exe_name = std::path::Path::new(exe)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned());
    if let Some(name) = exe_name {
        let (app, _) = hkcu
            .create_subkey(format!(
                "{base}\\Applications\\{name}\\shell\\open\\command"
            ))
            .map_err(e)?;
        app.set_value("", &command).map_err(e)?;
    }
    for x in exts {
        let (with, _) = hkcu.create_subkey(ext_key(base, x)).map_err(e)?;
        with.set_value(PROG_ID, &"").map_err(e)?;
        let (ext, _) = hkcu.create_subkey(format!("{base}\\{x}")).map_err(e)?;
        let current: String = ext.get_value("").unwrap_or_default();
        if current.is_empty() {
            ext.set_value("", &PROG_ID).map_err(e)?;
        }
    }
    Ok(())
}

pub fn unassociate_in(base: &str, exts: &[String]) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let flags = KEY_SET_VALUE | KEY_QUERY_VALUE;
    for x in exts {
        valid(x)?;
        if let Ok(with) = hkcu.open_subkey_with_flags(ext_key(base, x), flags) {
            let _ = with.delete_value(PROG_ID);
        }
        if let Ok(ext) = hkcu.open_subkey_with_flags(format!("{base}\\{x}"), flags)
            && ext.get_value::<String, _>("").is_ok_and(|v| v == PROG_ID)
        {
            let _ = ext.delete_value("");
        }
    }
    Ok(())
}

/// Deletes the `TurboText.File` handler class itself (used by `--uninstall`, after the per-extension
/// registrations are removed, since it points at the exe that is being deleted).
pub fn remove_prog_in(base: &str) {
    let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(prog_key(base));
}

pub fn is_associated_in(base: &str, ext: &str) -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(ext_key(base, ext))
        .is_ok_and(|k| k.get_value::<String, _>(PROG_ID).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_cases() {
        assert_eq!(normalize_ext("json"), ".json");
        assert_eq!(normalize_ext(".JSON"), ".json");
        assert_eq!(normalize_ext("  .Env "), ".env");
        assert_eq!(normalize_ext(""), "");
        assert_eq!(normalize_ext("."), "");
        assert_eq!(normalize_ext("a\\b"), "");
        assert_eq!(normalize_ext("a b"), "");
    }

    #[test]
    fn key_builders() {
        assert_eq!(
            ext_key("Software\\Classes", ".md"),
            "Software\\Classes\\.md\\OpenWithProgids"
        );
        assert_eq!(
            prog_key("Software\\Classes"),
            "Software\\Classes\\TurboText.File"
        );
    }

    /// Real registry round trip, isolated under a throwaway HKCU key and cleaned up after.
    #[test]
    fn registry_roundtrip_in_sandbox_key() {
        let base = format!("Software\\TurboTextTest{}\\Classes", std::process::id());
        let exts = vec![".ttassoc".to_string(), ".ttassoc2".to_string()];
        assert!(!is_associated_in(&base, ".ttassoc"));
        associate_in(&base, &exts, "C:\\Apps\\turbo-text.exe").unwrap();
        assert!(is_associated_in(&base, ".ttassoc") && is_associated_in(&base, ".ttassoc2"));

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let cmd = hkcu
            .open_subkey(format!("{}\\shell\\open\\command", prog_key(&base)))
            .unwrap();
        let v: String = cmd.get_value("").unwrap();
        assert_eq!(v, "\"C:\\Apps\\turbo-text.exe\" \"%1\"");

        // default value is only set when empty: pre-existing default is preserved
        let other = hkcu.create_subkey(format!("{base}\\.ttassoc2")).unwrap().0;
        other.set_value("", &"Other.Handler").unwrap();
        associate_in(&base, &exts, "C:\\Apps\\turbo-text.exe").unwrap();
        let d: String = hkcu
            .open_subkey(format!("{base}\\.ttassoc2"))
            .unwrap()
            .get_value("")
            .unwrap();
        assert_eq!(d, "Other.Handler");
        let d: String = hkcu
            .open_subkey(format!("{base}\\.ttassoc"))
            .unwrap()
            .get_value("")
            .unwrap();
        assert_eq!(d, PROG_ID);

        unassociate_in(&base, &exts).unwrap();
        assert!(!is_associated_in(&base, ".ttassoc") && !is_associated_in(&base, ".ttassoc2"));
        let d: String = hkcu
            .open_subkey(format!("{base}\\.ttassoc2"))
            .unwrap()
            .get_value("")
            .unwrap();
        assert_eq!(d, "Other.Handler");
        assert!(
            hkcu.open_subkey(format!("{base}\\.ttassoc"))
                .map(|k| k.get_value::<String, _>("").is_err())
                .unwrap_or(true)
        );

        remove_prog_in(&base);
        assert!(hkcu.open_subkey(prog_key(&base)).is_err());

        let root =
            base.split('\\').next().unwrap().to_string() + "\\" + base.split('\\').nth(1).unwrap();
        let _ = hkcu.delete_subkey_all(root);
    }

    #[test]
    fn invalid_ext_is_rejected() {
        let base = format!("Software\\TurboTextTest{}b\\Classes", std::process::id());
        assert!(associate_in(&base, &["".to_string()], "x").is_err());
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let _ = hkcu.delete_subkey_all(format!("Software\\TurboTextTest{}b", std::process::id()));
    }
}
