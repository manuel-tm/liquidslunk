//! Running a plan on an iPod (4_external_libraries.md §8.3). The iPod reads
//! only its database, which is the last thing written, so an iPod unplugged
//! midway still plays what it played before, and the next sync carries on:
//! 1. once the user agrees, the tracks liquidslunk didn't write are erased
//!    (§7.8);
//! 2. when the new files wouldn't fit before the removed ones are gone, the
//!    database is first written without them, and they're removed;
//! 3. the new files are copied in: converted several at once, written one at
//!    a time, each journaled as it's put in place;
//! 4. the artwork and the iTunesDB are written under temporary names, then
//!    renamed over the old ones: the counts in Play Counts are added in and
//!    the file removed, and the old database is kept beside the new one;
//! 5. the files of removed tracks are deleted, and those a new file
//!    replaced;
//! 6. the manifest is written.
//!
//! A retag changes only the database. Cancelling stops after the file in
//! hand, and the database is still written for what's done.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::sync_channel;
use std::time::{SystemTime, UNIX_EPOCH};

use super::artwork::{self, FIRST_IMAGE_ID, ITHMB_MAX_BYTES, Image, Pixels, Thumb};
use super::audio::{self, Facts};
use super::itunesdb::{self, Database, DbTrack, FileKind, Gapless, Header, Stats};
use super::play_counts::{self, Counted};
use super::{ARTWORK, ARTWORK_DB, ITUNES_DB, MUSIC, PLAY_COUNTS, PREVIOUS_DB, Random};
use super::{location, mac_time, path_of};
use crate::external::folder::Folder;
use crate::external::manifest::{self, Change, CoverKey, Entry, IpodFile, Manifest, Written};
use crate::external::plan::{Action, Plan, Step, Subject};
use crate::external::scratch::Scratch;
use crate::external::sync::{Covers, Failure, Outcome, Prepared, Progress, Stage, fields, prepare};
use crate::{Error, Result};

/// Whether a release's tracks share their thumbnails' pixels, each cover
/// stored once (§7.5)
const SHARE_THUMBNAILS: bool = true;

/// A retag's tags as written, and its cover
type Retag = (BTreeMap<String, Vec<String>>, Option<CoverKey>);

/// The artwork made: each track's image, and the `.ithmb` files, by name
type Artwork = (Vec<Image>, Vec<(String, PathBuf)>);

/// What an iPod's sync needs besides its plan.
pub struct IpodRun<'a> {
    pub plan: &'a Plan,
    /// The iPod's disk
    pub folder: &'a Folder,
    /// The external library's UUID and name, for the manifest; the name is
    /// the master playlist's too
    pub uuid: &'a str,
    pub name: &'a str,
    /// Each cover in the library, by its key, for the artwork
    pub covers: HashMap<CoverKey, PathBuf>,
    pub cancel: &'a AtomicBool,
    pub scratch: &'a Scratch,
    /// The user agreed to erase the tracks liquidslunk didn't write
    pub erase: bool,
    /// The new files wouldn't fit before the removed ones are gone
    pub make_room_first: bool,
}

