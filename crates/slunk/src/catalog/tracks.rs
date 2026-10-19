//! Tracks: the track list, a track with everything saved with it, and a
//! release's whole track list with its disc titles (slunk's design §6).

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;

use super::credited::{CreditedNames, first_of_each};
use super::titles::{alternate_title_rows, save_alternate_titles};
use super::{
    ArtistLink, Catalog, CreditOwner, CreditedArtistInput, DiscTitle, DiscTitleInput, NewTrack,
    Release, ReleaseTrackInput, Track, TrackArtist, TrackChanges, TrackFilterArgs, TrackInput,
    TrackQuery, TrackSort, not_found,
};
use crate::languages::language_tag;
use crate::media::{LinkOwner, Media};
use crate::paging::{ListArgs, Page, paging_of, search_of, sort_of};
use crate::repo::titles::TitleOwner;
use crate::repo::{credits, files, releases, tracks};
use crate::tagging::{TagOwner, Tagging};
use crate::text::clean_text;
use crate::{Error, Result};

// Moves track numbers clear of the ones being saved, so renumbering a
// release never trips its unique (release, disc, number) index midway
const RENUMBER_OFFSET: i64 = 1_000_000;

impl Catalog {
    /// A page of tracks. The search matches the title or an alternate title,
    /// and the filters all have to match.
    pub fn list_tracks(&self, args: &ListArgs, filters: &TrackFilterArgs) -> Result<Page<Track>> {
        let query = TrackQuery {
            paging: paging_of(args)?,
            sort: sort_of(
                args.sort.as_deref(),
                &[("title", TrackSort::Title)],
                "Tracks",
            )?,
            search: search_of(args.q.as_deref()),
            release: filters.release,
            tags: filters.tags.clone(),
        };
        let (items, total) = tracks::list(&self.store.conn(), &query)?;
        Ok(Page { items, total })
    }

    pub fn get_track(&self, id: i64) -> Result<Option<Track>> {
        tracks::get(&self.store.conn(), id)
    }

    /// The track each of these files is the audio of, by file id.
    pub fn tracks_of_files(&self, file_ids: &[i64]) -> Result<HashMap<i64, Track>> {
        tracks::of_files(&self.store.conn(), file_ids)
    }

    /// Creates a track from a registered audio file, reading its duration,
    /// with its alternate titles, artists, credit lines, file links and tags,
    /// in one transaction.
    pub fn create_track(&self, input: &TrackInput) -> Result<Track> {
        let duration_ms = self.duration_of(input.file_id)?;
        let prepared = self.prepare_refs(input.files.iter().map(|link| &link.file))?;
        self.store.transaction(|tx| {
            let registered = Media::insert_prepared(tx, prepared)?;
            let track = tracks::insert(
                tx,
                &NewTrack {
                    title: clean_text(&input.title),
                    title_language: language_tag(
                        input.title_language.as_ref().and_then(Option::as_deref),
                    )?,
                    duration_ms,
                    track_number: input.track_number,
                    cd_number: input.cd_number,
                    release_id: input.release_id,
                    file_id: input.file_id,
                },
            )?;
            self.save_track_parts(tx, &track, input, &registered)?;
            Ok(track)
        })
    }

    /// Saves a track and everything saved with it. A new audio file means a
    /// new duration. Its title's language, and its tags, left out, stay as
    /// they are.
    pub fn update_track(&self, id: i64, input: &TrackInput) -> Result<Track> {
        let current = self.get_track(id)?.ok_or_else(|| not_found("track", id))?;
        let duration_ms = if input.file_id == current.file_id {
            current.duration_ms
        } else {
            self.duration_of(input.file_id)?
        };
        let prepared = self.prepare_refs(input.files.iter().map(|link| &link.file))?;
        self.store.transaction(|tx| {
            let registered = Media::insert_prepared(tx, prepared)?;
            let title_language = input
                .title_language
                .as_ref()
                .map(|language| language_tag(language.as_deref()))
                .transpose()?;
            let track = tracks::update(
                tx,
                id,
                &TrackChanges {
                    title: Some(clean_text(&input.title)),
                    title_language,
                    duration_ms: Some(duration_ms),
                    track_number: Some(input.track_number),
                    cd_number: Some(input.cd_number),
                    release_id: Some(input.release_id),
                    file_id: Some(input.file_id),
                },
            )?
            .ok_or_else(|| not_found("track", id))?;
            self.save_track_parts(tx, &track, input, &registered)?;
            Ok(track)
        })
    }

