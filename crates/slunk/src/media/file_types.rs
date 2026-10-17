//! A file's type, from its extension.

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

use super::paths;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileType {
    Music,
    Image,
    Other,
}

const MUSIC: &[&str] = &[
    "aac", "aif", "aiff", "alac", "ape", "dff", "dsf", "flac", "m4a", "mp3", "mpc", "oga", "ogg",
    "opus", "wav", "wma", "wv",
];

const IMAGES: &[&str] = &[
    "avif", "bmp", "gif", "heic", "jpeg", "jpg", "png", "svg", "tif", "tiff", "webp",
];

impl FileType {
    /// As the database stores it.
    pub fn as_str(self) -> &'static str {
        match self {
            FileType::Music => "music",
            FileType::Image => "image",
            FileType::Other => "other",
        }
    }

    pub fn parse(text: &str) -> Option<FileType> {
        match text {
            "music" => Some(FileType::Music),
            "image" => Some(FileType::Image),
            "other" => Some(FileType::Other),
            _ => None,
        }
    }
}

/// A file's type, from its extension. Anything unknown is `Other`.
pub fn file_type_of(path: &str) -> FileType {
    // Lowercased as JavaScript's toLowerCase() does
    let extension = paths::extname(path).trim_start_matches('.').to_lowercase();
    if MUSIC.contains(&extension.as_str()) {
        FileType::Music
    } else if IMAGES.contains(&extension.as_str()) {
        FileType::Image
    } else {
        FileType::Other
    }
}

impl ToSql for FileType {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

impl FromSql for FileType {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let text = value.as_str()?;
        FileType::parse(text)
            .ok_or_else(|| FromSqlError::Other(format!("not a file type: {text}").into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_come_from_the_extension_ignoring_case() {
        for (path, expected) in [
            ("Ryo Fukui/01 Early Summer.flac", FileType::Music),
            ("a/B.MP3", FileType::Music),
            ("a/cover.JPG", FileType::Image),
            ("a/scan.tiff", FileType::Image),
            ("a/rip.log", FileType::Other),
            ("a/notes", FileType::Other),
            // A dot that starts the name isn't an extension
            ("a/.flac", FileType::Other),
            ("a/..flac", FileType::Music),
            ("a.flac/notes", FileType::Other),
        ] {
            assert_eq!(file_type_of(path), expected, "{path}");
        }
    }
}