/// Runs a plan on an iPod. A file that can't be converted is a failure, and
/// the rest goes on. A disk that can't be written ends the sync with its
/// error, keeping what's done.
pub fn run(run: IpodRun<'_>, progress: &dyn Fn(Progress)) -> Result<Outcome> {
    let folder = run.folder;
    folder.prepare()?;
    let old = read_database(folder)?;
    let counted = read_play_counts(folder);
    let old_images: HashMap<u64, Image> = fs::read(folder.path(ARTWORK_DB))
        .ok()
        .and_then(|bytes| artwork::read(&bytes).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|image| (image.track, image))
        .collect();

    // Each track's counts, by the dbid the manifest gives its file
    let dbids: HashMap<String, u64> = run
        .plan
        .present
        .iter()
        .filter_map(|entry| Some((entry.path.to_lowercase(), ipod_of(entry)?.dbid)))
        .collect();
    let mut stats = HashMap::new();
    if let Some(old) = &old {
        for (index, track) in old.tracks.iter().enumerate() {
            let mut counts = track.stats;
            if let Some(counted) = counted.get(index) {
                counted.add_to(&mut counts);
            }
            if let Some(dbid) = dbids.get(&path_of(&track.location).to_lowercase()) {
                stats.insert(*dbid, counts);
            }
        }
    }
    let mut random = Random::new();
    let header = old.as_ref().map_or_else(
        || Header {
            id: random.next_u64(),
            platform: 1,
            id_0x24: random.next_u64(),
            language: u16::from_le_bytes(*b"en"),
            library_id: random.next_u64(),
            ..Header::default()
        },
        |old| old.header.clone(),
    );
    let master_id = old
        .as_ref()
        .map(|old| old.master_id)
        .filter(|id| *id != 0)
        .unwrap_or_else(|| random.next_u64());
    let old_covers: HashMap<u64, CoverKey> = run
        .plan
        .present
        .iter()
        .filter_map(|entry| match &entry.written {
            Written::Track {
                cover: Some(cover),
                ipod: Some(ipod),
                ..
            } => Some((ipod.dbid, cover.clone())),
            _ => None,
        })
        .collect();

    let mut sync = IpodSync {
        files: run.plan.present.clone(),
        outcome: Outcome::default(),
        done: 0,
        total: run.plan.steps.len(),
        progress,
        stats,
        header,
        master_id,
        had_database: old.is_some(),
        kept_previous: false,
        old_images,
        old_covers,
        random,
        run: &run,
    };
    let result = sync.steps();
    let cancelled = run.cancel.load(Ordering::Relaxed);
    sync.report(Stage::Finishing, None);
    let manifest = Manifest {
        version: manifest::VERSION,
        external_library: run.uuid.to_string(),
        name: run.name.to_string(),
        files: std::mem::take(&mut sync.files),
    };
    match result {
        Ok(()) => {
            folder.manifest().write(&manifest)?;
            folder.finish()?;
            sync.outcome.cancelled = cancelled;
            Ok(sync.outcome)
        }
        Err(error) => {
            // The journal keeps what's done when the iPod is gone
            let _ = folder.manifest().write(&manifest);
            Err(error)
        }
    }
}

/// The iPod's database as it is, or `None` for an iPod without one.
pub fn read_database(folder: &Folder) -> Result<Option<Database>> {
    let path = folder.path(ITUNES_DB);
    match fs::read(&path) {
        Ok(bytes) => itunesdb::read(&bytes).map(Some),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Error::io(&path)(error)),
    }
}

/// What the iPod counted since the last sync. A file that can't be read
/// counts nothing.
fn read_play_counts(folder: &Folder) -> Vec<Counted> {
    fs::read(folder.path(PLAY_COUNTS))
        .ok()
        .and_then(|bytes| play_counts::read(&bytes).ok())
        .unwrap_or_default()
}

fn ipod_of(entry: &Entry) -> Option<&IpodFile> {
    match &entry.written {
        Written::Track { ipod, .. } => ipod.as_ref(),
        Written::Cover { .. } => None,
    }
}

struct IpodSync<'a> {
    /// The manifest's files, as the sync changes them
    files: Vec<Entry>,
    outcome: Outcome,
    done: usize,
    total: usize,
    progress: &'a dyn Fn(Progress),
    /// Each track's counts, by dbid: the database's, with Play Counts'
    stats: HashMap<u64, Stats>,
    header: Header,
    master_id: u64,
    had_database: bool,
    /// Whether the database as it was is kept already
    kept_previous: bool,
    /// The artwork as it was, by track
    old_images: HashMap<u64, Image>,
    /// Each track's cover as its artwork was made from, by dbid
    old_covers: HashMap<u64, CoverKey>,
    random: Random,
    run: &'a IpodRun<'a>,
}

