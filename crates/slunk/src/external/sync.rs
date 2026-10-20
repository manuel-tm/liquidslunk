//! Running a plan on a folder library (4_external_libraries.md §8.3).
//! - Removals and moves go first, to make room, then additions,
//!   conversions and retags, then covers.
//! - Converting runs several files at once, one per core but one, in the
//!   sync's temporary folder. Writing to the destination is one file at a
//!   time.
//! - Each file reaches its path whole (`Folder::stage()`, `place()`), and
//!   the journal hears of it first, so a sync that's interrupted carries on
//!   at the next one. Cancelling stops after the file in hand.

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::sync_channel;
use std::sync::{Arc, Mutex};

use super::convert::{self, Becomes};
use super::covers;
use super::folder::Folder;
use super::manifest::{self, Change, CoverKey, Entry, Manifest, Written};
use super::plan::{Action, Plan, Step, Subject, WantedTrack};
use super::scratch::Scratch;
use super::template::folded;
use super::write_tags;
use crate::{Error, Result};

/// What a running sync says about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    pub stage: Stage,
    /// The plan's steps done, and all of them
    pub done: usize,
    pub total: usize,
    /// Written to the destination so far
    pub bytes: u64,
    /// What's in hand: a track's title, or a path
    pub current: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// On an iPod, erasing the tracks liquidslunk didn't write
    Erasing,
    Removing,
    Moving,
    /// Adding, converting again and restoring
    Writing,
    Retagging,
    Covers,
    /// On an iPod, writing its database and artwork
    Database,
    Finishing,
}

/// How a sync went.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outcome {
    /// The steps done, by action
    pub done: HashMap<Action, usize>,
    pub failed: Vec<Failure>,
    pub cancelled: bool,
    pub bytes: u64,
    /// On an iPod, the tracks liquidslunk didn't write that it erased
    pub erased: usize,
}

/// A file a sync couldn't write, and why. The rest of the sync goes on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub subject: Subject,
    pub path: String,
    pub reason: String,
}

/// What a sync needs besides its plan.
pub struct Run<'a> {
    pub plan: &'a Plan,
    pub folder: &'a Folder,
    /// The external library's UUID and name, for the manifest
    pub uuid: &'a str,
    pub name: &'a str,
    /// Each release's cover, an absolute path, and the size covers fit
    pub cover_paths: HashMap<i64, PathBuf>,
    pub cover_px: u32,
    pub cancel: &'a AtomicBool,
    pub scratch: &'a Scratch,
}

/// Runs a plan. A file that can't be converted or tagged is a failure, and
/// the rest goes on. A destination that can't be written ends the sync with
/// its error, keeping what's done.
pub fn run(run: Run<'_>, progress: &dyn Fn(Progress)) -> Result<Outcome> {
    let covers = Covers::new(run.cover_px, run.cover_paths.clone(), true);
    let mut sync = Sync {
        files: run.plan.present.clone(),
        touched: Vec::new(),
        outcome: Outcome::default(),
        done: 0,
        total: run.plan.steps.len(),
        covers: &covers,
        run: &run,
        progress,
    };
    run.folder.prepare()?;
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
            run.folder.manifest().write(&manifest)?;
            run.folder.finish()?;
            run.folder
                .remove_empty_folders(sync.touched.iter().map(String::as_str))?;
            sync.outcome.cancelled = cancelled;
            Ok(sync.outcome)
        }
        Err(error) => {
            // The journal keeps what's done when the destination is gone
            let _ = run.folder.manifest().write(&manifest);
            Err(error)
        }
    }
}

struct Sync<'a> {
    /// The manifest's files, as the sync changes them
    files: Vec<Entry>,
    /// Paths files were removed or moved from, whose folders may be empty
    touched: Vec<String>,
    outcome: Outcome,
    done: usize,
    total: usize,
    covers: &'a Covers,
    run: &'a Run<'a>,
    progress: &'a dyn Fn(Progress),
}