    /// Deletes a track, its artists, credit lines and file links. Its audio
    /// file stays registered. An unknown id deletes nothing.
    pub fn delete_track(&self, id: i64) -> Result<()> {
        tracks::delete(&self.store.conn(), id)
    }

    /// Saves a release's whole track list: updates the rows with an id,
    /// creates tracks for the others, registering files given by path, and
    /// deletes the tracks left out, whose files stay registered. Disc titles,
    /// when given, replace the release's, after the tracks, since a title
    /// needs a track on its disc. Left out, a disc keeps its title, unless
    /// it's left without tracks.
    pub fn set_release_tracks(
        &self,
        id: i64,
        rows: &[ReleaseTrackInput],
        disc_titles: Option<&[DiscTitleInput]>,
    ) -> Result<Release> {
        let release = self
            .get_release(id)?
            .ok_or_else(|| not_found("release", id))?;
        let titles = disc_titles.map(disc_title_rows).transpose()?;
        let fresh: Vec<&ReleaseTrackInput> = rows.iter().filter(|row| row.id.is_none()).collect();
        let prepared = self.prepare_refs(fresh.iter().map(|row| &row.file))?;
        // Durations are read now, before the transaction holds the write lock
        let mut durations: HashMap<String, Option<i64>> = HashMap::new();
        for row in &fresh {
            let registered = match row.file.file_id {
                Some(file_id) => files::get(&self.store.conn(), file_id)?,
                None => None,
            };
            let path = match registered {
                Some(file) => file.path,
                None => self
                    .media
                    .normalize_path(row.file.path.as_deref().unwrap_or("")),
            };
            let duration = self.media.read_audio_at(&path).duration_ms;
            durations.insert(path, duration);
        }

        self.store.transaction(|tx| {
            let mut credited = CreditedNames::default();
            let existing = tracks::of_releases(tx, &[id])?
                .remove(&id)
                .unwrap_or_default();
            let mut kept: Vec<i64> = Vec::new();
            for row_id in rows.iter().filter_map(|row| row.id) {
                if !kept.contains(&row_id) {
                    kept.push(row_id);
                }
            }
            for track in &existing {
                if !kept.contains(&track.id) {
                    tracks::delete(tx, track.id)?;
                }
            }
            if let Some(missing) = kept
                .iter()
                .find(|kept| !existing.iter().any(|track| track.id == **kept))
            {
                return Err(Error::Invalid(format!(
                    "Track {missing} isn't on this release"
                )));
            }
            tracks::shift_numbers(tx, id, RENUMBER_OFFSET)?;
            let registered = Media::insert_prepared(tx, prepared)?;

            for row in rows {
                let title = clean_text(&row.title);
                // Left out, it stays as it is
                let title_language = row
                    .title_language
                    .as_ref()
                    .map(|language| language_tag(language.as_deref()))
                    .transpose()?;
                let track_id = match row.id {
                    Some(track_id) => {
                        tracks::update(
                            tx,
                            track_id,
                            &TrackChanges {
                                title: Some(title.clone()),
                                title_language,
                                cd_number: Some(row.cd_number),
                                track_number: Some(row.track_number),
                                ..TrackChanges::default()
                            },
                        )?;
                        track_id
                    }
                    None => {
                        let file_id = self.ref_id(&row.file, &registered)?;
                        let path = files::get(tx, file_id)?.map(|file| file.path);
                        let duration_ms = durations
                            .get(path.as_deref().unwrap_or(""))
                            .copied()
                            .flatten();
                        tracks::insert(
                            tx,
                            &NewTrack {
                                title: title.clone(),
                                title_language: title_language.flatten(),
                                duration_ms,
                                track_number: row.track_number,
                                cd_number: row.cd_number,
                                release_id: id,
                                file_id,
                            },
                        )?
                        .id
                    }
                };
                save_alternate_titles(
                    tx,
                    TitleOwner::Track,
                    track_id,
                    &title,
                    row.alternate_titles.as_deref(),
                    &format!("track {}", row.track_number),
                )?;
                let artists = track_artists(tx, &mut credited, &row.artists, &row.features)?;
                tracks::set_artists(tx, track_id, &artists)?;
            }
            // After the tracks, since a title needs a track on its disc
            if let Some(titles) = &titles {
                releases::set_disc_titles(tx, id, titles)?;
            }
            Ok(release)
        })
    }

