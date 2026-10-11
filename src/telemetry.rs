//! Local-only usage log. One JSON object per line in `usage.jsonl`. No network.
//! Never records file contents, keystrokes, or full paths. All I/O errors are swallowed.

use crate::settings::Settings;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;
use std::time::Instant;

const MAX_LINE: usize = 300;
const MAX_STR: usize = 100;

pub struct Telemetry {
    tx: Mutex<Option<Sender<String>>>,
    writer: Mutex<Option<JoinHandle<()>>>,
    started: Instant,
    salt: String,
    mode_secs: Mutex<BTreeMap<(String, bool), f64>>,
}

pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn init(settings: &Settings) -> Telemetry {
    init_in(crate::settings::dir(), settings)
}

const LOG: &str = "usage.jsonl";
const LOG_OLD: &str = "usage.1.jsonl";
const FEATURES: [&str; 8] = [
    "toggle_mode",
    "toggle_wrap",
    "find_open",
    "replace_all",
    "save",
    "save_fail",
    "problem",
    "hotkey",
];

pub fn init_in(dir: PathBuf, settings: &Settings) -> Telemetry {
    let mut t = Telemetry {
        tx: Mutex::new(None),
        writer: Mutex::new(None),
        started: Instant::now(),
        salt: String::new(),
        mode_secs: Mutex::new(BTreeMap::new()),
    };
    if !settings.telemetry || std::fs::create_dir_all(&dir).is_err() {
        return t;
    }
    rotate(
        &dir,
        settings.telemetry_max_kb,
        settings.telemetry_max_days,
        now_unix(),
    );
    t.salt = load_salt(&dir);
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let path = dir.join(LOG);
    let handle = std::thread::spawn(move || {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok();
        for line in rx {
            if let Some(f) = file.as_mut() {
                let _ = writeln!(f, "{line}");
                let _ = f.flush();
            }
        }
    });
    *t.tx.lock().unwrap() = Some(tx);
    *t.writer.lock().unwrap() = Some(handle);
    t
}

fn load_salt(dir: &Path) -> String {
    let p = dir.join("salt");
    if let Ok(s) = std::fs::read_to_string(&p)
        && !s.trim().is_empty()
    {
        return s.trim().to_owned();
    }
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::time::SystemTime::now().hash(&mut h);
    std::process::id().hash(&mut h);
    let salt = format!("{:016x}", h.finish());
    let _ = std::fs::write(&p, &salt);
    salt
}

impl Telemetry {
    pub fn enabled(&self) -> bool {
        self.tx.lock().map(|g| g.is_some()).unwrap_or(false)
    }

    pub fn event(&self, name: &str, fields: Value) {
        if let Ok(g) = self.tx.lock()
            && let Some(tx) = g.as_ref()
        {
            let _ = tx.send(event_line(now_unix(), name, fields));
        }
    }

    pub fn add_mode_time(&self, fmt: &str, view: bool, secs: f64) {
        if let Ok(mut m) = self.mode_secs.lock() {
            *m.entry((fmt.to_owned(), view)).or_insert(0.0) += secs;
        }
    }

    pub fn token(&self, path: &Path) -> String {
        path_token(path, &self.salt)
    }

    pub fn since_start_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    /// Emits `session_summary`, stops the writer, and waits for it to flush.
    pub fn shutdown(&self) {
        if !self.enabled() {
            return;
        }
        let (mut view, mut edit) = (serde_json::Map::new(), serde_json::Map::new());
        if let Ok(m) = self.mode_secs.lock() {
            for ((fmt, is_view), secs) in m.iter() {
                let target = if *is_view { &mut view } else { &mut edit };
                target.insert(fmt.clone(), json!((secs * 10.0).round() / 10.0));
            }
        }
        self.event(
            "session_summary",
            json!({"duration_s": self.started.elapsed().as_secs(), "view_secs": view, "edit_secs": edit}),
        );
        if let Ok(mut g) = self.tx.lock() {
            g.take();
        }
        if let Ok(mut w) = self.writer.lock()
            && let Some(h) = w.take()
        {
            let _ = h.join();
        }
    }
}

/// Synchronous append used from the panic hook, where the writer thread may already be gone.
pub fn log_panic(dir: &Path, msg: &str, enabled: bool) {
    if !enabled {
        return;
    }
    use std::io::Write;
    let line = event_line(now_unix(), "problem", json!({"kind": "panic", "msg": msg}));
    let _ = std::fs::create_dir_all(dir);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(LOG))
    {
        let _ = writeln!(f, "{line}");
    }
}

