//! The catalog context's commands (slunk's `catalog.graphql`). Ported so far:
//! artists, labels, the lists the user manages, the languages in use (phase
//! 2), and releases (phase 4).
//!
//! - `artists`, `labels`, `lists` and `releases` each hold their commands
//!   and DTOs, and the plain functions behind them, which the write parity
//!   script calls too, like `artists::create(library, args)`.
//! - This module holds what they share: arguments, the DTOs of what a
//!   release refers to, and `Related`, which loads it.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use slunk::catalog as domain;
use slunk::media::FileRecord;
use slunk::paging::ListArgs;
use specta::Type;
use tauri::AppHandle;

use super::media::File;
use super::on_library;
use crate::error::CommandError;
use crate::library::Library;

pub mod artists;
pub mod labels;
pub mod lists;
pub mod releases;

/// Pickers offer the first 20 matches (slunk's design §7.3).
pub(crate) const OPTIONS: i64 = 20;

/// A page of a list: artists, labels, formats or media.
#[derive(Debug, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct PageArgs {
    /// From 1. Default 1.
    #[specta(optional)]
    pub page: Option<i64>,
    /// Default 50, at most 200
    #[specta(optional)]
    pub per_page: Option<i64>,
    /// `name` or `id`, with `-` in front for descending. Default `-id`,
    /// newest first.
    #[specta(optional)]
    pub sort: Option<String>,
    /// Part of the name, ignoring case. For artists and labels, part of an
    /// alternate name finds them too.
    #[specta(optional)]
    pub q: Option<String>,
}

impl From<PageArgs> for ListArgs {
    fn from(args: PageArgs) -> Self {
        ListArgs {
            page: args.page,
            per_page: args.per_page,
            sort: args.sort,
            q: args.q,
        }
    }
}

/// What a picker searches: the first matches by name.
#[derive(Debug, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct OptionsArgs {
    #[specta(optional)]
    pub q: Option<String>,
}

impl OptionsArgs {
    pub(crate) fn list(self) -> ListArgs {
        ListArgs {
            per_page: Some(OPTIONS),
            sort: Some("name".into()),
            q: self.q,
            ..ListArgs::default()
        }
    }
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct IdArgs {
    pub id: i64,
}

/// A file by id, or a path to register first.
#[derive(Debug, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct FileRefInput {
    #[specta(optional)]
    pub file_id: Option<i64>,
    /// Relative to the database's folder, or absolute
    #[specta(optional)]
    pub path: Option<String>,
}

impl From<FileRefInput> for domain::FileRef {
    fn from(file: FileRefInput) -> Self {
        domain::FileRef {
            file_id: file.file_id,
            path: file.path,
        }
    }
}

/// A file to link, by id or by a path to register first, with a note.
#[derive(Debug, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct FileLinkInput {
    #[specta(optional)]
    pub file_id: Option<i64>,
    #[specta(optional)]
    pub path: Option<String>,
    /// Trimmed. Blank means none.
    #[specta(optional)]
    pub note: Option<String>,
}

impl From<FileLinkInput> for domain::FileLinkInput {
    fn from(link: FileLinkInput) -> Self {
        domain::FileLinkInput {
            file: domain::FileRef {
                file_id: link.file_id,
                path: link.path,
            },
            note: link.note,
        }
    }
}

/// Sets only a picture or logo.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SetPictureArgs {
    pub id: i64,
    /// A file given by path is registered first, in the same transaction.
    pub picture: FileRefInput,
}

/// A file linked to an artist, with its note.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileLink {
    pub file: File,
    pub note: Option<String>,
}

/// Language tags in use on names and titles, the most used first, for the
/// language picker.
#[tauri::command]
#[specta::specta]
pub async fn languages(app: AppHandle) -> Result<Vec<String>, CommandError> {
    on_library(app, in_use).await
}

pub fn in_use(library: &Library) -> Result<Vec<String>, CommandError> {
    Ok(library.catalog.languages()?)
}

