//! What the launcher remembers: the reader's choice and the last remote URL,
//! a small JSON file in the app's config directory.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Where the app opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Choice {
    /// menger in-process, its tickets on this device.
    Local,
    /// A menger at [`Settings::remote_url`].
    Remote,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// The remembered choice; `None` shows the launcher on start.
    #[serde(default)]
    pub choice: Option<Choice>,
    /// The last remote URL the reader opened.
    #[serde(default)]
    pub remote_url: Option<String>,
}

impl Settings {
    pub fn file(config_dir: &Path) -> PathBuf {
        config_dir.join("settings.json")
    }

    /// The settings at `path`; defaults when the file is missing or unreadable
    /// (an unreadable file is logged, never fatal).
    pub fn load(path: &Path) -> Settings {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                tracing::warn!("ignoring unreadable settings {}: {e}", path.display());
                Settings::default()
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Settings::default(),
            Err(e) => {
                tracing::warn!("cannot read settings {}: {e}", path.display());
                Settings::default()
            }
        }
    }

    /// Writes the settings (through a temporary file, so a crash never leaves
    /// half a file).
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

/// A remote URL as the reader typed it, checked: http or https, with a host.
/// A missing scheme is taken as https (`menger.example` → `https://menger.example/`).
pub fn parse_remote_url(input: &str) -> Result<url::Url, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("Enter the address of a menger server.".into());
    }
    let with_scheme = if input.contains("://") {
        input.to_string()
    } else {
        format!("https://{input}")
    };
    let url =
        url::Url::parse(&with_scheme).map_err(|e| format!("“{input}” is not a URL ({e})."))?;
    match url.scheme() {
        "http" | "https" => {}
        other => {
            return Err(format!(
                "Only http:// and https:// addresses work here, not {other}://."
            ))
        }
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err(format!("“{input}” has no host."));
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_a_file() {
        let dir =
            std::env::temp_dir().join(format!("menger-desktop-settings-{}", std::process::id()));
        let path = Settings::file(&dir);
        assert_eq!(Settings::load(&path), Settings::default());
        let s = Settings {
            choice: Some(Choice::Remote),
            remote_url: Some("http://127.0.0.1:4451/".into()),
        };
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path), s);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn remote_urls_are_http_or_https() {
        assert_eq!(
            parse_remote_url("menger.example").unwrap().as_str(),
            "https://menger.example/"
        );
        assert_eq!(
            parse_remote_url(" http://127.0.0.1:4451/ ")
                .unwrap()
                .as_str(),
            "http://127.0.0.1:4451/"
        );
        assert!(parse_remote_url("ftp://x").is_err());
        assert!(parse_remote_url("file:///etc/passwd").is_err());
        assert!(parse_remote_url("").is_err());
    }
}