impl Sync<'_> {
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

    fn steps(&mut self) -> Result<()> {
        let run = self.run;
        let steps = &run.plan.steps;
        let of = |actions: &[Action]| -> Vec<&Step> {
            steps
                .iter()
                .filter(|step| actions.contains(&step.action))
                .collect()
        };

        self.report(Stage::Removing, None);
        for step in of(&[Action::Remove]) {
            if self.cancelled() {
                return Ok(());
            }
            self.remove(&step.path)?;
            self.finished(Action::Remove);
            self.report(Stage::Removing, Some(step.path.clone()));
        }
        // A file written again at a new path: its old one goes first
        for step in of(&[Action::Convert, Action::Restore]) {
            if let Some(from) = &step.from {
                self.remove(from)?;
            }
        }

        self.report(Stage::Moving, None);
        let mut retags: Vec<Step> = Vec::new();
        let mut moves: Vec<Step> = of(&[Action::Move]).into_iter().cloned().collect();
        while !moves.is_empty() {
            if self.cancelled() {
                return Ok(());
            }
            // A move to a path another move has yet to leave waits for it,
            // and when every one waits, one moves aside first
            let leaving: Vec<String> = moves
                .iter()
                .filter_map(|step| step.from.as_deref().map(folded))
                .collect();
            let free = moves.iter().position(|step| {
                let to = folded(&step.path);
                let own = step.from.as_deref().map(folded);
                !leaving
                    .iter()
                    .any(|from| *from == to && Some(from) != own.as_ref())
            });
            match free {
                Some(index) => {
                    let step = moves.remove(index);
                    let from = step.from.clone().unwrap_or_default();
                    self.rename(&from, &step.path)?;
                    self.report(Stage::Moving, Some(step.path.clone()));
                    if step.changes.is_empty() {
                        self.finished(Action::Move);
                    } else {
                        retags.push(step);
                    }
                }
                None => {
                    let from = moves[0].from.clone().unwrap_or_default();
                    let aside = self.aside(&from);
                    self.rename(&from, &aside)?;
                    // It moves on from there
                    moves[0].from = Some(aside);
                }
            }
        }

        self.report(Stage::Writing, None);
        let tracks: Vec<&Step> = of(&[Action::Add, Action::Convert, Action::Restore])
            .into_iter()
            .filter(|step| matches!(step.subject, Subject::Track(_)))
            .collect();
        self.write_tracks(&tracks)?;
        if self.cancelled() {
            return Ok(());
        }

        retags.extend(of(&[Action::Retag]).into_iter().cloned());
        for step in &retags {
            if self.cancelled() {
                return Ok(());
            }
            self.report(Stage::Retagging, Some(self.label(step)));
            self.retag(step)?;
        }

        for step in steps.iter().filter(|step| {
            matches!(step.subject, Subject::Release(_)) && step.action != Action::Remove
        }) {
            if self.cancelled() {
                return Ok(());
            }
            self.report(Stage::Covers, Some(step.path.clone()));
            self.write_cover(step)?;
        }
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

    fn remove(&mut self, path: &str) -> Result<()> {
        self.run.folder.remove(path)?;
        self.change(Change::Removed { path: path.into() })?;
        self.touched.push(path.into());
        Ok(())
    }

    fn rename(&mut self, from: &str, to: &str) -> Result<()> {
        let Some(entry) = self.files.iter().find(|file| file.path == from).cloned() else {
            return Ok(());
        };
        self.change(Change::Put {
            entry: Entry {
                path: to.into(),
                ..entry
            },
            from: Some(from.into()),
        })?;
        self.run.folder.rename(from, to)?;
        self.touched.push(from.into());
        Ok(())
    }

    /// A free path beside a file, to move it aside to.
    fn aside(&self, path: &str) -> String {
        (1..)
            .map(|n| format!("{path}.moving-{n}"))
            .find(|candidate| !self.run.folder.exists(candidate))
            .expect("a free name")
    }

    /// Converts or copies tracks, several at once, and writes them one at a
    /// time as they're ready.
    fn write_tracks(&mut self, steps: &[&Step]) -> Result<()> {
        if steps.is_empty() {
            return Ok(());
        }
        let workers = std::thread::available_parallelism()
            .map_or(1, |n| n.get().saturating_sub(1))
            .clamp(1, steps.len());
        let next = AtomicUsize::new(0);
        let stop = AtomicBool::new(false);
        let plan = self.run.plan;
        let scratch = self.run.scratch;
        let cancel = self.run.cancel;
        let covers = self.covers;
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
                        let prepared = match step.subject {
                            Subject::Track(track) => match plan.wanted.get(&track) {
                                Some(wanted) => prepare(wanted, index, scratch, covers),
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
                    break;
                }
                let step = steps[index];
                match prepared {
                    Err(reason) => self.fail(step, reason),
                    Ok(prepared) => {
                        self.report(Stage::Writing, Some(self.label(step)));
                        let written = self.place_track(step, &prepared);
                        let _ = fs::remove_file(&prepared.path);
                        if let Err(error) = written {
                            result = Err(error);
                            break;
                        }
                        self.finished(step.action);
                    }
                }
                // What it last wrote, until the next is ready
                self.report(Stage::Writing, Some(self.label(step)));
            }
            stop.store(true, Ordering::Relaxed);
            result
        })
    }

    /// Writes a prepared track at its path.
    fn place_track(&mut self, step: &Step, prepared: &Prepared) -> Result<()> {
        let Subject::Track(track) = step.subject else {
            return Ok(());
        };
        let Some(wanted) = self.run.plan.wanted.get(&track) else {
            return Ok(());
        };
        let staged = self.run.folder.stage(&prepared.path)?;
        let entry = Entry {
            path: step.path.clone(),
            size: staged.size,
            mtime: staged.mtime,
            written: Written::Track {
                track,
                audio: wanted.audio.clone(),
                tags: fields(wanted),
                cover: prepared.cover.clone(),
                ipod: None,
            },
        };
        self.outcome.bytes += staged.size;
        self.change(Change::Put { entry, from: None })?;
        self.run.folder.place(staged, &step.path)
    }

    /// Writes a tagged copy of a track's file in place of the old one.
    fn retag(&mut self, step: &Step) -> Result<()> {
        let Subject::Track(track) = step.subject else {
            return Ok(());
        };
        let Some(wanted) = self.run.plan.wanted.get(&track) else {
            return Ok(());
        };
        let copy = self
            .run
            .scratch
            .path(&format!("retag-{track}.{}", extension(&step.path)));
        copy_bytes(&self.run.folder.path(&step.path), &copy)?;
        let cover = self.covers.embedded(wanted);
        let tagged = write_tags::write(
            &copy,
            &wanted.tags,
            cover.as_ref().map(|(jpeg, _)| jpeg.as_slice()),
        );
        if let Err(error) = tagged {
            let _ = fs::remove_file(&copy);
            self.fail(step, error.to_string());
            return Ok(());
        }
        let staged = self.run.folder.stage(&copy);
        let _ = fs::remove_file(&copy);
        let staged = staged?;
        let Some(old) = self
            .files
            .iter()
            .find(|file| file.path == step.path)
            .cloned()
        else {
            return Ok(());
        };
        let Written::Track { audio, .. } = old.written else {
            return Ok(());
        };
        self.outcome.bytes += staged.size;
        self.change(Change::Put {
            entry: Entry {
                path: step.path.clone(),
                size: staged.size,
                mtime: staged.mtime,
                written: Written::Track {
                    track,
                    audio,
                    tags: fields(wanted),
                    cover: cover.map(|(_, key)| key),
                    ipod: None,
                },
            },
            from: None,
        })?;
        self.run.folder.place(staged, &step.path)?;
        self.finished(step.action);
        Ok(())
    }

    /// Writes a release's `cover.jpg`.
    fn write_cover(&mut self, step: &Step) -> Result<()> {
        let Subject::Release(release) = step.subject else {
            return Ok(());
        };
        let Some(key) = self.run.plan.cover_files.get(&release).cloned() else {
            return Ok(());
        };
        let jpeg = match self.covers.jpeg(release).as_ref() {
            Ok(jpeg) => jpeg.clone(),
            Err(reason) => {
                self.fail(step, reason.clone());
                return Ok(());
            }
        };
        let file = self.run.scratch.path(&format!("cover-{release}.jpg"));
        fs::write(&file, jpeg).map_err(Error::io(&file))?;
        let staged = self.run.folder.stage(&file);
        let _ = fs::remove_file(&file);
        let staged = staged?;
        self.outcome.bytes += staged.size;
        self.change(Change::Put {
            entry: Entry {
                path: step.path.clone(),
                size: staged.size,
                mtime: staged.mtime,
                written: Written::Cover {
                    release,
                    cover: key,
                },
            },
            from: None,
        })?;
        self.run.folder.place(staged, &step.path)?;
        self.finished(step.action);
        Ok(())
    }
}

