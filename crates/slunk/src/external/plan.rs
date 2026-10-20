//! The plan (backend.md, "Syncing a folder library"): what a sync would do,
//! worked out from what should be on a destination and what is, before
//! anything is written. It's plain functions, one for a folder library and
//! one for an iPod, so their rules are tested without files.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

use super::Tags;
use super::convert::Becomes;
use super::manifest::{AudioKey, CoverKey, Entry, Written};
use super::template::{TrackPath, folded};

/// The name of the cover a folder that holds one release's tracks gets
pub const COVER_FILE: &str = "cover.jpg";

/// A track that should be on the destination.
#[derive(Debug, Clone)]
pub struct WantedTrack {
    pub track: i64,
    pub release: i64,
    /// Where its template puts it, before it's told apart from others
    pub path: TrackPath,
    /// Its audio file in the library, an absolute path
    pub source: PathBuf,
    pub becomes: Becomes,
    pub audio: AudioKey,
    pub tags: Tags,
    /// The release's cover, when it's embedded
    pub cover: Option<CoverKey>,
    /// What it will take, estimated
    pub bytes: i64,
}

/// A file the manifest lists, and whether it's on the disk as liquidslunk
/// wrote it.
#[derive(Debug, Clone)]
pub struct PresentFile {
    pub entry: Entry,
    pub as_written: bool,
    /// On an iPod, whether its database lists it. Always, in a folder.
    pub listed: bool,
}

/// A track on an iPod that liquidslunk didn't write, which its first sync
/// erases once the user agrees (backend.md, "Syncing an iPod").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtherTrack {
    /// Relative to the iPod
    pub path: String,
    pub label: Label,
}

/// What a sync leaves out, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeftOut {
    pub subject: Subject,
    pub reason: String,
}

/// How a plan names a track or a release: by its title, its artist and its
/// album, or a release by its title and its artist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
}

/// What a plan is worked out from.
#[derive(Debug, Clone, Default)]
pub struct Inputs {
    /// In the order the plan lists them
    pub wanted: Vec<WantedTrack>,
    /// Tracks whose files stay as they are, since their source file is
    /// missing from the library, with their release
    pub kept: HashMap<i64, i64>,
    pub left_out: Vec<LeftOut>,
    /// Each release's cover, for its `cover.jpg`, when the profile asks
    /// for one
    pub cover_files: HashMap<i64, CoverKey>,
    pub present: Vec<PresentFile>,
    /// Labels for what's left out, besides the wanted tracks' own
    pub labels: HashMap<Subject, Label>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// A file that should be there and isn't
    Add,
    /// A file whose audio key changed: a new audio file for its track, or a
    /// new format. A `cover.jpg` whose cover changed.
    Convert,
    /// A file missing from the destination, or not as liquidslunk wrote it
    Restore,
    /// A file whose tags or cover changed
    Retag,
    /// A file whose path changed, retagged too when its tags changed
    Move,
    /// A file that's there but has left the selection, or been deleted
    Remove,
}

/// What a step or a reason is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Subject {
    Track(i64),
    /// A release's `cover.jpg`, or its cover
    Release(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub action: Action,
    pub subject: Subject,
    /// Where the file goes, or a removal's, where it is
    pub path: String,
    /// Where it is now, when that's elsewhere: a move's, or a file written
    /// again at a new path, whose old file is removed
    pub from: Option<String>,
    /// What a retag changes, or a move that retags too
    pub changes: Vec<FieldChange>,
    /// What it writes, estimated, or a removal's, what it frees
    pub bytes: i64,
}

/// A field a retag changes: "Title: Shoka → 初夏".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldChange {
    pub field: String,
    pub was: Vec<String>,
    pub now: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Plan {
    /// In the order they're listed: tracks in the selection's order, then
    /// covers, then removals
    pub steps: Vec<Step>,
    pub left_out: Vec<LeftOut>,
    /// Files that stay as they are
    pub unchanged: usize,
    /// The manifest's files, which the sync starts from
    pub present: Vec<Entry>,
    pub wanted: HashMap<i64, WantedTrack>,
    pub cover_files: HashMap<i64, CoverKey>,
    /// How it names each track and release in it: wanted tracks by their
    /// tags, and the files it removes by their tags as written
    pub labels: HashMap<Subject, Label>,
    /// What it writes, estimated
    pub added: i64,
    /// What the files it removes or replaces took
    pub freed: i64,
    /// On an iPod, tracks liquidslunk didn't write, which it erases first
    pub other: Vec<OtherTrack>,
}

