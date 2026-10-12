//! Font choices: egui's bundled fonts plus a short list of fonts that ship with Windows.
//! System fonts are read from disk only when selected, so startup cost is unchanged.

use eframe::egui::{FontData, FontDefinitions, FontFamily};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct Face {
    pub id: &'static str,
    pub label: &'static str,
    pub file: &'static str,
    pub mono: bool,
}

const fn face(id: &'static str, label: &'static str, file: &'static str, mono: bool) -> Face {
    Face {
        id,
        label,
        file,
        mono,
    }
}

pub const FACES: [Face; 12] = [
    face("consolas", "Consolas", "consola.ttf", true),
    face("cascadia-code", "Cascadia Code", "CascadiaCode.ttf", true),
    face("cascadia-mono", "Cascadia Mono", "CascadiaMono.ttf", true),
    face("courier-new", "Courier New", "cour.ttf", true),
    face("lucida-console", "Lucida Console", "lucon.ttf", true),
    face("segoe-ui", "Segoe UI", "segoeui.ttf", false),
    face("arial", "Arial", "arial.ttf", false),
    face("calibri", "Calibri", "calibri.ttf", false),
    face("verdana", "Verdana", "verdana.ttf", false),
    face("tahoma", "Tahoma", "tahoma.ttf", false),
    face("georgia", "Georgia", "georgia.ttf", false),
    face("times-new-roman", "Times New Roman", "times.ttf", false),
];

/// Every value `editor_font` / `view_font` may hold.
pub const IDS: [&str; 14] = [
    "monospace",
    "proportional",
    "consolas",
    "cascadia-code",
    "cascadia-mono",
    "courier-new",
    "lucida-console",
    "segoe-ui",
    "arial",
    "calibri",
    "verdana",
    "tahoma",
    "georgia",
    "times-new-roman",
];

pub fn system_dir() -> PathBuf {
    let win =
        std::env::var_os("WINDIR").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
    win.join("Fonts")
}

/// Display name for a stored font id.
pub fn label(id: &str) -> &str {
    match id {
        "monospace" => "Built-in monospace",
        "proportional" => "Built-in proportional",
        _ => FACES.iter().find(|f| f.id == id).map_or(id, |f| f.label),
    }
}

/// Faces whose font file exists in `dir`.
pub fn available(dir: &Path) -> Vec<&'static Face> {
    FACES
        .iter()
        .filter(|f| dir.join(f.file).is_file())
        .collect()
}

/// egui font definitions: the defaults plus one named family per wanted font that could be read.
/// Each named family falls back to the bundled fonts, so missing glyphs still render.
/// Returns the ids that were actually loaded.
pub fn definitions(dir: &Path, wanted: &[&str]) -> (FontDefinitions, Vec<String>) {
    let mut defs = FontDefinitions::default();
    let mut loaded: Vec<String> = Vec::new();
    for id in wanted {
        let Some(f) = FACES.iter().find(|f| f.id == *id) else {
            continue;
        };
        if loaded.iter().any(|l| l == id) {
            continue;
        }
        let Ok(bytes) = std::fs::read(dir.join(f.file)) else {
            continue;
        };
        let base = if f.mono {
            FontFamily::Monospace
        } else {
            FontFamily::Proportional
        };
        let mut chain = vec![f.id.to_string()];
        chain.extend(defs.families.get(&base).cloned().unwrap_or_default());
        defs.font_data
            .insert(f.id.to_string(), Arc::new(FontData::from_owned(bytes)));
        defs.families.insert(FontFamily::Name(f.id.into()), chain);
        loaded.push(f.id.to_string());
    }
    (defs, loaded)
}

/// The family to draw with for a stored id; falls back to the bundled font of the same kind
/// when the font was not loaded.
pub fn family(id: &str, loaded: &[String]) -> FontFamily {
    if loaded.iter().any(|l| l == id) {
        return FontFamily::Name(id.into());
    }
    match FACES.iter().find(|f| f.id == id) {
        Some(f) if !f.mono => FontFamily::Proportional,
        None if id == "proportional" => FontFamily::Proportional,
        _ => FontFamily::Monospace,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir_with(files: &[&str]) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tt-fonts-{}", files.join("-")));
        std::fs::create_dir_all(&d).unwrap();
        for f in files {
            std::fs::write(d.join(f), b"not a real font").unwrap();
        }
        d
    }

    #[test]
    fn ids_match_faces() {
        assert_eq!(&IDS[..2], ["monospace", "proportional"]);
        for (id, f) in IDS[2..].iter().zip(FACES.iter()) {
            assert_eq!(*id, f.id);
        }
        assert_eq!(IDS.len(), FACES.len() + 2);
    }

    #[test]
    fn only_files_that_exist_are_available() {
        let d = dir_with(&["consola.ttf", "arial.ttf"]);
        let ids: Vec<_> = available(&d).iter().map(|f| f.id).collect();
        assert_eq!(ids, ["consolas", "arial"]);
        assert!(available(&d.join("missing")).is_empty());
    }

    #[test]
    fn definitions_load_wanted_fonts_with_bundled_fallback() {
        let d = dir_with(&["consola.ttf", "arial.ttf", "fallback-check.ttf"]);
        let (defs, loaded) = definitions(
            &d,
            &["consolas", "consolas", "georgia", "nonsense", "arial"],
        );
        assert_eq!(
            loaded,
            ["consolas", "arial"],
            "dupes, missing files and unknown ids skipped"
        );
        let mono = &defs.families[&FontFamily::Name("consolas".into())];
        assert_eq!(mono[0], "consolas");
        assert!(mono.len() > 1, "keeps the bundled fonts as fallback");
        assert!(
            !defs
                .families
                .contains_key(&FontFamily::Name("georgia".into()))
        );
    }

    #[test]
    fn family_falls_back_to_the_same_kind_when_not_loaded() {
        let loaded = vec!["consolas".to_string()];
        assert_eq!(
            family("consolas", &loaded),
            FontFamily::Name("consolas".into())
        );
        assert_eq!(family("courier-new", &loaded), FontFamily::Monospace);
        assert_eq!(family("arial", &loaded), FontFamily::Proportional);
        assert_eq!(family("proportional", &[]), FontFamily::Proportional);
        assert_eq!(family("monospace", &[]), FontFamily::Monospace);
        assert_eq!(family("junk", &[]), FontFamily::Monospace);
    }
}
