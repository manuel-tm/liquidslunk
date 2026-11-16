//! The player (backend.md, "The player"): the queue played through the
//! engine, moving on as each track is heard, skipping tracks that can't
//! play, seeking, coming back from what was remembered, and losing deleted
//! tracks. A device thread renders the engine faster than real time.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use slunk_player::queue::Repeat;
use slunk_player::{Command, Device, Fade, Library, Options, Player, SKIP, State, Update};

mod common;
use common::{RATE, TRACKS, fixture, pcm, signal, write_wav};

/// A track's file, and its length, if the library has one.
type Known = (PathBuf, Option<Duration>);

/// A library of tracks by id, which a test can add to and delete from.
#[derive(Clone, Default)]
struct Tracks(Arc<Mutex<HashMap<i64, Known>>>);

impl Tracks {
    fn add(&self, id: i64, path: PathBuf, frames: Option<usize>) {
        let duration = frames.map(frames_time);
        self.0.lock().unwrap().insert(id, (path, duration));
    }
}

/// Frames at the fixtures' rate, as a time.
fn frames_time(frames: usize) -> Duration {
    Duration::from_secs_f64(frames as f64 / f64::from(RATE))
}

impl Library for Tracks {
    fn path(&self, track_id: i64) -> Option<PathBuf> {
        self.0
            .lock()
            .unwrap()
            .get(&track_id)
            .map(|(path, _)| path.clone())
    }

    fn duration(&self, track_id: i64) -> Option<Duration> {
        self.0.lock().unwrap().get(&track_id)?.1
    }

    fn existing(&self, track_ids: &[i64]) -> Vec<i64> {
        let tracks = self.0.lock().unwrap();
        track_ids
            .iter()
            .copied()
            .filter(|id| tracks.contains_key(id))
            .collect()
    }
}

struct Harness {
    player: Player,
    tracks: Tracks,
    /// Everything the device played
    heard: Arc<Mutex<Vec<f32>>>,
    updates: Receiver<Update>,
    states: Vec<State>,
    failed: Vec<i64>,
    /// The times said, in seconds
    times: Vec<f64>,
    /// What was said to remember
    remembered: Vec<Option<String>>,
}