pub fn size_bucket(bytes: u64) -> &'static str {
    match bytes {
        0..=1023 => "<1k",
        1024..=10_239 => "<10k",
        10_240..=102_399 => "<100k",
        102_400..=1_048_575 => "<1m",
        1_048_576..=5_242_879 => "<5m",
        _ => ">=5m",
    }
}

pub fn trunc_str(s: &str, max: usize) -> String {
    let mut end = max.min(s.len());
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_owned()
}

pub fn event_line(now: u64, name: &str, fields: Value) -> String {
    let mut extra: Vec<(String, Value)> = match fields {
        Value::Object(m) => m
            .into_iter()
            .map(|(k, v)| {
                let v = match v {
                    Value::String(s) => Value::String(trunc_str(&s, MAX_STR)),
                    other => other,
                };
                (trunc_str(&k, 30), v)
            })
            .collect(),
        _ => Vec::new(),
    };
    loop {
        let mut obj = serde_json::Map::new();
        obj.insert("t".into(), json!(now));
        obj.insert("e".into(), json!(trunc_str(name, 40)));
        for (k, v) in &extra {
            obj.insert(k.clone(), v.clone());
        }
        let line = Value::Object(obj).to_string();
        if line.len() <= MAX_LINE {
            return line;
        }
        if extra.pop().is_none() {
            return json!({"t": now, "e": "truncated"}).to_string();
        }
    }
}

/// `ext:hash` — never the file name or directory.
pub fn path_token(path: &Path, salt: &str) -> String {
    use std::hash::{Hash, Hasher};
    let ext: String = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(8)
        .collect::<String>()
        .to_ascii_lowercase();
    let ext = if ext.is_empty() {
        "none".to_owned()
    } else {
        ext
    };
    let mut h = std::collections::hash_map::DefaultHasher::new();
    salt.hash(&mut h);
    path.to_string_lossy().hash(&mut h);
    format!("{ext}:{:016x}", h.finish())
}

fn event_time(line: &str) -> Option<u64> {
    serde_json::from_str::<Value>(line).ok()?.get("t")?.as_u64()
}

pub fn rotate(dir: &Path, max_kb: u64, max_days: u32, now: u64) {
    let log = dir.join(LOG);
    let Ok(meta) = std::fs::metadata(&log) else {
        return;
    };
    if meta.len() <= max_kb * 1024 {
        return;
    }
    let Ok(bytes) = std::fs::read(&log) else {
        return;
    };
    let cutoff = now.saturating_sub(max_days as u64 * 86_400);
    let kept: String = String::from_utf8_lossy(&bytes)
        .lines()
        .filter(|l| event_time(l).is_some_and(|t| t >= cutoff))
        .flat_map(|l| [l, "\n"])
        .collect();
    if std::fs::write(dir.join(LOG_OLD), kept).is_ok() {
        let _ = std::fs::remove_file(&log);
    }
}

pub fn stats(dir: &Path) -> String {
    let (mut sessions, mut startup_sum, mut startup_n) = (0u64, 0u64, 0u64);
    let mut opens: BTreeMap<String, u64> = BTreeMap::new();
    let mut features: BTreeMap<String, u64> = BTreeMap::new();
    let mut modes: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    for name in [LOG_OLD, LOG] {
        let Ok(bytes) = std::fs::read(dir.join(name)) else {
            continue;
        };
        for line in String::from_utf8_lossy(&bytes).lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let Some(e) = v.get("e").and_then(Value::as_str) else {
                continue;
            };
            match e {
                "session_start" => {
                    sessions += 1;
                    if let Some(ms) = v.get("startup_ms").and_then(Value::as_u64) {
                        startup_sum += ms;
                        startup_n += 1;
                    }
                }
                "file_open" => {
                    let f = v.get("format").and_then(Value::as_str).unwrap_or("?");
                    *opens.entry(f.to_owned()).or_default() += 1;
                }
                "session_summary" => {
                    for (key, is_view) in [("view_secs", true), ("edit_secs", false)] {
                        if let Some(m) = v.get(key).and_then(Value::as_object) {
                            for (fmt, secs) in m {
                                let e = modes.entry(fmt.clone()).or_default();
                                let s = secs.as_f64().unwrap_or(0.0);
                                if is_view { e.0 += s } else { e.1 += s }
                            }
                        }
                    }
                }
                other if FEATURES.contains(&other) => {
                    *features.entry(other.to_owned()).or_default() += 1;
                }
                _ => {}
            }
        }
    }
    let mut out = format!("sessions: {sessions}\n");
    match startup_sum.checked_div(startup_n) {
        Some(avg) => out += &format!("avg startup: {avg} ms\n"),
        None => out += "avg startup: n/a\n",
    }
    out += "files opened:\n";
    for (k, n) in &opens {
        out += &format!("  {k}: {n}\n");
    }
    out += "features:\n";
    for (k, n) in &features {
        out += &format!("  {k}: {n}\n");
    }
    out += "time per format:\n";
    for (k, (v, e)) in &modes {
        out += &format!("  {k}: view {v:.0}s, edit {e:.0}s\n");
    }
    out
}

