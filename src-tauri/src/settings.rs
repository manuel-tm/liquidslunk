//! The settings file (backend.md, "Choosing the library"): the library to
//! open at launch, the recent ones, and how many backups to keep. Rust owns
//! it: the webview never reads it, it only calls commands.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use slunk::repo::backups;

/// The file's name in the app's config folder. Debug builds keep their own,
/// so `npm run tauri dev` never opens the library the installed app uses
/// (getting-started.md, "Running the app").
pub const FILE: &str = if cfg!(debug_assertions) {
    "settings.dev.json"
} else {
    "settings.json"
};

/// How many recent libraries are remembered.
const RECENT: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub version: u32,
    /// The library's database, as an absolute path.
    #[serde(default)]
    pub library: Option<PathBuf>,
    /// The last libraries opened, newest first.
    #[serde(default)]
    pub recent: Vec<PathBuf>,
    /// How many backups `slunk-backups/` keeps: a whole number from 1.
    #[serde(default = "default_keep_backups")]
    pub keep_backups: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            library: None,
            recent: Vec::new(),
            keep_backups: backups::DEFAULT_KEEP,
        }
    }
}

fn default_keep_backups() -> u32 {
    backups::DEFAULT_KEEP
}

impl Settings {
    /// The settings at `path`. A file that's missing or can't be read gives
    /// the defaults, so the app still starts, and asks for a library.
    pub fn load(path: &Path) -> Settings {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Settings::default(),
            Err(error) => {
                eprintln!(
                    "Can't read {}, so using the defaults: {error}",
                    path.display()
                );
                return Settings::default();
            }
        };
        match serde_json::from_str::<Settings>(&text) {
            Ok(mut settings) => {
                if settings.keep_backups < 1 {
                    settings.keep_backups = backups::DEFAULT_KEEP;
                }
                settings
            }
            Err(error) => {
                eprintln!(
                    "Can't read {}, so using the defaults: {error}",
                    path.display()
                );
                Settings::default()
            }
        }
    }

    /// Writes the settings atomically: a temporary file, then a rename.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder)?;
        }
        let temporary = path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        fs::write(&temporary, text + "\n")?;
        fs::rename(&temporary, path)
    }

    /// Makes `library` the one to open at launch, and the newest recent one.
    pub fn remember(&mut self, library: &Path) {
        self.library = Some(library.to_path_buf());
        self.recent.retain(|recent| recent != library);
        self.recent.insert(0, library.to_path_buf());
        self.recent.truncate(RECENT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remembers_the_last_five_newest_first() {
        let mut settings = Settings::default();
        for name in ["a", "b", "c", "d", "e", "f", "c"] {
            settings.remember(Path::new(name));
        }
        assert_eq!(settings.library.as_deref(), Some(Path::new("c")));
        let recent: Vec<&str> = settings
            .recent
            .iter()
            .map(|p| p.to_str().unwrap())
            .collect();
        assert_eq!(recent, ["c", "f", "e", "d", "b"]);
    }

    #[test]
    fn saves_and_loads_the_file_of_design_6_2() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config").join(FILE);
        assert_eq!(Settings::load(&path), Settings::default());

        let mut settings = Settings::default();
        settings.remember(Path::new("/Users/me/Music/slunk.db"));
        settings.save(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "version": 1,
                "library": "/Users/me/Music/slunk.db",
                "recent": ["/Users/me/Music/slunk.db"],
                "keepBackups": 10
            })
        );
        assert_eq!(Settings::load(&path), settings);
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn a_bad_file_gives_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE);
        fs::write(&path, "{ not json").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
        fs::write(&path, r#"{ "version": 1, "keepBackups": 0 }"#).unwrap();
        assert_eq!(Settings::load(&path).keep_backups, 10);
    }
}