impl Plan {
    /// The space it needs on the destination, in bytes: what it writes,
    /// less what it frees.
    pub fn bytes_needed(&self) -> i64 {
        self.added - self.freed
    }
}

/// Works out a plan. `occupied` says whether anything is at a path on the
/// destination: a file there that the manifest doesn't list is someone
/// else's, and is never replaced.
pub fn plan(inputs: Inputs, occupied: &dyn Fn(&str) -> bool) -> Plan {
    let Inputs {
        wanted,
        kept,
        mut left_out,
        cover_files,
        present,
        mut labels,
    } = inputs;
    label_all(&wanted, &present, &mut labels);
    let ours: HashSet<String> = present
        .iter()
        .map(|file| folded(&file.entry.path))
        .collect();
    let foreign = |path: &str| !ours.contains(&folded(path)) && occupied(path);

    let mut tracks_present: HashMap<i64, Vec<&PresentFile>> = HashMap::new();
    let mut covers_present: HashMap<String, &PresentFile> = HashMap::new();
    for file in &present {
        match &file.entry.written {
            Written::Track { track, .. } => tracks_present.entry(*track).or_default().push(file),
            Written::Cover { .. } => {
                covers_present.insert(folded(&file.entry.path), file);
            }
        }
    }

    // Kept tracks' files stay where they are
    let mut claimed: HashSet<&str> = HashSet::new();
    let mut taken: HashSet<String> = HashSet::new();
    for track in kept.keys() {
        for file in tracks_present.get(track).into_iter().flatten() {
            claimed.insert(&file.entry.path);
            taken.insert(folded(&file.entry.path));
        }
    }
    let current: HashMap<i64, &str> = tracks_present
        .iter()
        .filter(|(track, _)| !kept.contains_key(track))
        .filter_map(|(track, files)| Some((*track, files.first()?.entry.path.as_str())))
        .collect();
    let assigned = assign(&wanted, &current, taken);

    let mut plan = Plan {
        cover_files,
        ..Plan::default()
    };
    let mut placed: Vec<(i64, String)> = Vec::new();
    for wanted in &wanted {
        let path = &assigned[&wanted.track];
        let files = tracks_present
            .get(&wanted.track)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let file = files
            .iter()
            .find(|file| &file.entry.path == path)
            .or_else(|| files.first());
        let step = |action, from: Option<&str>, changes, bytes| Step {
            action,
            subject: Subject::Track(wanted.track),
            path: path.clone(),
            from: from.map(str::to_string),
            changes,
            bytes,
        };
        let taken_by_someone = || LeftOut {
            subject: Subject::Track(wanted.track),
            reason: format!("A file liquidslunk didn't write is at {path}"),
        };
        let Some(file) = file else {
            if foreign(path) {
                left_out.push(taken_by_someone());
            } else {
                plan.added += wanted.bytes;
                plan.steps
                    .push(step(Action::Add, None, Vec::new(), wanted.bytes));
                placed.push((wanted.release, path.clone()));
            }
            continue;
        };
        claimed.insert(&file.entry.path);
        let Written::Track {
            audio, tags, cover, ..
        } = &file.entry.written
        else {
            continue;
        };
        let moving = &file.entry.path != path;
        if moving && foreign(path) {
            // Its file stays where it is
            left_out.push(taken_by_someone());
            continue;
        }
        placed.push((wanted.release, path.clone()));
        let from = moving.then_some(file.entry.path.as_str());
        let freed = if file.as_written {
            file.entry.size as i64
        } else {
            0
        };
        if !audio.matches(&wanted.audio) {
            plan.added += wanted.bytes;
            plan.freed += freed;
            plan.steps
                .push(step(Action::Convert, from, Vec::new(), wanted.bytes));
        } else if !file.as_written {
            plan.added += wanted.bytes;
            plan.steps
                .push(step(Action::Restore, from, Vec::new(), wanted.bytes));
        } else {
            let changes = changes(tags, &wanted.tags, cover.as_ref(), wanted.cover.as_ref());
            if moving {
                plan.steps.push(step(Action::Move, from, changes, 0));
            } else if !changes.is_empty() {
                plan.steps.push(step(Action::Retag, None, changes, 0));
            } else {
                plan.unchanged += 1;
            }
        }
    }
    for (track, release) in &kept {
        for file in tracks_present.get(track).into_iter().flatten() {
            placed.push((*release, file.entry.path.clone()));
        }
    }

    // A cover.jpg in each folder that holds one release's tracks
    let mut folders: BTreeMap<String, BTreeSet<i64>> = BTreeMap::new();
    for (release, path) in &placed {
        let folder = path.rsplit_once('/').map_or("", |(folder, _)| folder);
        folders
            .entry(folder.to_string())
            .or_default()
            .insert(*release);
    }
    for (folder, releases) in &folders {
        let (Some(release), 1) = (releases.first(), releases.len()) else {
            continue;
        };
        let Some(key) = plan.cover_files.get(release) else {
            continue;
        };
        let path = if folder.is_empty() {
            COVER_FILE.to_string()
        } else {
            format!("{folder}/{COVER_FILE}")
        };
        let step = |action| Step {
            action,
            subject: Subject::Release(*release),
            path: path.clone(),
            from: None,
            changes: Vec::new(),
            bytes: 0,
        };
        match covers_present.get(&folded(&path)) {
            Some(file) => {
                claimed.insert(&file.entry.path);
                let same =
                    matches!(&file.entry.written, Written::Cover { cover, .. } if cover == key);
                if !same {
                    plan.steps.push(step(Action::Convert));
                } else if !file.as_written {
                    plan.steps.push(step(Action::Restore));
                } else {
                    plan.unchanged += 1;
                }
            }
            None if foreign(&path) => left_out.push(LeftOut {
                subject: Subject::Release(*release),
                reason: format!("A file liquidslunk didn't write is at {path}"),
            }),
            None => plan.steps.push(step(Action::Add)),
        }
    }

    // What's there and no longer wanted
    for file in &present {
        if claimed.contains(file.entry.path.as_str()) {
            continue;
        }
        let subject = match file.entry.written {
            Written::Track { track, .. } => Subject::Track(track),
            Written::Cover { release, .. } => Subject::Release(release),
        };
        let bytes = if file.as_written {
            file.entry.size as i64
        } else {
            0
        };
        plan.freed += bytes;
        plan.steps.push(Step {
            action: Action::Remove,
            subject,
            path: file.entry.path.clone(),
            from: None,
            changes: Vec::new(),
            bytes,
        });
    }

    plan.left_out = left_out;
    plan.labels = labels;
    plan.present = present.into_iter().map(|file| file.entry).collect();
    plan.wanted = wanted
        .into_iter()
        .map(|wanted| (wanted.track, wanted))
        .collect();
    plan
}