/// Registered files by id, as DTOs.
fn files_by_id(
    library: &Library,
    ids: &[i64],
) -> Result<std::collections::HashMap<i64, File>, CommandError> {
    let files: Vec<FileRecord> = library.media.files_by_ids(ids)?;
    Ok(files
        .iter()
        .map(|file| (file.id, File::from(file)))
        .collect())
}

/// A title besides the main one, of a release, a track or a disc (slunk's
/// 3_alternate_names.md §4.2).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AlternateTitle {
    pub title: String,
    /// A BCP 47 tag. Null when it isn't known.
    pub language: Option<String>,
    /// On the release as published. False for a title added here, like a
    /// translation.
    pub printed: bool,
}

impl From<domain::AlternateTitle> for AlternateTitle {
    fn from(title: domain::AlternateTitle) -> Self {
        AlternateTitle {
            title: title.title,
            language: title.language,
            printed: title.printed,
        }
    }
}

/// A name, and its language.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NameRef {
    pub name: String,
    pub language: Option<String>,
}

/// An artist a link names.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArtistRef {
    pub id: i64,
    /// The main name
    pub name: String,
    pub name_language: Option<String>,
}

/// An artist a link names, with its other names, in order, for a page that
/// shows a title's artists in the title's language (slunk's
/// 3_alternate_names.md §7.2).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NamedArtistRef {
    #[serde(flatten)]
    pub artist: ArtistRef,
    pub alternate_names: Vec<NameRef>,
}

/// An artist as credited on a release, a track or a credit line (slunk's
/// 3_alternate_names.md §4.2).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CreditedArtist {
    pub artist: ArtistRef,
    /// The name printed, when it isn't the artist's main name
    pub credited_as: Option<NameRef>,
}

/// An artist as credited, with its alternate names.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NamedCreditedArtist {
    pub artist: NamedArtistRef,
    pub credited_as: Option<NameRef>,
}

/// A credit line: a role, as printed, and the artists who hold it, in
/// order.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Credit {
    pub role: String,
    pub artists: Vec<CreditedArtist>,
}

/// A release format or medium, by name.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NamedItem {
    pub id: i64,
    pub name: String,
}

/// A label, by name.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LabelRef {
    pub id: i64,
    pub name: String,
    pub name_language: Option<String>,
}

/// What a page's rows refer to: artists, the names they're credited with,
/// labels, formats and media, each loaded once, with one query per kind,
/// as slunk's DataLoaders did (design §5.4).
#[derive(Default)]
pub(crate) struct Related {
    artists: HashMap<i64, domain::Artist>,
    alternate_names: HashMap<i64, Vec<domain::AlternateName>>,
    credited_names: HashMap<i64, domain::AlternateName>,
    labels: HashMap<i64, domain::Label>,
    items: HashMap<(domain::ListKind, i64), domain::ListItem>,
}

