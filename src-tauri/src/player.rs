//! The app's player (backend.md, "The app's player"): `slunk-player`'s
//! player for the open library, made as it opens and stopped as it closes.
//! What it says goes to the webview as events, to the Controls menu and to
//! Now Playing. It reads the library's files by their paths, read-only.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use slunk::catalog::Catalog;
use slunk::media::Media;
use slunk_player::queue;
use slunk_player::{Command, Device, Options, Player, State, Update};
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::commands::catalog::tracks::{PlayerTracksArgs, for_player};
use crate::library::{Libraries, Library};
use crate::menu::{self, Controls};
use crate::now_playing::{self, Info};

/// How the queue repeats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Repeat {
    Off,
    All,
    One,
}

impl From<queue::Repeat> for Repeat {
    fn from(repeat: queue::Repeat) -> Self {
        match repeat {
            queue::Repeat::Off => Repeat::Off,
            queue::Repeat::All => Repeat::All,
            queue::Repeat::One => Repeat::One,
        }
    }
}

impl From<Repeat> for queue::Repeat {
    fn from(repeat: Repeat) -> Self {
        match repeat {
            Repeat::Off => queue::Repeat::Off,
            Repeat::All => queue::Repeat::All,
            Repeat::One => queue::Repeat::One,
        }
    }
}

/// How long a fade is where the player cuts the audio, in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum Fade {
    #[serde(rename = "off")]
    Off,
    #[serde(rename = "10")]
    Ms10,
    #[serde(rename = "50")]
    Ms50,
    #[serde(rename = "200")]
    Ms200,
}

impl Fade {
    pub const ALL: [Fade; 4] = [Fade::Off, Fade::Ms10, Fade::Ms50, Fade::Ms200];
}

impl From<slunk_player::Fade> for Fade {
    fn from(fade: slunk_player::Fade) -> Self {
        match fade {
            slunk_player::Fade::Off => Fade::Off,
            slunk_player::Fade::Ms10 => Fade::Ms10,
            slunk_player::Fade::Ms50 => Fade::Ms50,
            slunk_player::Fade::Ms200 => Fade::Ms200,
        }
    }
}

impl From<Fade> for slunk_player::Fade {
    fn from(fade: Fade) -> Self {
        match fade {
            Fade::Off => slunk_player::Fade::Off,
            Fade::Ms10 => slunk_player::Fade::Ms10,
            Fade::Ms50 => slunk_player::Fade::Ms50,
            Fade::Ms200 => slunk_player::Fade::Ms200,
        }
    }
}

/// A queued track. Its key is its own, so a track queued twice is two.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlayerEntry {
    pub key: u64,
    pub track_id: i64,
}

/// The player, as it is: whenever anything in it changes, and from
/// `player_state`.
#[derive(Debug, Clone, Serialize, Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct PlayerState {
    /// In the order they play
    pub entries: Vec<PlayerEntry>,
    /// The current entry's index, with something queued
    pub current: Option<u32>,
    pub playing: bool,
    /// Whether it plays, but waits for its file
    pub waiting: bool,
    pub shuffle: bool,
    pub repeat: Repeat,
    /// In hundredths, from 0 to 100
    pub volume: u8,
    pub muted: bool,
    pub fade: Fade,
    /// The queue to remember for the library, as JSON, for
    /// `player_restore`; null with nothing queued
    pub remembered: Option<String>,
}

impl From<&State> for PlayerState {
    fn from(state: &State) -> Self {
        PlayerState {
            entries: state
                .entries
                .iter()
                .map(|entry| PlayerEntry {
                    key: entry.key,
                    track_id: entry.track_id,
                })
                .collect(),
            current: state.current.map(|index| index as u32),
            playing: state.playing,
            waiting: state.waiting,
            shuffle: state.shuffle,
            repeat: state.repeat.into(),
            volume: hundredths(state.volume),
            muted: state.muted,
            fade: state.fade.into(),
            remembered: state.remembered.clone(),
        }
    }
}