    /// A track's duration, read from its audio file, which has to be a
    /// registered music file.
    fn duration_of(&self, file_id: i64) -> Result<Option<i64>> {
        let file =
            files::get(&self.store.conn(), file_id)?.ok_or_else(|| not_found("file", file_id))?;
        if file.file_type != crate::media::FileType::Music {
            return Err(Error::Invalid(format!("{} isn't a music file", file.path)));
        }
        Ok(self.media.read_audio(&file).duration_ms)
    }

    /// What's saved with a track besides its own columns: its alternate
    /// titles, artists, credit lines, file links, and its tags when they're
    /// given.
    fn save_track_parts(
        &self,
        tx: &Connection,
        track: &Track,
        input: &TrackInput,
        registered: &HashMap<String, crate::media::FileRecord>,
    ) -> Result<()> {
        save_alternate_titles(
            tx,
            TitleOwner::Track,
            track.id,
            &track.title,
            input.alternate_titles.as_deref(),
            "the track",
        )?;
        let mut credited = CreditedNames::default();
        let artists = track_artists(tx, &mut credited, &input.artists, &input.features)?;
        tracks::set_artists(tx, track.id, &artists)?;
        credits::set(
            tx,
            CreditOwner::Track,
            track.id,
            &credited.lines(tx, &input.credits)?,
        )?;
        self.set_file_links(tx, LinkOwner::Track, track.id, &input.files, registered)?;
        if let Some(tags) = &input.tags {
            Tagging::set_tags_with(tx, TagOwner::Track, track.id, tags)?;
        }
        Ok(())
    }
}

/// Who performs a track (slunk's design §4.3), as credited: its own
/// artists, without features; on a regular release with none, the release's,
/// with their credited names; on a various-artists release, only its own.
pub fn performers(
    release: &Release,
    own: &[TrackArtist],
    release_artists: &[ArtistLink],
) -> Vec<ArtistLink> {
    let performing: Vec<ArtistLink> = own
        .iter()
        .filter(|artist| !artist.featured)
        .map(|artist| ArtistLink {
            artist_id: artist.artist_id,
            credited_name_id: artist.credited_name_id,
        })
        .collect();
    if !performing.is_empty() || release.various_artists {
        performing
    } else {
        release_artists.to_vec()
    }
}

/// A track's artists, then its features, each once, with their credited
/// names resolved. An artist can't be both.
fn track_artists(
    tx: &Connection,
    credited: &mut CreditedNames,
    artists: &[CreditedArtistInput],
    features: &[CreditedArtistInput],
) -> Result<Vec<TrackArtist>> {
    let featured: HashSet<i64> = features.iter().map(|link| link.artist_id).collect();
    if artists
        .iter()
        .any(|link| featured.contains(&link.artist_id))
    {
        return Err(Error::Invalid(
            "An artist can't both perform a track and be featured on it".into(),
        ));
    }
    let (artists, features) = (first_of_each(artists), first_of_each(features));
    let flags =
        std::iter::repeat_n(false, artists.len()).chain(std::iter::repeat_n(true, features.len()));
    let links: Vec<CreditedArtistInput> = artists.into_iter().chain(features).collect();
    Ok(credited
        .resolve(tx, &links)?
        .into_iter()
        .zip(flags)
        .map(|(link, featured)| TrackArtist {
            artist_id: link.artist_id,
            credited_name_id: link.credited_name_id,
            featured,
        })
        .collect())
}

/// Disc titles as they're saved: cleaned, and a blank one left out, since a
/// disc without a title has no row. Refuses two titles for one disc, and
/// alternate titles on a disc with no title, since they belong to its title.
fn disc_title_rows(titles: &[DiscTitleInput]) -> Result<Vec<DiscTitle>> {
    let mut discs = HashSet::new();
    let mut rows = Vec::new();
    for input in titles {
        let title = clean_text(&input.title);
        let cd_number = input.cd_number;
        if title.is_empty() {
            if input
                .alternate_titles
                .iter()
                .any(|alternate| !clean_text(&alternate.title).is_empty())
            {
                return Err(Error::Invalid(format!(
                    "Disc {cd_number} has alternate titles but no title"
                )));
            }
            continue;
        }
        if !discs.insert(cd_number) {
            return Err(Error::Invalid(format!("Disc {cd_number} has two titles")));
        }
        rows.push(DiscTitle {
            cd_number,
            language: language_tag(input.language.as_deref())?,
            alternate_titles: alternate_title_rows(
                &title,
                &input.alternate_titles,
                &format!("disc {cd_number}"),
            )?,
            title,
        });
    }
    Ok(rows)
}