impl Related {
    /// Loads the artists these links name, and the names they're credited
    /// with. With `with_names`, the artists' alternate names too.
    pub(crate) fn load_artists<'a>(
        &mut self,
        library: &Library,
        links: impl IntoIterator<Item = &'a domain::ArtistLink>,
        with_names: bool,
    ) -> Result<(), CommandError> {
        let mut artist_ids = Vec::new();
        let mut name_ids = Vec::new();
        for link in links {
            if !self.artists.contains_key(&link.artist_id) && !artist_ids.contains(&link.artist_id)
            {
                artist_ids.push(link.artist_id);
            }
            if let Some(name) = link.credited_name_id
                && !self.credited_names.contains_key(&name)
            {
                name_ids.push(name);
            }
        }
        let catalog = &library.catalog;
        for artist in catalog.artists_by_ids(&artist_ids)? {
            self.artists.insert(artist.id, artist);
        }
        for name in catalog.artist_names_by_ids(&name_ids)? {
            self.credited_names.insert(name.id, name);
        }
        if with_names {
            let missing: Vec<i64> = self
                .artists
                .keys()
                .filter(|id| !self.alternate_names.contains_key(id))
                .copied()
                .collect();
            let mut names = catalog.artist_names_of(&missing)?;
            for id in missing {
                self.alternate_names
                    .insert(id, names.remove(&id).unwrap_or_default());
            }
        }
        Ok(())
    }

    /// Loads labels by id.
    pub(crate) fn load_labels(
        &mut self,
        library: &Library,
        ids: &[i64],
    ) -> Result<(), CommandError> {
        let missing: Vec<i64> = ids
            .iter()
            .filter(|id| !self.labels.contains_key(id))
            .copied()
            .collect();
        for label in library.catalog.labels_by_ids(&missing)? {
            self.labels.insert(label.id, label);
        }
        Ok(())
    }

    /// Loads formats or media by id.
    pub(crate) fn load_items(
        &mut self,
        library: &Library,
        kind: domain::ListKind,
        ids: &[i64],
    ) -> Result<(), CommandError> {
        let missing: Vec<i64> = ids
            .iter()
            .filter(|id| !self.items.contains_key(&(kind, **id)))
            .copied()
            .collect();
        for item in library.catalog.list_items_by_ids(kind, &missing)? {
            self.items.insert((kind, item.id), item);
        }
        Ok(())
    }

    /// An artist as credited.
    pub(crate) fn credited(&self, link: &domain::ArtistLink) -> Option<CreditedArtist> {
        let artist = self.artists.get(&link.artist_id)?;
        Some(CreditedArtist {
            artist: ArtistRef {
                id: artist.id,
                name: artist.name.clone(),
                name_language: artist.name_language.clone(),
            },
            credited_as: link
                .credited_name_id
                .and_then(|id| self.credited_names.get(&id))
                .map(name_ref),
        })
    }

    /// Artists as credited, in order.
    pub(crate) fn credited_all(&self, links: &[domain::ArtistLink]) -> Vec<CreditedArtist> {
        links
            .iter()
            .filter_map(|link| self.credited(link))
            .collect()
    }

    /// Artists as credited, in order, with the alternate names
    /// `load_artists()` loaded with them.
    pub(crate) fn credited_named(&self, links: &[domain::ArtistLink]) -> Vec<NamedCreditedArtist> {
        self.credited_all(links)
            .into_iter()
            .map(|credited| NamedCreditedArtist {
                artist: NamedArtistRef {
                    alternate_names: self
                        .alternate_names
                        .get(&credited.artist.id)
                        .map(|names| names.iter().map(name_ref).collect())
                        .unwrap_or_default(),
                    artist: credited.artist,
                },
                credited_as: credited.credited_as,
            })
            .collect()
    }

    /// Credit lines, with their artists as credited.
    pub(crate) fn credits(&self, credits: &[domain::Credit]) -> Vec<Credit> {
        credits
            .iter()
            .map(|credit| Credit {
                role: credit.role.clone(),
                artists: self.credited_all(&credit.artists),
            })
            .collect()
    }

    pub(crate) fn labels(&self, ids: &[i64]) -> Vec<LabelRef> {
        ids.iter()
            .filter_map(|id| self.labels.get(id))
            .map(|label| LabelRef {
                id: label.id,
                name: label.name.clone(),
                name_language: label.name_language.clone(),
            })
            .collect()
    }

    pub(crate) fn items(&self, kind: domain::ListKind, ids: &[i64]) -> Vec<NamedItem> {
        ids.iter()
            .filter_map(|id| self.items.get(&(kind, *id)))
            .map(|item| NamedItem {
                id: item.id,
                name: item.name.clone(),
            })
            .collect()
    }
}

fn name_ref(name: &domain::AlternateName) -> NameRef {
    NameRef {
        name: name.name.clone(),
        language: name.language.clone(),
    }
}
