use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Markdown,
    Env,
    Config,
    Plain,
}

pub fn detect(path: &Path) -> Format {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name == ".env" || name.starts_with(".env.") || name.ends_with(".env") {
        return Format::Env;
    }
    match name.rsplit_once('.').map(|(_, e)| e) {
        Some("json") => Format::Json,
        Some("md" | "markdown") => Format::Markdown,
        Some("ini" | "toml" | "yaml" | "yml" | "conf" | "cfg" | "config" | "properties") => {
            Format::Config
        }
        _ => Format::Plain,
    }
}

impl Format {
    pub fn name(&self) -> &'static str {
        match self {
            Format::Json => "json",
            Format::Markdown => "markdown",
            Format::Env => "env",
            Format::Config => "config",
            Format::Plain => "plain",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Format {
        detect(Path::new(s))
    }

    #[test]
    fn json_and_markdown() {
        assert_eq!(d("a.json"), Format::Json);
        assert_eq!(d("a.md"), Format::Markdown);
        assert_eq!(d("a.markdown"), Format::Markdown);
        assert_eq!(d("A.JSON"), Format::Json);
    }

    #[test]
    fn env_variants() {
        assert_eq!(d(".env"), Format::Env);
        assert_eq!(d(".env.local"), Format::Env);
        assert_eq!(d("prod.env"), Format::Env);
        assert_eq!(d("C:/x/.ENV"), Format::Env);
    }

    #[test]
    fn config_variants() {
        for f in [
            "a.ini",
            "a.toml",
            "a.yaml",
            "a.yml",
            "a.conf",
            "a.cfg",
            "a.config",
            "a.properties",
        ] {
            assert_eq!(d(f), Format::Config, "{f}");
        }
    }

    #[test]
    fn plain_fallback() {
        assert_eq!(d("a.txt"), Format::Plain);
        assert_eq!(d("README"), Format::Plain);
        assert_eq!(d(""), Format::Plain);
    }
}