/// How far into the current track it is: four times a second while it
/// plays, and at once after a change of track or a pause.
#[derive(Debug, Clone, Serialize, Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct PlayerTime {
    /// Milliseconds heard: played, less the output's latency
    pub ms: u64,
}

/// A track that can't play, which the player skipped.
#[derive(Debug, Clone, Serialize, Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct PlayerFailed {
    pub track_id: i64,
    /// Its audio file, as an absolute path
    pub path: Option<String>,
    /// Why, as a sentence's end: "its file can't be read: …"
    pub reason: String,
}

/// The open library's player, in the library. Dropped, it stops, and the
/// Controls menu and Now Playing show nothing.
pub struct LibraryPlayer {
    player: Option<Player>,
    notifier: Arc<Mutex<Notifier>>,
    app: AppHandle,
}

impl LibraryPlayer {
    pub fn send(&self, command: Command) {
        if let Some(player) = &self.player {
            player.send(command);
        }
    }

    pub fn state(&self) -> State {
        self.player.as_ref().map(Player::state).unwrap_or_default()
    }

    /// The library changed: Now Playing reads the current track again.
    pub fn library_changed(&self) {
        lock(&self.notifier).now_playing(true);
    }
}

impl Drop for LibraryPlayer {
    fn drop(&mut self) {
        // Stopped first, so nothing it says comes after
        self.player.take();
        menu::show(&self.app, Controls::NONE);
        now_playing::show(&self.app, None);
    }
}

/// A player for a library that has just opened, with nothing queued: the
/// page restores what it remembers.
pub fn start(app: &AppHandle, library: &Arc<Library>) -> LibraryPlayer {
    let notifier = Arc::new(Mutex::new(Notifier {
        app: app.clone(),
        library: Arc::downgrade(library),
        state: None,
        time: 0.0,
        played: false,
        shown: None,
        sent: None,
    }));
    let notify = {
        let notifier = notifier.clone();
        move |update| lock(&notifier).update(update)
    };
    let player = Player::start(Options {
        library: Box::new(Files {
            catalog: library.catalog.clone(),
            media: library.media.clone(),
        }),
        notify: Box::new(notify),
        device: Device::System,
    });
    LibraryPlayer {
        player: Some(player),
        notifier,
        app: app.clone(),
    }
}

/// Hands the open library's player a command: false without one.
pub fn tell(app: &AppHandle, command: Command) -> bool {
    let Some(libraries) = app.try_state::<Libraries>() else {
        return false;
    };
    let Ok(library) = libraries.library() else {
        return false;
    };
    library.with_player(|player| player.send(command)).is_some()
}

/// What the player reads of the library: tracks' files, and which tracks
/// are still there.
struct Files {
    catalog: Arc<Catalog>,
    media: Arc<Media>,
}

impl slunk_player::Library for Files {
    fn path(&self, track_id: i64) -> Option<PathBuf> {
        let track = self.catalog.tracks_by_ids(&[track_id]).ok()?.pop()?;
        self.media.file_path(track.file_id).ok()?
    }

    fn existing(&self, track_ids: &[i64]) -> Vec<i64> {
        match self.catalog.tracks_by_ids(track_ids) {
            Ok(tracks) => tracks.into_iter().map(|track| track.id).collect(),
            // Every one, rather than losing the queue to an error
            Err(_) => track_ids.to_vec(),
        }
    }
}

/// Hears the player, on its thread: the webview's events, the Controls
/// menu, and Now Playing.
struct Notifier {
    app: AppHandle,
    library: Weak<Library>,
    state: Option<State>,
    /// Seconds heard, as last said
    time: f64,
    /// Whether it has played since the library opened: until then, Now
    /// Playing is left to the app that played last, which the media keys
    /// go to
    played: bool,
    /// The track Now Playing shows, and what it shows of it
    shown: Option<(i64, Info)>,
    /// What Now Playing was last given: its time, whether it played, and
    /// when, to tell how far macOS has moved the time since
    sent: Option<(f64, bool, Instant)>,
}

