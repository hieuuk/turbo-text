use std::io;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eol {
    Lf,
    Crlf,
}

pub struct Loaded {
    pub text: String,
    pub eol: Eol,
    pub lossy: bool,
    pub size: u64,
}

pub fn load(path: &Path) -> io::Result<Loaded> {
    let bytes = std::fs::read(path)?;
    let size = bytes.len() as u64;
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    let (text, lossy) = match std::str::from_utf8(bytes) {
        Ok(s) => (s.to_owned(), false),
        Err(_) => (String::from_utf8_lossy(bytes).into_owned(), true),
    };
    let crlf = text.matches("\r\n").count();
    let lf = text.matches('\n').count() - crlf;
    let eol = if crlf > lf { Eol::Crlf } else { Eol::Lf };
    let text = if crlf > 0 {
        text.replace("\r\n", "\n")
    } else {
        text
    };
    Ok(Loaded {
        text,
        eol,
        lossy,
        size,
    })
}

/// Writes via a temp file in the same directory, then renames over the target.
pub fn save(path: &Path, text: &str, eol: Eol) -> io::Result<()> {
    let out = match eol {
        Eol::Lf => text.to_owned(),
        Eol::Crlf => text.replace('\n', "\r\n"),
    };
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = path.with_file_name(format!(".{name}.tt-tmp"));
    let result = std::fs::write(&tmp, out).and_then(|_| std::fs::rename(&tmp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tt-io-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn empty_file() {
        let d = tmp("empty");
        let p = d.join("a.txt");
        fs::write(&p, b"").unwrap();
        let l = load(&p).unwrap();
        assert_eq!(l.text, "");
        assert!(!l.lossy);
        assert_eq!(l.size, 0);
    }

    #[test]
    fn crlf_roundtrip() {
        let d = tmp("crlf");
        let p = d.join("a.txt");
        fs::write(&p, b"a\r\nb\r\n").unwrap();
        let l = load(&p).unwrap();
        assert_eq!(l.eol, Eol::Crlf);
        assert_eq!(l.text, "a\nb\n");
        save(&p, &l.text, l.eol).unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"a\r\nb\r\n");
    }

    #[test]
    fn lf_roundtrip() {
        let d = tmp("lf");
        let p = d.join("a.txt");
        fs::write(&p, b"a\nb\n").unwrap();
        let l = load(&p).unwrap();
        assert_eq!(l.eol, Eol::Lf);
        save(&p, &l.text, l.eol).unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"a\nb\n");
    }

    #[test]
    fn mixed_takes_majority() {
        let d = tmp("mixed");
        let p = d.join("a.txt");
        fs::write(&p, b"a\r\nb\r\nc\nd").unwrap();
        assert_eq!(load(&p).unwrap().eol, Eol::Crlf);
        fs::write(&p, b"a\nb\nc\r\nd").unwrap();
        assert_eq!(load(&p).unwrap().eol, Eol::Lf);
    }

    #[test]
    fn invalid_utf8_is_lossy() {
        let d = tmp("bad");
        let p = d.join("a.txt");
        fs::write(&p, [b'a', 0xff, b'b']).unwrap();
        let l = load(&p).unwrap();
        assert!(l.lossy);
        assert!(l.text.contains('a') && l.text.contains('b'));
    }

    #[test]
    fn bom_is_stripped() {
        let d = tmp("bom");
        let p = d.join("a.txt");
        fs::write(&p, b"\xEF\xBB\xBFhi").unwrap();
        assert_eq!(load(&p).unwrap().text, "hi");
    }

    #[test]
    fn save_leaves_no_temp_file() {
        let d = tmp("notmp");
        let p = d.join("a.txt");
        save(&p, "x", Eol::Lf).unwrap();
        assert_eq!(fs::read_dir(&d).unwrap().count(), 1);
    }

    #[test]
    fn failed_save_keeps_original() {
        let d = tmp("fail");
        let p = d.join("a.txt");
        fs::write(&p, b"orig").unwrap();
        let mut perm = fs::metadata(&p).unwrap().permissions();
        perm.set_readonly(true);
        fs::set_permissions(&p, perm.clone()).unwrap();
        let r = save(&p, "new", Eol::Lf);
        #[allow(clippy::permissions_set_readonly_false)]
        perm.set_readonly(false);
        fs::set_permissions(&p, perm).unwrap();
        assert!(r.is_err());
        assert_eq!(fs::read(&p).unwrap(), b"orig");
        assert_eq!(fs::read_dir(&d).unwrap().count(), 1);
    }
}