/// A track's file, made in the temporary folder, tagged and ready to copy.
pub(super) struct Prepared {
    pub path: PathBuf,
    /// The cover embedded in it
    pub cover: Option<CoverKey>,
}

/// Converts or copies a track's audio into the temporary folder, and tags
/// it. Why not, when it can't be.
pub(super) fn prepare(
    wanted: &WantedTrack,
    index: usize,
    scratch: &Scratch,
    covers: &Covers,
) -> std::result::Result<Prepared, String> {
    let source_extension = extension(&wanted.source.to_string_lossy());
    let path = scratch.path(&format!(
        "{index}.{}",
        wanted.becomes.extension(&source_extension)
    ));
    let made = match &wanted.becomes {
        Becomes::Copy => copy_bytes(&wanted.source, &path),
        Becomes::Convert(encoding) => convert::convert(&wanted.source, encoding, &path),
    };
    let cover = covers.embedded(wanted);
    let tagged = made.and_then(|()| {
        write_tags::write(
            &path,
            &wanted.tags,
            cover.as_ref().map(|(jpeg, _)| jpeg.as_slice()),
        )
    });
    match tagged {
        Ok(()) => Ok(Prepared {
            path,
            cover: cover.map(|(_, key)| key),
        }),
        Err(error) => {
            let _ = fs::remove_file(&path);
            Err(error.to_string())
        }
    }
}