impl IpodSync<'_> {
    fn cancelled(&self) -> bool {
        self.run.cancel.load(Ordering::Relaxed)
    }

    fn report(&self, stage: Stage, current: Option<String>) {
        (self.progress)(Progress {
            stage,
            done: self.done,
            total: self.total,
            bytes: self.outcome.bytes,
            current,
        });
    }

    fn finished(&mut self, action: Action) {
        self.done += 1;
        *self.outcome.done.entry(action).or_default() += 1;
    }

    fn fail(&mut self, step: &Step, reason: String) {
        self.done += 1;
        self.outcome.failed.push(Failure {
            subject: step.subject,
            path: step.path.clone(),
            reason,
        });
    }

    /// Records a change in the journal and in the files.
    fn change(&mut self, change: Change) -> Result<()> {
        self.run.folder.manifest().append(&change)?;
        manifest::apply(&mut self.files, &change);
        Ok(())
    }

    fn label(&self, step: &Step) -> String {
        match step.subject {
            Subject::Track(track) => self
                .run
                .plan
                .wanted
                .get(&track)
                .map_or_else(|| step.path.clone(), |wanted| wanted.tags.title.clone()),
            Subject::Release(_) => step.path.clone(),
        }
    }

    fn steps(&mut self) -> Result<()> {
        let plan = self.run.plan;
        let of = |actions: &[Action]| -> Vec<&Step> {
            plan.steps
                .iter()
                .filter(|step| actions.contains(&step.action))
                .collect()
        };

        if self.run.erase && !plan.other.is_empty() {
            self.erase()?;
        }

        // Files that go: removed tracks', and those a new file replaces
        let removals: Vec<&Step> = of(&[Action::Remove]);
        if self.run.make_room_first {
            let leaving: HashSet<String> = removals
                .iter()
                .map(|step| step.path.clone())
                .chain(
                    of(&[Action::Convert, Action::Restore])
                        .iter()
                        .filter_map(|step| step.from.clone()),
                )
                .collect();
            if !leaving.is_empty() {
                self.report(Stage::Removing, None);
                let staying: Vec<Entry> = self
                    .files
                    .iter()
                    .filter(|file| !leaving.contains(&file.path))
                    .cloned()
                    .collect();
                self.write_database(&staying)?;
                for path in &leaving {
                    self.remove(path)?;
                }
                for _ in &removals {
                    self.finished(Action::Remove);
                }
            }
        }

        self.report(Stage::Writing, None);
        let writes = of(&[Action::Add, Action::Convert, Action::Restore]);
        let replaced = self.write_tracks(&writes)?;

        // The database, with the retags, without what goes
        self.report(Stage::Database, None);
        let retags: HashMap<String, Retag> = of(&[Action::Retag])
            .iter()
            .filter_map(|step| {
                let Subject::Track(track) = step.subject else {
                    return None;
                };
                let wanted = plan.wanted.get(&track)?;
                Some((step.path.clone(), (fields(wanted), wanted.cover.clone())))
            })
            .collect();
        let going: HashSet<String> = removals
            .iter()
            .map(|step| step.path.clone())
            .chain(replaced)
            .filter(|path| self.files.iter().any(|file| &file.path == path))
            .collect();
        let retagged: Vec<Entry> = self
            .files
            .iter()
            .filter_map(|file| {
                let (tags, cover) = retags.get(&file.path)?;
                let mut file = file.clone();
                if let Written::Track {
                    tags: old_tags,
                    cover: old_cover,
                    ..
                } = &mut file.written
                {
                    *old_tags = tags.clone();
                    *old_cover = cover.clone();
                }
                Some(file)
            })
            .collect();
        let staying: Vec<Entry> = self
            .files
            .iter()
            .filter(|file| !going.contains(&file.path))
            .map(|file| {
                retagged
                    .iter()
                    .find(|retagged| retagged.path == file.path)
                    .unwrap_or(file)
                    .clone()
            })
            .collect();
        self.write_database(&staying)?;
        for entry in retagged {
            self.change(Change::Put { entry, from: None })?;
            self.finished(Action::Retag);
        }

        self.report(Stage::Removing, None);
        let removed: HashSet<&str> = removals.iter().map(|step| step.path.as_str()).collect();
        for path in going {
            self.remove(&path)?;
            if removed.contains(path.as_str()) {
                self.finished(Action::Remove);
            }
            self.report(Stage::Removing, Some(path));
        }
        Ok(())
    }

    /// Erases the tracks liquidslunk didn't write: only their files in
    /// `iPod_Control/Music`, and never one the manifest lists.
    fn erase(&mut self) -> Result<()> {
        self.report(Stage::Erasing, None);
        let ours: HashSet<String> = self
            .files
            .iter()
            .map(|file| file.path.to_lowercase())
            .collect();
        let music = format!("{}/", MUSIC.to_lowercase());
        for other in &self.run.plan.other {
            let path = other.path.to_lowercase();
            if !path.starts_with(&music) || ours.contains(&path) || path.contains("..") {
                continue;
            }
            self.run.folder.remove(&other.path)?;
            self.outcome.erased += 1;
            self.report(Stage::Erasing, Some(other.label.title.clone()));
        }
        Ok(())
    }

    /// Removes a file liquidslunk wrote, and records it. Its music folder
    /// stays, as the iPod has it.
    fn remove(&mut self, path: &str) -> Result<()> {
        self.run.folder.remove(path)?;
        self.change(Change::Removed { path: path.into() })
    }

    /// Converts or copies tracks, several at once, and writes them one at a
    /// time as they're ready. The paths of the files they replace, for those
    /// written.
    fn write_tracks(&mut self, steps: &[&Step]) -> Result<Vec<String>> {
        let mut replaced = Vec::new();
        if steps.is_empty() {
            return Ok(replaced);
        }
        let workers = std::thread::available_parallelism()
            .map_or(1, |n| n.get().saturating_sub(1))
            .clamp(1, steps.len());
        let next = AtomicUsize::new(0);
        let stop = AtomicBool::new(false);
        let plan = self.run.plan;
        let scratch = self.run.scratch;
        let cancel = self.run.cancel;
        let covers = Covers::new(0, HashMap::new(), false);
        let covers = &covers;
        std::thread::scope(|scope| {
            let (sender, receiver) = sync_channel(2);
            for _ in 0..workers {
                let sender = sender.clone();
                let (next, stop) = (&next, &stop);
                scope.spawn(move || {
                    loop {
                        if cancel.load(Ordering::Relaxed) || stop.load(Ordering::Relaxed) {
                            break;
                        }
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(step) = steps.get(index) else {
                            break;
                        };
                        let prepared =
                            match step.subject {
                                Subject::Track(track) => match plan.wanted.get(&track) {
                                    Some(wanted) => prepare(wanted, index, scratch, covers)
                                        .and_then(|prepared| match audio::read(&prepared.path) {
                                            Ok(facts) => Ok((prepared, facts)),
                                            Err(reason) => {
                                                let _ = fs::remove_file(&prepared.path);
                                                Err(reason)
                                            }
                                        }),
                                    None => Err("It's no longer in the plan".into()),
                                },
                                Subject::Release(_) => Err("It isn't a track".into()),
                            };
                        if sender.send((index, prepared)).is_err() {
                            break;
                        }
                    }
                });
            }
            drop(sender);
            let mut result = Ok(());
            for (index, prepared) in receiver {
                if self.cancelled() {
                    if let Ok((prepared, _)) = prepared {
                        let _ = fs::remove_file(&prepared.path);
                    }
                    continue;
                }
                let step = steps[index];
                match prepared {
                    Err(reason) => self.fail(step, reason),
                    Ok((prepared, facts)) => {
                        self.report(Stage::Writing, Some(self.label(step)));
                        let written = self.place_track(step, &prepared, &facts);
                        let _ = fs::remove_file(&prepared.path);
                        match written {
                            Ok(true) => {
                                replaced.extend(step.from.clone());
                                self.finished(step.action);
                            }
                            Ok(false) => {}
                            Err(error) => {
                                result = Err(error);
                                break;
                            }
                        }
                    }
                }
                self.report(Stage::Writing, Some(self.label(step)));
            }
            stop.store(true, Ordering::Relaxed);
            result
        })?;
        Ok(replaced)
    }

    /// Writes a prepared track at its path, journaled first. False when
    /// something is at its path already, which is a failure.
    fn place_track(&mut self, step: &Step, prepared: &Prepared, facts: &Facts) -> Result<bool> {
        let Subject::Track(track) = step.subject else {
            return Ok(false);
        };
        let Some(wanted) = self.run.plan.wanted.get(&track) else {
            return Ok(false);
        };
        if self.run.folder.exists(&step.path) {
            self.fail(step, format!("Something is at {} already", step.path));
            return Ok(false);
        }
        // A track written again keeps its id, and when it was added
        let before = step
            .from
            .as_ref()
            .and_then(|from| self.files.iter().find(|file| &file.path == from))
            .and_then(ipod_of)
            .map(|ipod| (ipod.dbid, ipod.added));
        let (dbid, added) = match before {
            Some(before) => before,
            None => (self.new_dbid(), now()),
        };
        let staged = self.run.folder.stage(&prepared.path)?;
        let kind = match facts.kind {
            Some(FileKind::Alac) => "alac",
            Some(FileKind::Mp3) => "mp3",
            _ => "aac",
        };
        let entry = Entry {
            path: step.path.clone(),
            size: staged.size,
            mtime: staged.mtime,
            written: Written::Track {
                track,
                audio: wanted.audio.clone(),
                tags: fields(wanted),
                cover: wanted.cover.clone(),
                ipod: Some(IpodFile {
                    dbid,
                    kind: kind.into(),
                    length_ms: facts.length_ms,
                    bitrate: facts.bitrate,
                    sample_rate: facts.sample_rate,
                    sound_check: facts.sound_check,
                    pregap: facts.gapless.pregap,
                    postgap: facts.gapless.postgap,
                    sample_count: facts.gapless.sample_count,
                    gapless_data: facts.gapless.data,
                    added,
                }),
            },
        };
        self.outcome.bytes += staged.size;
        self.change(Change::Put { entry, from: None })?;
        self.run.folder.place(staged, &step.path)?;
        Ok(true)
    }

    /// A dbid no file has.
    fn new_dbid(&mut self) -> u64 {
        loop {
            let dbid = self.random.next_u64();
            if dbid != 0
                && !self
                    .files
                    .iter()
                    .any(|file| ipod_of(file).is_some_and(|ipod| ipod.dbid == dbid))
            {
                return dbid;
            }
        }
    }

    /// Writes the database and the artwork for these files, each under a
    /// temporary name and then renamed over the old one. The database as it
    /// was is kept, the first time, and Play Counts goes, since its counts
    /// are in the new one.
    fn write_database(&mut self, files: &[Entry]) -> Result<()> {
        let folder = self.run.folder;
        let mut entries: Vec<(&Entry, &IpodFile)> = files
            .iter()
            .filter_map(|entry| Some((entry, ipod_of(entry)?)))
            .collect();
        entries.sort_by(|(a, _), (b, _)| compare_entries(a, b));

        let (images, ithmbs) = self.write_artwork(&entries)?;
        let artwork_of: HashMap<u64, (u32, u32)> = images
            .iter()
            .map(|image| (image.track, (image.id, image.source_size)))
            .collect();
        let tracks: Vec<DbTrack> = entries
            .iter()
            .map(|(entry, ipod)| {
                db_track(
                    entry,
                    ipod,
                    self.stats.get(&ipod.dbid).copied().unwrap_or_default(),
                    artwork_of.get(&ipod.dbid).copied(),
                )
            })
            .collect();
        let database = itunesdb::write(
            &Database {
                header: self.header.clone(),
                master_id: self.master_id,
                tracks,
            },
            self.run.name,
        );
        let artwork_db = artwork::write(&images);

        let mut staged = Vec::new();
        for (name, path) in &ithmbs {
            staged.push((folder.stage(path)?, format!("{ARTWORK}/{name}")));
            let _ = fs::remove_file(path);
        }
        staged.push((
            folder.stage_with(|file| file.write_all(&artwork_db))?,
            ARTWORK_DB.to_string(),
        ));
        let new_database = folder.stage_with(|file| file.write_all(&database))?;
        if self.had_database && !self.kept_previous {
            let previous = folder.stage(&folder.path(ITUNES_DB))?;
            folder.place(previous, PREVIOUS_DB)?;
            self.kept_previous = true;
        }
        folder.remove(PLAY_COUNTS)?;
        for (staged, path) in staged {
            folder.place(staged, &path)?;
        }
        folder.place(new_database, ITUNES_DB)?;
        self.had_database = true;

        // Thumbnail files the new artwork doesn't use
        let made: HashSet<String> = ithmbs.iter().map(|(name, _)| name.clone()).collect();
        if let Ok(entries) = fs::read_dir(folder.path(ARTWORK)) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let ours = artwork::FORMATS
                    .iter()
                    .any(|(format, _)| name.starts_with(&format!("F{format}_")))
                    && name.ends_with(".ithmb");
                if ours && !made.contains(&name) {
                    folder.remove(&format!("{ARTWORK}/{name}"))?;
                }
            }
        }
        Ok(())
    }

    /// Writes the thumbnails of these tracks' covers into new `.ithmb` files
    /// in the sync's temporary folder: the pixels already on the iPod for a
    /// cover that's the same, and new ones made from the library's. Each
    /// track's image, and the files, by name.
    fn write_artwork(&self, entries: &[(&Entry, &IpodFile)]) -> Result<Artwork> {
        let cover_of = |entry: &Entry| match &entry.written {
            Written::Track { cover, .. } => cover.clone(),
            Written::Cover { .. } => None,
        };
        // What's to be made from the library's covers, several at once
        let reusable = |entry: &Entry, ipod: &IpodFile| {
            let cover = cover_of(entry);
            cover.is_some()
                && self.old_covers.get(&ipod.dbid) == cover.as_ref()
                && self.old_images.contains_key(&ipod.dbid)
        };
        let to_make: Vec<CoverKey> = entries
            .iter()
            .filter(|(entry, ipod)| !reusable(entry, ipod))
            .filter_map(|(entry, _)| cover_of(entry))
            .filter(|cover| self.run.covers.contains_key(cover))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let made = make_thumbnails(&to_make, &self.run.covers);

        let mut ithmbs = Ithmbs::new(self.run.scratch);
        let mut old_files: HashMap<String, File> = HashMap::new();
        let mut placed: HashMap<CoverKey, Vec<Thumb>> = HashMap::new();
        let mut images = Vec::new();
        for (entry, ipod) in entries {
            let Some(cover) = cover_of(entry) else {
                continue;
            };
            let thumbs = match placed.get(&cover).filter(|_| SHARE_THUMBNAILS) {
                Some(thumbs) => Some(thumbs.clone()),
                None => {
                    let pixels = if reusable(entry, ipod) {
                        self.old_images.get(&ipod.dbid).and_then(|image| {
                            read_old_pixels(self.run.folder, image, &mut old_files)
                        })
                    } else {
                        made.get(&cover).cloned()
                    };
                    match pixels {
                        Some(pixels) => {
                            let thumbs = ithmbs.append(&pixels)?;
                            placed.insert(cover.clone(), thumbs.clone());
                            Some(thumbs)
                        }
                        None => None,
                    }
                }
            };
            if let Some(thumbs) = thumbs {
                images.push(Image {
                    id: FIRST_IMAGE_ID + images.len() as u32,
                    track: ipod.dbid,
                    source_size: thumbs.iter().map(|thumb| thumb.size).sum(),
                    thumbs,
                });
            }
        }
        Ok((images, ithmbs.finish()?))
    }
}