pub fn clear(dir: &Path) {
    let _ = std::fs::remove_file(dir.join(LOG));
    let _ = std::fs::remove_file(dir.join(LOG_OLD));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tt-tel-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn panic_is_logged_truncated_when_enabled_only() {
        let d = tmp("panic");
        log_panic(&d, &"x".repeat(1000), true);
        let body = fs::read_to_string(d.join("usage.jsonl")).unwrap();
        let v: Value = serde_json::from_str(body.trim()).unwrap();
        assert_eq!(v["e"], "problem");
        assert_eq!(v["kind"], "panic");
        assert!(body.len() <= MAX_LINE + 1);
        let d2 = tmp("panic-off");
        log_panic(&d2, "boom", false);
        assert!(!d2.join("usage.jsonl").exists());
    }

    #[test]
    fn size_buckets() {
        assert_eq!(size_bucket(0), "<1k");
        assert_eq!(size_bucket(1023), "<1k");
        assert_eq!(size_bucket(1024), "<10k");
        assert_eq!(size_bucket(50_000), "<100k");
        assert_eq!(size_bucket(500_000), "<1m");
        assert_eq!(size_bucket(3_000_000), "<5m");
        assert_eq!(size_bucket(9_000_000), ">=5m");
    }

    #[test]
    fn trunc_is_boundary_safe() {
        assert_eq!(trunc_str("abc", 10), "abc");
        assert_eq!(trunc_str("\u{e9}\u{e9}\u{e9}", 3), "\u{e9}");
        assert_eq!(trunc_str("\u{65e5}\u{672c}", 4), "\u{65e5}");
        assert_eq!(trunc_str("abc", 0), "");
    }

    #[test]
    fn event_line_is_json_and_bounded() {
        let l = event_line(100, "file_open", json!({"format": "json", "ms": 12}));
        let v: Value = serde_json::from_str(&l).unwrap();
        assert_eq!(v["t"], 100);
        assert_eq!(v["e"], "file_open");
        assert_eq!(v["format"], "json");
        let big = "x".repeat(5000);
        let l = event_line(1, "problem", json!({"msg": big, "a": big, "b": big}));
        assert!(l.len() <= MAX_LINE, "{}", l.len());
        assert!(serde_json::from_str::<Value>(&l).is_ok());
    }

    #[test]
    fn path_token_hides_name_and_depends_on_salt() {
        let p = Path::new("C:/secret/project/prod.env");
        let a = path_token(p, "s1");
        assert!(!a.contains("secret") && !a.contains("prod") && !a.contains("project"));
        assert!(a.starts_with("env:"));
        assert_eq!(a, path_token(p, "s1"));
        assert_ne!(a, path_token(p, "s2"));
        assert!(
            path_token(Path::new("noext"), "s").starts_with(":")
                || path_token(Path::new("noext"), "s").starts_with("none:")
        );
    }

    #[test]
    fn rotate_over_cap_replaces_old_rotation() {
        let d = tmp("rot");
        let now = 10_000_000;
        fs::write(d.join("usage.1.jsonl"), "{\"t\":1,\"e\":\"older\"}\n").unwrap();
        let line = format!(
            "{{\"t\":{},\"e\":\"x\",\"pad\":\"{}\"}}\n",
            now,
            "p".repeat(200)
        );
        fs::write(d.join("usage.jsonl"), line.repeat(20)).unwrap();
        rotate(&d, 1, 90, now);
        assert!(!d.join("usage.jsonl").exists());
        let r = fs::read_to_string(d.join("usage.1.jsonl")).unwrap();
        assert_eq!(r.lines().count(), 20);
        assert!(!r.contains("older"));
    }

    #[test]
    fn rotate_under_cap_untouched() {
        let d = tmp("rot-small");
        fs::write(d.join("usage.jsonl"), "{\"t\":1,\"e\":\"x\"}\n").unwrap();
        rotate(&d, 1, 90, 5_000_000);
        assert!(d.join("usage.jsonl").exists());
        assert!(!d.join("usage.1.jsonl").exists());
    }

    #[test]
    fn rotate_drops_old_and_torn_lines() {
        let d = tmp("rot-age");
        let now = 100 * 86_400;
        let old = format!("{{\"t\":{},\"e\":\"old\"}}\n", now - 91 * 86_400);
        let new = format!(
            "{{\"t\":{},\"e\":\"new\",\"pad\":\"{}\"}}\n",
            now - 86_400,
            "p".repeat(2000)
        );
        let body = format!("{}{}{}{{\"t\":", old, new, "garbage\n");
        fs::write(d.join("usage.jsonl"), body).unwrap();
        rotate(&d, 1, 90, now);
        let r = fs::read_to_string(d.join("usage.1.jsonl")).unwrap();
        assert!(r.contains("new") && !r.contains("old") && !r.contains("garbage"));
    }

    #[test]
    fn stats_skips_garbage_and_aggregates() {
        let d = tmp("stats");
        let body = [
            r#"{"t":1,"e":"session_start","startup_ms":100}"#,
            r#"{"t":2,"e":"session_start","startup_ms":300}"#,
            r#"{"t":3,"e":"file_open","format":"json"}"#,
            r#"{"t":4,"e":"file_open","format":"json"}"#,
            r#"{"t":5,"e":"file_open","format":"env"}"#,
            r#"{"t":6,"e":"save"}"#,
            "not json at all",
            r#"{"t":7,"e":"session_summary","view_secs":{"json":10.0},"edit_secs":{"json":5.0,"env":20.0}}"#,
            r#"{"t":8,"e":"sav"#,
        ]
        .join("\n");
        fs::write(d.join("usage.jsonl"), body).unwrap();
        let s = stats(&d);
        assert!(s.contains("sessions: 2"), "{s}");
        assert!(s.contains("avg startup: 200"), "{s}");
        assert!(s.contains("json: 2"), "{s}");
        assert!(s.contains("env: 1"), "{s}");
        assert!(s.contains("save: 1"), "{s}");
        assert!(s.contains("view 10s"), "{s}");
    }

    #[test]
    fn stats_on_missing_dir_does_not_panic() {
        let s = stats(Path::new("Z:/definitely/not/here"));
        assert!(s.contains("sessions: 0"));
    }

    #[test]
    fn clear_removes_both_files() {
        let d = tmp("clear");
        fs::write(d.join("usage.jsonl"), "x").unwrap();
        fs::write(d.join("usage.1.jsonl"), "x").unwrap();
        clear(&d);
        assert!(!d.join("usage.jsonl").exists() && !d.join("usage.1.jsonl").exists());
    }

    #[test]
    fn enabled_writes_events_and_summary() {
        let d = tmp("write");
        let t = init_in(d.clone(), &Settings::default());
        t.add_mode_time("json", true, 2.5);
        t.event("save", json!({"format": "json"}));
        t.shutdown();
        let body = fs::read_to_string(d.join("usage.jsonl")).unwrap();
        assert!(body.contains("\"e\":\"save\""));
        assert!(body.contains("session_summary") && body.contains("view_secs"));
        assert!(!body.contains(d.to_string_lossy().as_ref()));
    }

    #[test]
    fn disabled_writes_nothing() {
        let d = tmp("off");
        let s = Settings {
            telemetry: false,
            ..Default::default()
        };
        let t = init_in(d.clone(), &s);
        t.event("save", json!({}));
        t.shutdown();
        assert!(!t.enabled());
        assert_eq!(fs::read_dir(&d).unwrap().count(), 0);
    }

    #[test]
    fn unwritable_dir_does_not_panic() {
        let d = tmp("bad");
        let blocker = d.join("file");
        fs::write(&blocker, "x").unwrap();
        let t = init_in(blocker.join("sub"), &Settings::default());
        t.event("save", json!({}));
        t.shutdown();
    }
}
