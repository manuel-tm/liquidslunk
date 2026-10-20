//! The external libraries context's commands (4_external_libraries.md §10).
//! So far, the track page's: a track's tags as an external library writes
//! them, and the path a template gives it (phase 0).

use serde::{Deserialize, Serialize};
use slunk::external::{DEFAULT_TEMPLATE, Template};
use specta::Type;
use tauri::AppHandle;

use super::catalog::files_by_id;
use super::media::File;
use super::on_library;
use crate::error::CommandError;
use crate::library::Library;

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportedTagsArgs {
    /// A track
    pub id: i64,
    /// A path template. Default: the default template.
    #[serde(default)]
    #[specta(optional)]
    pub template: Option<String>,
}

/// A track's tags, as every external library writes them, whatever its
/// format (4_external_libraries.md §4.1). A field with no value isn't
/// written: it's null or empty.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportedTags {
    /// With its features: "Bridges (feat. Mari, Sander)"
    pub title: String,
    /// Who performs it, as credited, joined with ", "
    pub artist: Option<String>,
    /// Each performer, then each feature, as credited
    pub artists: Vec<String>,
    /// The release's title
    pub album: String,
    /// The release's artists, as credited, joined with ", "
    pub album_artist: Option<String>,
    /// A various-artists release
    pub compilation: bool,
    /// `2006`, `2006-01` or `2006-01-10`
    pub date: Option<String>,
    pub track_number: i64,
    /// The last track number on its disc
    pub track_total: i64,
    pub disc_number: i64,
    /// The release's last disc
    pub disc_total: i64,
    pub disc_title: Option<String>,
    pub labels: Vec<String>,
    pub catalog_number: Option<String>,
    pub media: Vec<String>,
    /// The release's formats
    pub release_types: Vec<String>,
    /// What players sort the artist by, when it isn't in Latin script
    pub artist_sort: Option<String>,
    pub album_artist_sort: Option<String>,
    pub album_sort: Option<String>,
    pub title_sort: Option<String>,
    /// The release's cover
    pub cover: Option<File>,
    /// The template the path comes from
    pub template: String,
    /// Where a folder library that keeps the original files puts the track,
    /// by the template
    pub path: String,
}

/// A track's tags as an external library writes them, and its path by a
/// template, the default one unless one is given, or null for an unknown id.
/// A template that can't be used is `invalid`, with the reason.
#[tauri::command]
#[specta::specta]
pub async fn exported_tags(
    app: AppHandle,
    args: ExportedTagsArgs,
) -> Result<Option<ExportedTags>, CommandError> {
    on_library(app, move |library| exported(library, args)).await
}

pub fn exported(
    library: &Library,
    args: ExportedTagsArgs,
) -> Result<Option<ExportedTags>, CommandError> {
    let template = args.template.unwrap_or_else(|| DEFAULT_TEMPLATE.into());
    let parsed = Template::parse(&template)?;
    let Some(exported) = library.external.exported(args.id, &parsed)? else {
        return Ok(None);
    };
    let tags = exported.tags;
    let cover = match tags.cover {
        Some(id) => files_by_id(library, &[id])?.remove(&id),
        None => None,
    };
    Ok(Some(ExportedTags {
        title: tags.title,
        artist: tags.artist,
        artists: tags.artists,
        album: tags.album,
        album_artist: tags.album_artist,
        compilation: tags.compilation,
        date: tags.date,
        track_number: tags.track_number,
        track_total: tags.track_total,
        disc_number: tags.disc_number,
        disc_total: tags.disc_total,
        disc_title: tags.disc_title,
        labels: tags.labels,
        catalog_number: tags.catalog_number,
        media: tags.media,
        release_types: tags.release_types,
        artist_sort: tags.sort.artist,
        album_artist_sort: tags.sort.album_artist,
        album_sort: tags.sort.album,
        title_sort: tags.sort.title,
        cover,
        template,
        path: exported.path,
    }))
}

#[cfg(test)]
mod tests {
    use slunk::repo::{connection, migrator};

    use super::*;

    const HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    #[test]
    fn exported_tags_come_with_the_cover_and_the_templates_path() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("slunk.db");
        migrator::migrate(&db, 10).unwrap();
        let library = Library::new(&db, connection::open(&db).unwrap());
        library
            .store
            .conn()
            .execute_batch(&format!(
                r#"
                insert into files (id, path, type, size_bytes, mtime, sha256) values
                    (1, 'Scenery/01.flac', 'music', 1, 0, '{HASH}'),
                    (2, 'Scenery/cover.jpg', 'image', 1, 0, '{HASH}');
                insert into artists (id, name) values (1, 'Ryo Fukui');
                insert into releases (id, title, cover_picture, various_artists)
                    values (1, 'Scenery', 2, 0);
                insert into artists_releases (artist_id, release_id, position) values (1, 1, 0);
                insert into tracks (id, title, track_number, cd_number, release_id, file_id)
                    values (1, 'It Could Happen to You', 1, 1, 1, 1);
                "#
            ))
            .unwrap();
        let args = |template: Option<&str>| ExportedTagsArgs {
            id: 1,
            template: template.map(str::to_string),
        };

        let tags = exported(&library, args(None)).unwrap().unwrap();
        assert_eq!(tags.artist.as_deref(), Some("Ryo Fukui"));
        assert_eq!(
            tags.cover.map(|cover| cover.path).as_deref(),
            Some("Scenery/cover.jpg")
        );
        assert_eq!(tags.template, DEFAULT_TEMPLATE);
        assert_eq!(
            tags.path,
            "Ryo Fukui/Scenery/01 It Could Happen to You.flac"
        );

        let tags = exported(&library, args(Some("${track_title}.${format}")))
            .unwrap()
            .unwrap();
        assert_eq!(tags.path, "It Could Happen to You.flac");

        let error = exported(&library, args(Some("${genre}.${format}"))).unwrap_err();
        assert_eq!(error.message, "${genre} isn't a template variable");

        let unknown = ExportedTagsArgs {
            id: 99,
            template: None,
        };
        assert!(exported(&library, unknown).unwrap().is_none());
    }
}