/// The thumbnails of covers, made several at once. A cover that can't be
/// read has none.
fn make_thumbnails(
    covers: &[CoverKey],
    paths: &HashMap<CoverKey, PathBuf>,
) -> HashMap<CoverKey, Vec<Pixels>> {
    if covers.is_empty() {
        return HashMap::new();
    }
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = covers.len().div_ceil(workers).max(1);
    std::thread::scope(|scope| {
        let handles: Vec<_> = covers
            .chunks(chunk)
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .filter_map(|cover| {
                            let image = image::open(paths.get(cover)?).ok()?;
                            Some((cover.clone(), artwork::thumbnails(&image)))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap_or_default())
            .collect()
    })
}

/// A track's thumbnails as they're on the iPod now, read from its `.ithmb`
/// files, or `None` when they can't be.
fn read_old_pixels(
    folder: &Folder,
    image: &Image,
    files: &mut HashMap<String, File>,
) -> Option<Vec<Pixels>> {
    let mut pixels = Vec::new();
    for (format, side) in artwork::FORMATS {
        let thumb = image.thumbs.iter().find(|thumb| thumb.format == format)?;
        if thumb.size != side * side * 2 {
            return None;
        }
        let name = thumb.file.trim_start_matches(':').replace(':', "/");
        let file = match files.entry(name.clone()) {
            std::collections::hash_map::Entry::Occupied(file) => file.into_mut(),
            std::collections::hash_map::Entry::Vacant(slot) => {
                slot.insert(File::open(folder.path(&format!("{ARTWORK}/{name}"))).ok()?)
            }
        };
        let mut bytes = vec![0u8; thumb.size as usize];
        file.seek(SeekFrom::Start(u64::from(thumb.offset))).ok()?;
        file.read_exact(&mut bytes).ok()?;
        pixels.push(Pixels {
            format,
            bytes,
            width: thumb.width,
            height: thumb.height,
            horizontal_padding: thumb.horizontal_padding,
            vertical_padding: thumb.vertical_padding,
        });
    }
    Some(pixels)
}

/// New `.ithmb` files, a format's each, written in the sync's temporary
/// folder and moved onto the iPod once they're whole.
struct Ithmbs<'a> {
    scratch: &'a Scratch,
    /// By format: its file's number, the file, and how much is in it
    open: HashMap<u32, (u32, BufWriter<File>, u64)>,
    /// The files made, by name, and where they are
    made: Vec<(String, PathBuf)>,
}