/// A player of tracks 1, 2 and 3, joined from one signal, and 9, missing.
fn start() -> Harness {
    let tracks = Tracks::default();
    for (index, frames) in TRACKS.iter().enumerate() {
        let path = fixture(&format!("join-{}.flac", index + 1));
        tracks.add(index as i64 + 1, path, Some(*frames));
    }
    tracks.add(9, fixture("missing.flac"), None);
    let (renderers, given) = mpsc::channel();
    let (sender, updates) = mpsc::channel();
    let player = Player::start(Options {
        library: Box::new(tracks.clone()),
        notify: Box::new(move |update| {
            let _ = sender.send(update);
        }),
        device: Device::Given {
            rate: RATE,
            renderers,
        },
    });
    // A device: it plays the latest renderer, a 256-frame buffer a
    // millisecond, about six times as fast as it would play, until the
    // player is gone
    let heard = Arc::new(Mutex::new(Vec::new()));
    let played = heard.clone();
    std::thread::spawn(move || {
        let mut renderer = None;
        let mut block = vec![0.0f32; 512];
        loop {
            match given.try_recv() {
                Ok(given) => renderer = Some(given),
                Err(TryRecvError::Disconnected) => return,
                Err(TryRecvError::Empty) => {}
            }
            if let Some(renderer) = renderer.as_mut() {
                renderer.render(&mut block, 2);
                played.lock().unwrap().extend_from_slice(&block);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    });
    Harness {
        player,
        tracks,
        heard,
        updates,
        states: Vec::new(),
        failed: Vec::new(),
        times: Vec::new(),
        remembered: Vec::new(),
    }
}

impl Harness {
    /// Takes updates until the player's state is as wanted.
    fn until(&mut self, wanted: impl Fn(&State) -> bool) -> State {
        loop {
            if let Update::State(state) = self.next_update()
                && wanted(&state)
            {
                return state;
            }
        }
    }

    /// Takes updates until the time said is as wanted.
    fn until_time(&mut self, wanted: impl Fn(f64) -> bool) -> f64 {
        loop {
            if let Update::Time(time) = self.next_update()
                && wanted(time)
            {
                return time;
            }
        }
    }

    /// Takes updates until what's said to remember is as wanted.
    fn until_remembered(&mut self, wanted: impl Fn(&str) -> bool) -> String {
        loop {
            if let Update::Remembered(Some(saved)) = self.next_update()
                && wanted(&saved)
            {
                return saved;
            }
        }
    }

    fn next_update(&mut self) -> Update {
        let update = self
            .updates
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|_| panic!("nothing more: {:#?}", self.states.last()));
        match &update {
            Update::State(state) => self.states.push(state.clone()),
            Update::Failed(failure) => self.failed.push(failure.track_id),
            Update::Time(time) => self.times.push(*time),
            Update::Remembered(saved) => self.remembered.push(saved.clone()),
        }
        update
    }

    /// What played, without the silence before the first track and after
    /// the last.
    fn played(&self) -> Vec<f32> {
        let heard = self.heard.lock().unwrap().clone();
        let first = heard.iter().position(|sample| *sample != 0.0).unwrap();
        let last = heard.iter().rposition(|sample| *sample != 0.0).unwrap();
        heard[first / 2 * 2..(last / 2 + 1) * 2].to_vec()
    }

    /// The current track of each state seen, once each in a row.
    fn currents(&self) -> Vec<i64> {
        let mut tracks: Vec<i64> = self
            .states
            .iter()
            .filter_map(|state| state.current_entry().map(|entry| entry.track_id))
            .collect();
        tracks.dedup();
        tracks
    }
}

fn track_ids(state: &State) -> Vec<i64> {
    state.entries.iter().map(|entry| entry.track_id).collect()
}

#[test]
fn plays_a_queue_through_and_stops_paused_at_its_last_track() {
    let mut harness = start();
    harness.player.send(Command::Play {
        track_ids: vec![1, 2, 3],
        start: None,
    });
    harness.until(|state| state.playing);
    let stopped = harness.until(|state| !state.playing);
    assert_eq!(stopped.current, Some(2));
    assert_eq!(harness.currents(), [1, 2, 3]);
    assert!(harness.failed.is_empty());
}

#[test]
fn the_queue_plays_joined_to_the_sample_past_a_track_that_cant_play() {
    let mut harness = start();
    harness.player.send(Command::Play {
        track_ids: vec![1, 9, 2, 3],
        start: None,
    });
    harness.until(|state| state.playing);
    harness.until(|state| !state.playing);
    // The fixtures' signal, every sample of it, and nothing else
    assert!(harness.played() == signal(0, TRACKS.iter().sum()));
    assert_eq!(harness.failed, [9]);
}

#[test]
fn a_track_that_cant_play_is_skipped_and_said_as_it_would_play() {
    let mut harness = start();
    harness.player.send(Command::Play {
        track_ids: vec![1, 9, 2],
        start: None,
    });
    harness.until(|state| state.playing);
    harness.until(|state| !state.playing);
    assert_eq!(harness.failed, [9]);
    // It's never current: the queue goes past it as the next one is heard
    assert_eq!(harness.currents(), [1, 2]);
}

#[test]
fn every_track_failing_stops_the_player() {
    let mut harness = start();
    harness.player.send(Command::Play {
        track_ids: vec![9, 9],
        start: None,
    });
    harness.until(|state| state.playing);
    let stopped = harness.until(|state| !state.playing);
    assert_eq!(harness.failed, [9, 9]);
    assert_eq!(stopped.entries.len(), 2);
}

#[test]
fn repeat_one_plays_a_track_again_and_again() {
    let mut harness = start();
    harness.player.send(Command::SetRepeat(Repeat::One));
    harness.player.send(Command::Play {
        track_ids: vec![1, 2],
        start: None,
    });
    harness.until(|state| state.playing);
    // Each time it's heard again, the time starts again
    let mut starts = 0;
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut last = 0.0;
    while starts < 2 && Instant::now() < deadline {
        if let Ok(Update::Time(time)) = harness.updates.recv_timeout(Duration::from_secs(1)) {
            if time < last {
                starts += 1;
            }
            last = time;
        }
    }
    assert_eq!(starts, 2);
    let state = harness.player.state();
    assert_eq!(state.current, Some(0));
    assert!(state.playing);
}

#[test]
fn comes_back_paused_from_what_was_remembered_once() {
    let mut harness = start();
    harness.player.send(Command::Restore {
        queue: Some(r#"{"trackIds":[1,2,3],"current":1,"unshuffled":null}"#.into()),
        volume: 0.5,
        muted: true,
        shuffle: false,
        repeat: Repeat::All,
        fade: Fade::Ms50,
    });
    let restored = harness.until(|state| state.current.is_some());
    assert_eq!(track_ids(&restored), [1, 2, 3]);
    assert_eq!(restored.current, Some(1));
    assert!(!restored.playing);
    assert_eq!(
        (
            restored.volume,
            restored.muted,
            restored.repeat,
            restored.fade
        ),
        (0.5, true, Repeat::All, Fade::Ms50)
    );
    // The page mounting again doesn't change the player
    harness.player.send(Command::Restore {
        queue: None,
        volume: 1.0,
        muted: false,
        shuffle: false,
        repeat: Repeat::Off,
        fade: Fade::Off,
    });
    harness.player.send(Command::Toggle);
    let playing = harness.until(|state| state.playing);
    assert_eq!(track_ids(&playing), [1, 2, 3]);
    assert_eq!(playing.fade, Fade::Ms50);
}

#[test]
fn deleted_tracks_leave_the_queue() {
    let mut harness = start();
    harness.player.send(Command::Add(vec![1, 2, 3]));
    harness.until(|state| state.entries.len() == 3);
    harness.tracks.0.lock().unwrap().remove(&2);
    harness.player.send(Command::Prune);
    let pruned = harness.until(|state| state.entries.len() == 2);
    assert_eq!(track_ids(&pruned), [1, 3]);
}

#[test]
fn the_volume_is_in_hundredths_and_a_change_unmutes() {
    let mut harness = start();
    harness.player.send(Command::SetVolume {
        volume: 0.8,
        muted: true,
    });
    harness.until(|state| state.muted);
    for _ in 0..3 {
        harness
            .player
            .send(Command::ChangeVolume(-slunk_player::VOLUME_STEP));
    }
    let state = harness.until(|state| state.volume < 0.6);
    assert_eq!((state.volume, state.muted), (0.5, false));
}

/// The fixtures' frames in `seconds`, as the engine counts them.
fn frames_in(seconds: f64) -> usize {
    (seconds * f64::from(RATE)).round() as usize
}

#[test]
fn a_seek_while_paused_plays_from_there_and_joins_the_next() {
    let mut harness = start();
    harness.player.send(Command::SetFade(Fade::Off));
    harness.player.send(Command::Add(vec![1, 2]));
    harness.until(|state| state.current.is_some());
    harness.player.send(Command::Seek(0.05));
    // The time is the seek's at once, though nothing is loaded
    harness.until_time(|time| time == 0.05);
    harness.player.send(Command::SetPlaying(true));
    harness.until(|state| state.playing);
    harness.until(|state| !state.playing);
    let from = frames_in(0.05);
    assert!(harness.played() == signal(from, TRACKS[0] + TRACKS[1] - from));
}

#[test]
fn a_seek_while_playing_plays_from_there_exactly() {
    let dir = tempfile::tempdir().unwrap();
    let long = dir.path().join("long.wav");
    let frames = 6 * RATE as usize;
    write_wav(&long, RATE, 2, &pcm(0, frames));
    let mut harness = start();
    harness.tracks.add(4, long, Some(frames));
    harness.player.send(Command::SetFade(Fade::Off));
    harness.player.send(Command::Play {
        track_ids: vec![4],
        start: None,
    });
    harness.until_time(|time| time > 0.2);
    harness.player.send(Command::Seek(5.0));
    // The seek says its time at once, and remembers it, though the engine
    // may not have taken it yet: never where it was before
    let saved = harness.until_remembered(|_| true);
    assert!(saved.ends_with(r#""time":5.0}"#), "{saved}");
    assert_eq!(harness.times.last(), Some(&5.0));
    let seeked = harness.times.len() - 1;
    harness.until(|state| !state.playing);
    // It was cut where it was, and played on from 5 seconds' first sample
    let from = frames_in(5.0);
    assert!(harness.played().ends_with(&signal(from, frames - from)));
    // Its time went on from there, to the end
    assert!(
        harness.times[seeked..]
            .windows(2)
            .all(|pair| pair[1] >= pair[0])
    );
}

#[test]
fn skipping_goes_no_further_back_than_the_start_and_forward_to_the_next() {
    let mut harness = start();
    harness.player.send(Command::Add(vec![1, 2, 3]));
    harness.until(|state| state.current.is_some());
    harness.player.send(Command::Seek(0.05));
    harness.until_time(|time| time == 0.05);
    harness.player.send(Command::Skip(-SKIP));
    harness.until_time(|time| time == 0.0);
    // Past the track's end, the next one, still paused
    harness.player.send(Command::Skip(SKIP));
    let next = harness.until(|state| state.current == Some(1));
    assert!(!next.playing);
}

#[test]
fn a_seek_to_the_last_tracks_end_stops_the_queue_as_its_end_does() {
    let mut harness = start();
    harness.player.send(Command::Play {
        track_ids: vec![3],
        start: None,
    });
    harness.until(|state| state.playing);
    harness.player.send(Command::Seek(1.0));
    let stopped = harness.until(|state| !state.playing);
    assert_eq!(stopped.current, Some(0));
    assert_eq!(harness.player.time(), 0.0);
}

#[test]
fn comes_back_at_its_remembered_time_and_remembers_where_it_is() {
    let mut harness = start();
    harness.player.send(Command::SetFade(Fade::Off));
    harness.player.send(Command::Restore {
        queue: Some(r#"{"trackIds":[1,2],"current":1,"unshuffled":null,"time":0.1}"#.into()),
        volume: 1.0,
        muted: false,
        shuffle: false,
        repeat: Repeat::Off,
        fade: Fade::Off,
    });
    harness.until_time(|time| time == 0.1);
    harness.until_remembered(|saved| {
        saved == r#"{"trackIds":[1,2],"current":1,"unshuffled":null,"time":0.1}"#
    });
    harness.player.send(Command::Toggle);
    harness.until(|state| state.playing);
    harness.until(|state| !state.playing);
    let from = TRACKS[0] + frames_in(0.1);
    assert!(harness.played() == signal(from, TRACKS[0] + TRACKS[1] - from));
    // Stopped at the end, it's remembered at the last track's start
    harness
        .until_remembered(|saved| saved.ends_with(r#""current":1,"unshuffled":null,"time":0.0}"#));
}