/// Names every track and release a plan mentions: the wanted tracks by
/// their tags, and the files that are there by their tags as written.
fn label_all(
    wanted: &[WantedTrack],
    present: &[PresentFile],
    labels: &mut HashMap<Subject, Label>,
) {
    for wanted in wanted {
        let tags = &wanted.tags;
        labels.insert(
            Subject::Track(wanted.track),
            Label {
                title: tags.title.clone(),
                artist: tags.artist.clone(),
                album: Some(tags.album.clone()),
            },
        );
        labels
            .entry(Subject::Release(wanted.release))
            .or_insert_with(|| Label {
                title: tags.album.clone(),
                artist: tags.album_artist.clone(),
                album: None,
            });
    }
    for file in present {
        // A cover.jpg of a release that's left: by its folder
        if let Written::Cover { release, .. } = &file.entry.written {
            let folder = file
                .entry
                .path
                .rsplit_once('/')
                .map_or("", |(folder, _)| folder);
            labels
                .entry(Subject::Release(*release))
                .or_insert_with(|| Label {
                    title: folder.to_string(),
                    artist: None,
                    album: None,
                });
        }
        if let Written::Track { track, tags, .. } = &file.entry.written {
            labels
                .entry(Subject::Track(*track))
                .or_insert_with(|| label_of(tags, &file.entry.path));
        }
    }
}