impl<'a> Ithmbs<'a> {
    fn new(scratch: &'a Scratch) -> Ithmbs<'a> {
        Ithmbs {
            scratch,
            open: HashMap::new(),
            made: Vec::new(),
        }
    }

    /// Appends a cover's thumbnails, and says where they are.
    fn append(&mut self, pixels: &[Pixels]) -> Result<Vec<Thumb>> {
        let mut thumbs = Vec::new();
        for thumb in pixels {
            let size = thumb.bytes.len() as u64;
            let full = self
                .open
                .get(&thumb.format)
                .is_some_and(|(_, _, offset)| offset + size > ITHMB_MAX_BYTES);
            if full || !self.open.contains_key(&thumb.format) {
                let number = self
                    .open
                    .get(&thumb.format)
                    .map_or(1, |(number, _, _)| number + 1);
                if let Some((_, mut writer, _)) = self.open.remove(&thumb.format) {
                    writer.flush().map_err(Error::io(Path::new("ithmb")))?;
                }
                let name = format!("F{}_{number}.ithmb", thumb.format);
                let path = self.scratch.path(&name);
                let file = File::create(&path).map_err(Error::io(&path))?;
                self.made.push((name, path));
                self.open
                    .insert(thumb.format, (number, BufWriter::new(file), 0));
            }
            let (number, writer, offset) = self.open.get_mut(&thumb.format).expect("opened above");
            writer
                .write_all(&thumb.bytes)
                .map_err(Error::io(Path::new("ithmb")))?;
            thumbs.push(Thumb {
                format: thumb.format,
                file: format!(":F{}_{number}.ithmb", thumb.format),
                offset: *offset as u32,
                size: size as u32,
                width: thumb.width,
                height: thumb.height,
                horizontal_padding: thumb.horizontal_padding,
                vertical_padding: thumb.vertical_padding,
            });
            *offset += size;
        }
        Ok(thumbs)
    }

    fn finish(self) -> Result<Vec<(String, PathBuf)>> {
        for (_, (_, mut writer, _)) in self.open {
            writer.flush().map_err(Error::io(Path::new("ithmb")))?;
        }
        Ok(self.made)
    }
}

/// A file's record in the database, from its tags as written and what the
/// manifest keeps of it.
fn db_track(entry: &Entry, ipod: &IpodFile, stats: Stats, artwork: Option<(u32, u32)>) -> DbTrack {
    let tags = match &entry.written {
        Written::Track { tags, .. } => tags.clone(),
        Written::Cover { .. } => BTreeMap::new(),
    };
    let one = |field: &str| tags.get(field).and_then(|values| values.first()).cloned();
    // "3 of 12"
    let numbers = |field: &str| -> (u32, u32) {
        let text = one(field).unwrap_or_default();
        let mut parts = text
            .split(" of ")
            .map(|part| part.trim().parse().unwrap_or(0));
        (parts.next().unwrap_or(0), parts.next().unwrap_or(0))
    };
    let (track_number, track_total) = numbers("Track");
    let (disc_number, disc_total) = numbers("Disc");
    DbTrack {
        id: 0,
        dbid: ipod.dbid,
        location: location(&entry.path),
        title: one("Title").unwrap_or_default(),
        artist: one("Artist"),
        album: one("Album"),
        album_artist: one("Album artist"),
        sort_artist: one("Artist sort"),
        sort_album_artist: one("Album artist sort"),
        sort_album: one("Album sort"),
        sort_title: one("Title sort"),
        kind: Some(match ipod.kind.as_str() {
            "alac" => FileKind::Alac,
            "mp3" => FileKind::Mp3,
            _ => FileKind::Aac,
        }),
        compilation: tags.contains_key("Compilation"),
        size: u32::try_from(entry.size).unwrap_or(u32::MAX),
        length_ms: ipod.length_ms,
        track_number,
        track_total,
        disc_number,
        disc_total,
        year: one("Date")
            .and_then(|date| date.get(..4)?.parse().ok())
            .unwrap_or(0),
        bitrate: ipod.bitrate,
        sample_rate: ipod.sample_rate,
        sound_check: ipod.sound_check,
        added: mac_time(ipod.added),
        modified: mac_time(entry.mtime / 1_000_000_000),
        stats,
        gapless: Gapless {
            pregap: ipod.pregap,
            postgap: ipod.postgap,
            sample_count: ipod.sample_count,
            data: ipod.gapless_data,
        },
        artwork,
    }
}

/// The database's order: by album artist, album, disc and track, each by
/// its sort form when it has one.
fn compare_entries(a: &Entry, b: &Entry) -> std::cmp::Ordering {
    use crate::media::library::compare;
    let key = |entry: &Entry| {
        let Written::Track { tags, .. } = &entry.written else {
            return (String::new(), String::new(), String::new(), String::new());
        };
        let one = |fields: &[&str]| {
            fields
                .iter()
                .find_map(|field| tags.get(*field).and_then(|values| values.first()))
                .cloned()
                .unwrap_or_default()
        };
        (
            one(&["Album artist sort", "Album artist", "Artist sort", "Artist"]),
            one(&["Album sort", "Album"]),
            one(&["Disc"]),
            one(&["Track"]),
        )
    };
    let (a, b) = (key(a), key(b));
    compare(&a.0, &b.0)
        .then_with(|| compare(&a.1, &b.1))
        .then_with(|| compare(&a.2, &b.2))
        .then_with(|| compare(&a.3, &b.3))
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs() as i64)
}