impl Notifier {
    fn update(&mut self, update: Update) {
        match update {
            Update::State(state) => {
                let _ = PlayerState::from(&state).emit(&self.app);
                menu::show(&self.app, controls(&state));
                let track = |state: &State| state.current_entry().map(|entry| entry.track_id);
                if self.state.as_ref().and_then(track) != track(&state) {
                    // A new track starts at its start: the time that comes
                    // next says so
                    self.time = 0.0;
                }
                self.state = Some(state);
                self.now_playing(false);
            }
            Update::Time(time) => {
                let _ = PlayerTime {
                    ms: (time * 1000.0).round() as u64,
                }
                .emit(&self.app);
                self.time = time;
                self.now_playing(false);
            }
            Update::Failed(failure) => {
                let _ = PlayerFailed {
                    track_id: failure.track_id,
                    path: failure.path.map(|path| path.to_string_lossy().into_owned()),
                    reason: failure.error.to_string(),
                }
                .emit(&self.app);
            }
        }
    }

    /// Shows the current track in Now Playing: its data, read again when
    /// the track changes or with `reread`, and its time, when it plays or
    /// pauses, or has moved from where macOS would have it.
    fn now_playing(&mut self, reread: bool) {
        let Some(state) = &self.state else { return };
        let playing = state.playing;
        self.played |= playing;
        if !self.played {
            return;
        }
        let Some(track_id) = state.current_entry().map(|entry| entry.track_id) else {
            if self.shown.take().is_some() {
                self.sent = None;
                now_playing::show(&self.app, None);
            }
            return;
        };
        let (mut info, changed) = match &self.shown {
            Some((shown, info)) if *shown == track_id && !reread => (info.clone(), false),
            shown => {
                let Some(info) = self.read(track_id) else {
                    return;
                };
                let changed = shown
                    .as_ref()
                    .is_none_or(|(shown, old)| *shown != track_id || !same_track(old, &info));
                (info, changed)
            }
        };
        let moved = self.sent.is_none_or(|(time, was_playing, at)| {
            let moving = if was_playing {
                at.elapsed().as_secs_f64()
            } else {
                0.0
            };
            was_playing != playing || (time + moving - self.time).abs() > 1.0
        });
        if !changed && !moved {
            return;
        }
        info.elapsed = self.time;
        info.playing = playing;
        self.sent = Some((self.time, playing, Instant::now()));
        self.shown = Some((track_id, info.clone()));
        now_playing::show(&self.app, Some(info));
    }

    /// What Now Playing shows of a track, as the player shows it.
    fn read(&self, track_id: i64) -> Option<Info> {
        let library = self.library.upgrade()?;
        let track = for_player(
            &library,
            PlayerTracksArgs {
                ids: vec![track_id],
            },
        )
        .ok()?
        .pop()?;
        let cover = track
            .cover
            .and_then(|cover| library.media.file_path(cover.id).ok().flatten());
        Some(Info {
            artist: track
                .performers
                .iter()
                .map(|performer| match &performer.credited_as {
                    Some(name) => name.name.clone(),
                    None => performer.artist.name.clone(),
                })
                .collect::<Vec<_>>()
                .join(", "),
            album: track.release.title,
            title: track.title,
            duration: track.duration_ms.map(|ms| ms as f64 / 1000.0),
            cover,
            elapsed: 0.0,
            playing: false,
        })
    }
}

/// Whether two tracks' data shown is the same, whatever their times.
fn same_track(a: &Info, b: &Info) -> bool {
    a.title == b.title
        && a.artist == b.artist
        && a.album == b.album
        && a.duration == b.duration
        && a.cover == b.cover
}

/// What the Controls menu shows of the player.
pub fn controls(state: &State) -> Controls {
    Controls {
        playing: state.playing,
        queued: state.current.is_some(),
        shuffle: state.shuffle,
        repeat: state.repeat.into(),
        fade: Some(state.fade.into()),
    }
}

/// A volume from 0 to 1, in hundredths from 0 to 100.
pub fn hundredths(volume: f64) -> u8 {
    (volume.clamp(0.0, 1.0) * 100.0).round() as u8
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