/// How a file is named by its tags as written, or by its path.
pub fn label_of(tags: &BTreeMap<String, Vec<String>>, path: &str) -> Label {
    let field = |name: &str| tags.get(name).map(|values| values.join(", "));
    Label {
        title: field("Title").unwrap_or_else(|| path.to_string()),
        artist: field("Artist"),
        album: field("Album"),
    }
}

/// Works out an iPod's plan (backend.md, "Syncing an iPod"). The iPod names
/// its own files, so a track's file keeps its path, and a file written again
/// gets a new one from `new_path`, given its extension; the old one is
/// removed once the database no longer lists it. A retag changes only the
/// database, and so does a file the database doesn't list yet. There are no
/// moves, no `cover.jpg`s, and nothing else at a path a track could want.
pub fn plan_ipod(inputs: Inputs, new_path: &mut dyn FnMut(&str) -> String) -> Plan {
    let Inputs {
        wanted,
        kept,
        left_out,
        cover_files: _,
        present,
        mut labels,
    } = inputs;
    label_all(&wanted, &present, &mut labels);
    let mut tracks_present: HashMap<i64, Vec<&PresentFile>> = HashMap::new();
    for file in &present {
        if let Written::Track { track, .. } = &file.entry.written {
            tracks_present.entry(*track).or_default().push(file);
        }
    }
    let mut claimed: HashSet<&str> = HashSet::new();
    for track in kept.keys() {
        for file in tracks_present.get(track).into_iter().flatten() {
            claimed.insert(&file.entry.path);
        }
    }

    let mut plan = Plan::default();
    for wanted in &wanted {
        let step = |action, path: String, from: Option<&str>, changes, bytes| Step {
            action,
            subject: Subject::Track(wanted.track),
            path,
            from: from.map(str::to_string),
            changes,
            bytes,
        };
        let extension = || {
            let source = wanted
                .source
                .extension()
                .map(|extension| extension.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            wanted.becomes.extension(&source)
        };
        // A sync cut short can leave two files for a track: the best one
        // stays, and the others go
        let best = tracks_present.get(&wanted.track).and_then(|files| {
            files
                .iter()
                .copied()
                .max_by_key(|file| match &file.entry.written {
                    Written::Track { audio, .. } => {
                        (audio.matches(&wanted.audio), file.as_written, file.listed)
                    }
                    Written::Cover { .. } => (false, false, false),
                })
        });
        let Some(file) = best else {
            plan.added += wanted.bytes;
            plan.steps.push(step(
                Action::Add,
                new_path(&extension()),
                None,
                Vec::new(),
                wanted.bytes,
            ));
            continue;
        };
        claimed.insert(&file.entry.path);
        let Written::Track {
            audio, tags, cover, ..
        } = &file.entry.written
        else {
            continue;
        };
        let freed = if file.as_written {
            file.entry.size as i64
        } else {
            0
        };
        let from = Some(file.entry.path.as_str());
        if !audio.matches(&wanted.audio) || !file.as_written {
            let action = if audio.matches(&wanted.audio) {
                Action::Restore
            } else {
                Action::Convert
            };
            plan.added += wanted.bytes;
            plan.freed += freed;
            plan.steps.push(step(
                action,
                new_path(&extension()),
                from,
                Vec::new(),
                wanted.bytes,
            ));
            continue;
        }
        let mut changes = changes(tags, &wanted.tags, cover.as_ref(), wanted.cover.as_ref());
        if !file.listed {
            changes.push(FieldChange {
                field: "Listed on the iPod".into(),
                was: vec!["No".into()],
                now: vec!["Yes".into()],
            });
        }
        if changes.is_empty() {
            plan.unchanged += 1;
        } else {
            plan.steps.push(step(
                Action::Retag,
                file.entry.path.clone(),
                None,
                changes,
                0,
            ));
        }
    }

    for file in &present {
        if claimed.contains(file.entry.path.as_str()) {
            continue;
        }
        let subject = match file.entry.written {
            Written::Track { track, .. } => Subject::Track(track),
            Written::Cover { release, .. } => Subject::Release(release),
        };
        let bytes = if file.as_written {
            file.entry.size as i64
        } else {
            0
        };
        plan.freed += bytes;
        plan.steps.push(Step {
            action: Action::Remove,
            subject,
            path: file.entry.path.clone(),
            from: None,
            changes: Vec::new(),
            bytes,
        });
    }

    plan.left_out = left_out;
    plan.labels = labels;
    plan.present = present.into_iter().map(|file| file.entry).collect();
    plan.wanted = wanted
        .into_iter()
        .map(|wanted| (wanted.track, wanted))
        .collect();
    plan
}

/// Each track's path, told apart from the others' (`unique_paths()`). A
/// track keeps the path it has while it's one of its template's numbered
/// forms, so names don't trade places between syncs. `taken` are paths no
/// track may have.
fn assign(
    wanted: &[WantedTrack],
    current: &HashMap<i64, &str>,
    mut taken: HashSet<String>,
) -> HashMap<i64, String> {
    let mut by_id: Vec<&WantedTrack> = wanted.iter().collect();
    by_id.sort_by_key(|wanted| wanted.track);
    let mut assigned = HashMap::new();
    for wanted in &by_id {
        if let Some(path) = current.get(&wanted.track)
            && wanted.path.number_of(path).is_some()
            && taken.insert(folded(path))
        {
            assigned.insert(wanted.track, path.to_string());
        }
    }
    let mut repeated = Vec::new();
    for wanted in &by_id {
        if assigned.contains_key(&wanted.track) {
            continue;
        }
        let safe = wanted.path.safe();
        if taken.insert(folded(&safe)) {
            assigned.insert(wanted.track, safe);
        } else {
            repeated.push(wanted);
        }
    }
    for wanted in repeated {
        let numbered = (2..)
            .map(|n| wanted.path.numbered(n))
            .find(|numbered| taken.insert(folded(numbered)))
            .expect("a number that's free");
        assigned.insert(wanted.track, numbered);
    }
    assigned
}

/// What changed in a track's tags and cover since they were written, field
/// by field, in the order they're written.
fn changes(
    was: &BTreeMap<String, Vec<String>>,
    now: &Tags,
    was_cover: Option<&CoverKey>,
    now_cover: Option<&CoverKey>,
) -> Vec<FieldChange> {
    let now = now.fields();
    let mut changes: Vec<FieldChange> = now
        .iter()
        .filter_map(|(field, values)| {
            let before = was.get(*field).cloned().unwrap_or_default();
            (&before != values).then(|| FieldChange {
                field: field.to_string(),
                was: before,
                now: values.clone(),
            })
        })
        .collect();
    let named: HashSet<&str> = now.iter().map(|(field, _)| *field).collect();
    for (field, values) in was {
        if !named.contains(field.as_str()) {
            changes.push(FieldChange {
                field: field.clone(),
                was: values.clone(),
                now: Vec::new(),
            });
        }
    }
    if was_cover != now_cover {
        let (was, now) = match (was_cover, now_cover) {
            (Some(_), Some(_)) => (vec!["Old cover".into()], vec!["New cover".into()]),
            (Some(_), None) => (vec!["Cover".into()], Vec::new()),
            _ => (Vec::new(), vec!["Cover".into()]),
        };
        changes.push(FieldChange {
            field: "Cover".into(),
            was,
            now,
        });
    }
    changes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::external::convert::Becomes;
    use crate::external::manifest::AudioKey;
    use crate::external::{SortFields, Template};

    fn tags(title: &str) -> Tags {
        Tags {
            title: title.into(),
            artist: Some("Ryo Fukui".into()),
            artists: vec!["Ryo Fukui".into()],
            album: "Scenery".into(),
            album_artist: Some("Ryo Fukui".into()),
            compilation: false,
            date: None,
            year: None,
            track_number: 1,
            track_total: 1,
            disc_number: 1,
            disc_total: 1,
            disc_title: None,
            labels: vec![],
            catalog_number: None,
            media: vec![],
            release_types: vec![],
            sort: SortFields::default(),
            cover: None,
        }
    }

    fn audio(source: &str) -> AudioKey {
        AudioKey {
            source: source.into(),
            becomes: "copy".into(),
            conversion: 1,
            encoder: "Core Audio".into(),
        }
    }

    /// A track whose template gives `Album/<title>.flac`.
    fn wanted(track: i64, title: &str) -> WantedTrack {
        let tags = tags(title);
        WantedTrack {
            track,
            release: 1,
            path: Template::parse("${album}/${track_title}.${format}")
                .unwrap()
                .path(&tags, "flac"),
            source: PathBuf::from(format!("/music/{track}.flac")),
            becomes: Becomes::Copy,
            audio: audio("a"),
            tags,
            cover: None,
            bytes: 100,
        }
    }

    fn present(track: i64, path: &str, title: &str) -> PresentFile {
        PresentFile {
            entry: Entry {
                path: path.into(),
                size: 100,
                mtime: 1,
                written: Written::Track {
                    track,
                    audio: audio("a"),
                    tags: tags(title)
                        .fields()
                        .into_iter()
                        .map(|(field, values)| (field.to_string(), values))
                        .collect(),
                    cover: None,
                    ipod: None,
                },
            },
            as_written: true,
            listed: true,
        }
    }

    fn steps(plan: &Plan) -> Vec<(Action, Subject, &str)> {
        plan.steps
            .iter()
            .map(|step| (step.action, step.subject, step.path.as_str()))
            .collect()
    }

    #[test]
    fn names_dont_trade_places_between_syncs() {
        // Tracks 1 and 2 had one name, so 2 got " (2)". When 1 leaves, 2
        // keeps its name, and a new track 3 with that name takes the free one
        let inputs = Inputs {
            wanted: vec![wanted(2, "Song"), wanted(3, "Song")],
            present: vec![
                present(1, "Scenery/Song.flac", "Song"),
                present(2, "Scenery/Song (2).flac", "Song"),
            ],
            ..Inputs::default()
        };
        let plan = plan(inputs, &|_| false);
        assert_eq!(
            steps(&plan),
            vec![
                (Action::Add, Subject::Track(3), "Scenery/Song.flac"),
                (Action::Remove, Subject::Track(1), "Scenery/Song.flac"),
            ]
        );
        assert_eq!(plan.unchanged, 1);
    }

    #[test]
    fn a_path_someone_else_uses_is_left_alone() {
        let inputs = Inputs {
            wanted: vec![wanted(1, "Song"), wanted(2, "Other")],
            present: vec![present(2, "Scenery/Before.flac", "Before")],
            ..Inputs::default()
        };
        // A disk that ignores case, with someone's song.flac and other.flac
        let theirs = ["scenery/song.flac", "scenery/other.flac"];
        let plan = plan(inputs, &|path| {
            theirs.contains(&path.to_lowercase().as_str())
        });
        // Other's file stays where it is
        assert!(plan.steps.is_empty(), "{:?}", plan.steps);
        assert_eq!(
            plan.left_out,
            vec![
                LeftOut {
                    subject: Subject::Track(1),
                    reason: "A file liquidslunk didn't write is at Scenery/Song.flac".into(),
                },
                LeftOut {
                    subject: Subject::Track(2),
                    reason: "A file liquidslunk didn't write is at Scenery/Other.flac".into(),
                },
            ]
        );
    }

    #[test]
    fn what_changed_decides_the_step() {
        let mut converted = wanted(1, "Song");
        converted.audio = audio("b");
        let mut missing = present(2, "Scenery/Two.flac", "Two");
        missing.as_written = false;
        let inputs = Inputs {
            wanted: vec![
                converted,
                wanted(2, "Two"),
                wanted(3, "New title"),
                wanted(4, "Case"),
            ],
            kept: HashMap::from([(5, 1)]),
            present: vec![
                present(1, "Scenery/Song.flac", "Song"),
                missing,
                present(3, "Scenery/New title.flac", "Old title"),
                present(4, "Scenery/case.flac", "Case"),
                present(5, "Scenery/Kept.flac", "Kept"),
            ],
            ..Inputs::default()
        };
        let plan = plan(inputs, &|_| false);
        assert_eq!(
            steps(&plan),
            vec![
                (Action::Convert, Subject::Track(1), "Scenery/Song.flac"),
                (Action::Restore, Subject::Track(2), "Scenery/Two.flac"),
                (Action::Retag, Subject::Track(3), "Scenery/New title.flac"),
                (Action::Move, Subject::Track(4), "Scenery/Case.flac"),
            ]
        );
        assert_eq!(
            plan.steps[2].changes,
            vec![FieldChange {
                field: "Title".into(),
                was: vec!["Old title".into()],
                now: vec!["New title".into()],
            }]
        );
        // What it writes, less the old file a conversion replaces
        assert_eq!((plan.added, plan.freed), (200, 100));
    }

    #[test]
    fn an_ipod_names_its_own_files() {
        let mut converted = wanted(1, "Song");
        converted.audio = audio("b");
        let mut unlisted = present(3, "iPod_Control/Music/F01/LSKCCCC.flac", "Three");
        unlisted.listed = false;
        let mut stale = present(4, "iPod_Control/Music/F02/LSKDDDD.flac", "Four");
        stale.as_written = false;
        let inputs = Inputs {
            wanted: vec![
                converted,
                wanted(2, "Two"),
                wanted(3, "Three"),
                wanted(4, "Four"),
                wanted(6, "New"),
            ],
            present: vec![
                present(1, "iPod_Control/Music/F00/LSKAAAA.flac", "Song"),
                present(2, "iPod_Control/Music/F00/LSKBBBB.flac", "Old two"),
                unlisted,
                stale.clone(),
                // A sync cut short left a second file for track 4
                PresentFile {
                    entry: Entry {
                        path: "iPod_Control/Music/F03/LSKEEEE.flac".into(),
                        ..stale.entry.clone()
                    },
                    as_written: true,
                    listed: false,
                },
                present(5, "iPod_Control/Music/F00/LSKFFFF.flac", "Gone"),
            ],
            ..Inputs::default()
        };
        let mut n = 0;
        let plan = plan_ipod(inputs, &mut |extension| {
            n += 1;
            format!("iPod_Control/Music/F09/NEW{n}.{extension}")
        });
        assert_eq!(
            plan.steps
                .iter()
                .map(|step| (
                    step.action,
                    step.subject,
                    step.path.as_str(),
                    step.from.as_deref()
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    Action::Convert,
                    Subject::Track(1),
                    "iPod_Control/Music/F09/NEW1.flac",
                    Some("iPod_Control/Music/F00/LSKAAAA.flac")
                ),
                (
                    Action::Retag,
                    Subject::Track(2),
                    "iPod_Control/Music/F00/LSKBBBB.flac",
                    None
                ),
                (
                    Action::Retag,
                    Subject::Track(3),
                    "iPod_Control/Music/F01/LSKCCCC.flac",
                    None
                ),
                (
                    Action::Retag,
                    Subject::Track(4),
                    "iPod_Control/Music/F03/LSKEEEE.flac",
                    None
                ),
                (
                    Action::Add,
                    Subject::Track(6),
                    "iPod_Control/Music/F09/NEW2.flac",
                    None
                ),
                (
                    Action::Remove,
                    Subject::Track(4),
                    "iPod_Control/Music/F02/LSKDDDD.flac",
                    None
                ),
                (
                    Action::Remove,
                    Subject::Track(5),
                    "iPod_Control/Music/F00/LSKFFFF.flac",
                    None
                ),
            ]
        );
        // A file the database doesn't list is put in it
        assert_eq!(
            plan.steps[2].changes,
            vec![FieldChange {
                field: "Listed on the iPod".into(),
                was: vec!["No".into()],
                now: vec!["Yes".into()],
            }]
        );
    }
}