/// A release's cover as a JPEG, or why it can't be written.
type Made = Arc<std::result::Result<Vec<u8>, String>>;

/// Covers as JPEGs, each release's made once, by whichever file needs it
/// first.
pub(super) struct Covers {
    px: u32,
    paths: HashMap<i64, PathBuf>,
    made: Mutex<HashMap<i64, Made>>,
    /// Whether tracks get their cover embedded: an iPod's don't
    embed: bool,
}

impl Covers {
    pub(super) fn new(px: u32, paths: HashMap<i64, PathBuf>, embed: bool) -> Covers {
        Covers {
            px,
            paths,
            made: Mutex::new(HashMap::new()),
            embed,
        }
    }

    fn jpeg(&self, release: i64) -> Made {
        if let Some(made) = self.lock().get(&release) {
            return made.clone();
        }
        let made = Arc::new(match self.paths.get(&release) {
            Some(path) => covers::jpeg(path, self.px),
            None => Err("Its release has no cover".into()),
        });
        self.lock().insert(release, made.clone());
        made
    }

    /// The cover to embed in a track, with what it was made from, or `None`
    /// when it has none, or its cover can't be read.
    fn embedded(&self, wanted: &WantedTrack) -> Option<(Vec<u8>, CoverKey)> {
        if !self.embed {
            return None;
        }
        let key = wanted.cover.clone()?;
        let jpeg = self.jpeg(wanted.release);
        jpeg.as_ref().as_ref().ok().map(|jpeg| (jpeg.clone(), key))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<i64, Made>> {
        self.made
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

pub(super) fn fields(wanted: &WantedTrack) -> BTreeMap<String, Vec<String>> {
    wanted
        .tags
        .fields()
        .into_iter()
        .map(|(field, values)| (field.to_string(), values))
        .collect()
}

/// Copies a file's bytes, and none of its attributes.
pub(super) fn copy_bytes(from: &Path, to: &Path) -> Result<()> {
    let mut source = File::open(from).map_err(Error::io(from))?;
    let mut target = File::create(to).map_err(Error::io(to))?;
    io::copy(&mut source, &mut target).map_err(Error::io(to))?;
    Ok(())
}

fn extension(path: &str) -> String {
    Path::new(path)
        .extension()
        .map(|extension| extension.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}
