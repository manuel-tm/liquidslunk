//! The player's queue (backend.md, "The queue"): the tracks it plays, in
//! order, and the one that's current. A plain value and the steps that
//! change it, with no engine, so its rules are tested on their own. The
//! player takes each step, and makes the engine follow. Shuffling takes its
//! seed from the step, so a step always does the same.

use std::collections::HashMap;

use serde::Serialize;
use serde_json::Value;

/// A queued track. Its key is its own, so a track queued twice is two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Entry {
    pub key: u64,
    pub track_id: i64,
}

/// Off, all (the first entry follows the last), or one (it plays again).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

impl Repeat {
    /// The next setting: off, all, one, then off again.
    pub fn next(self) -> Self {
        match self {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        }
    }
}

/// What changes the queue.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// A new queue, playing from the track at `start`. Shuffled, that track
    /// plays first, or a random one without `start`, and the rest after it.
    Play {
        track_ids: Vec<i64>,
        start: Option<i64>,
        seed: u32,
    },
    /// Right after the current entry; with nothing queued, a queue, paused
    PlayNext {
        track_ids: Vec<i64>,
    },
    /// At the end; with nothing queued, a queue, paused
    Add {
        track_ids: Vec<i64>,
    },
    Toggle,
    SetPlaying(bool),
    Next,
    /// `time` is how far into the current track it is, in seconds
    Previous {
        time: f64,
    },
    /// The current track played to its end, with nothing joined to it
    Ended,
    /// What was up next was heard, joined to the current track's end
    Handoff,
    /// The current track can't play: missing, or a format that can't
    Failed,
    /// The current track was heard
    Started,
    Jump {
        key: u64,
    },
    /// Moves an entry `by` places in the order they play
    Move {
        key: u64,
        by: i64,
    },
    Remove {
        key: u64,
    },
    /// Leaves only entries whose track is one of these: the others are gone
    Keep {
        track_ids: Vec<i64>,
    },
    Clear,
    SetShuffle {
        shuffle: bool,
        seed: u32,
    },
    SetRepeat(Repeat),
}

/// Previous goes back to the track's start once this much has played, in
/// seconds.
pub const RESTART_AFTER: f64 = 3.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Queue {
    /// In the order they play
    pub entries: Vec<Entry>,
    /// The current entry's index, with something queued
    pub current: Option<usize>,
    /// Whether the current entry plays, rather than waits paused
    pub playing: bool,
    /// Goes up whenever the current entry goes back to its start
    pub restarts: u64,
    /// Entries that failed in a row, since one last played
    pub failures: usize,
    /// The key the next entry gets
    pub next_key: u64,
    pub shuffle: bool,
    /// While shuffled, the entries in their order before shuffling
    pub unshuffled: Option<Vec<Entry>>,
    pub repeat: Repeat,
}

impl Default for Queue {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            current: None,
            playing: false,
            restarts: 0,
            failures: 0,
            next_key: 1,
            shuffle: false,
            unshuffled: None,
            repeat: Repeat::Off,
        }
    }
}

impl Queue {
    /// The current entry, or none with nothing queued.
    pub fn current_entry(&self) -> Option<Entry> {
        self.current
            .and_then(|index| self.entries.get(index).copied())
    }

    /// The entry that plays when the current one ends, by the rules `Ended`
    /// follows: the next one; after the last, the first with repeat all, or
    /// nothing; with repeat one, the current one again.
    pub fn up_next(&self) -> Option<Entry> {
        let current = self.current?;
        if self.repeat == Repeat::One {
            return self.current_entry();
        }
        if current + 1 < self.entries.len() {
            return Some(self.entries[current + 1]);
        }
        if self.repeat == Repeat::All {
            self.entries.first().copied()
        } else {
            None
        }
    }

    /// The entries that follow the current one as it ends, one after
    /// another, by the same rules, without coming round to it again; with
    /// repeat one, or one entry on repeat all, the current one again.
    pub fn following(&self) -> impl Iterator<Item = Entry> + '_ {
        let none: &[Entry] = &[];
        let (again, after, round) = match self.current {
            None => (None, none, none),
            Some(current)
                if self.repeat == Repeat::One
                    || (self.repeat == Repeat::All && self.entries.len() == 1) =>
            {
                (Some(self.entries[current]), none, none)
            }
            Some(current) => (
                None,
                &self.entries[current + 1..],
                if self.repeat == Repeat::All {
                    &self.entries[..current]
                } else {
                    none
                },
            ),
        };
        again
            .into_iter()
            .chain(after.iter().copied())
            .chain(round.iter().copied())
    }

    /// Takes a step.
    pub fn step(&mut self, step: Step) {
        let queued = self.current.is_some();
        match step {
            Step::Play {
                track_ids,
                start,
                seed,
            } => self.play(&track_ids, start, seed),
            Step::PlayNext { track_ids } => self.insert(&track_ids, false),
            Step::Add { track_ids } => self.insert(&track_ids, true),
            Step::Toggle => {
                if queued {
                    self.playing = !self.playing;
                }
            }
            Step::SetPlaying(playing) => {
                if queued {
                    self.playing = playing;
                }
            }
            Step::Next => {
                if queued {
                    self.advance(self.playing);
                }
            }
            Step::Previous { time } => {
                let Some(current) = self.current else { return };
                if time > RESTART_AFTER {
                    self.restart(self.playing);
                } else if current > 0 {
                    self.current = Some(current - 1);
                } else if self.repeat == Repeat::All && self.entries.len() > 1 {
                    self.current = Some(self.entries.len() - 1);
                } else {
                    self.restart(self.playing);
                }
            }
            Step::Ended => {
                if !queued {
                    return;
                }
                if self.repeat == Repeat::One {
                    self.restart(true);
                } else {
                    self.advance(true);
                }
            }
            Step::Handoff => {
                // As `Ended`, but what's up next already plays from its
                // start: it doesn't restart, even when it's the same entry
                let Some(next) = self.up_next() else {
                    return self.step(Step::Ended);
                };
                self.current = self.entries.iter().position(|entry| *entry == next);
                self.playing = true;
            }
            Step::Failed => {
                if !queued {
                    return;
                }
                self.failures += 1;
                // Every entry failed: stop, rather than going round. Repeat
                // one doesn't try the same track again.
                if self.failures >= self.entries.len() {
                    self.playing = false;
                } else {
                    self.advance(self.playing);
                }
            }
            Step::Started => self.failures = 0,
            Step::Jump { key } => {
                let Some(index) = self.index_of(key) else {
                    return;
                };
                if Some(index) == self.current {
                    self.restart(true);
                } else {
                    self.current = Some(index);
                    self.playing = true;
                }
            }
            Step::Move { key, by } => {
                let Some(from) = self.index_of(key) else {
                    return;
                };
                let to = from as i64 + by;
                if to < 0 || to >= self.entries.len() as i64 || to == from as i64 {
                    return;
                }
                let current = self.current_entry();
                let entry = self.entries.remove(from);
                self.entries.insert(to as usize, entry);
                self.current = current.and_then(|current| self.position(current));
            }
            Step::Remove { key } => self.without(|entry| entry.key == key),
            Step::Keep { track_ids } => {
                self.without(|entry| !track_ids.contains(&entry.track_id));
            }
            Step::Clear => self.clear(),
            Step::SetShuffle { shuffle, seed } => {
                if shuffle != self.shuffle {
                    if shuffle {
                        self.shuffle_rest(seed);
                    } else {
                        self.unshuffle();
                    }
                }
            }
            Step::SetRepeat(repeat) => self.repeat = repeat,
        }
    }

    fn index_of(&self, key: u64) -> Option<usize> {
        self.entries.iter().position(|entry| entry.key == key)
    }

    fn position(&self, entry: Entry) -> Option<usize> {
        self.entries.iter().position(|other| *other == entry)
    }

    fn entries_of(&mut self, track_ids: &[i64]) -> Vec<Entry> {
        let entries = track_ids
            .iter()
            .enumerate()
            .map(|(index, &track_id)| Entry {
                key: self.next_key + index as u64,
                track_id,
            })
            .collect();
        self.next_key += track_ids.len() as u64;
        entries
    }

    fn play(&mut self, track_ids: &[i64], start: Option<i64>, seed: u32) {
        if track_ids.is_empty() {
            return self.clear();
        }
        let entries = self.entries_of(track_ids);
        self.playing = true;
        self.failures = 0;
        let clamp = |index: i64| index.clamp(0, entries.len() as i64 - 1) as usize;
        if !self.shuffle {
            self.current = Some(clamp(start.unwrap_or(0)));
            self.entries = entries;
            self.unshuffled = None;
            return;
        }
        let mut random = generator(seed);
        let first = entries
            [clamp(start.unwrap_or_else(|| (random() * entries.len() as f64).floor() as i64))];
        let rest: Vec<Entry> = entries
            .iter()
            .copied()
            .filter(|entry| *entry != first)
            .collect();
        self.entries = std::iter::once(first)
            .chain(shuffled(rest, &mut random))
            .collect();
        self.current = Some(0);
        self.unshuffled = Some(entries);
    }

    /// Play next, or Add to queue at the end.
    fn insert(&mut self, track_ids: &[i64], at_end: bool) {
        if track_ids.is_empty() {
            return;
        }
        let added = self.entries_of(track_ids);
        let Some(current) = self.current else {
            self.unshuffled = self.shuffle.then(|| added.clone());
            self.entries = added;
            self.current = Some(0);
            self.playing = false;
            self.failures = 0;
            return;
        };
        let current_entry = self.entries[current];
        let at = if at_end {
            self.entries.len()
        } else {
            current + 1
        };
        self.entries.splice(at..at, added.iter().copied());
        if let Some(unshuffled) = self.unshuffled.as_mut() {
            let at = if at_end {
                unshuffled.len()
            } else {
                unshuffled
                    .iter()
                    .position(|entry| *entry == current_entry)
                    .map_or(0, |index| index + 1)
            };
            unshuffled.splice(at..at, added);
        }
    }

    /// The next entry, playing or not; after the last, by repeat.
    fn advance(&mut self, playing: bool) {
        let Some(current) = self.current else { return };
        if current + 1 < self.entries.len() {
            self.current = Some(current + 1);
            self.playing = playing;
        } else if self.repeat == Repeat::All {
            if self.entries.len() > 1 {
                self.current = Some(0);
                self.playing = playing;
            } else {
                self.restart(playing);
            }
        } else {
            // Repeat one only plays a track again when it ends
            self.restart(false);
        }
    }

    /// The current entry, from its start.
    fn restart(&mut self, playing: bool) {
        self.playing = playing;
        self.restarts += 1;
    }

    fn clear(&mut self) {
        *self = Queue {
            restarts: self.restarts,
            next_key: self.next_key,
            shuffle: self.shuffle,
            unshuffled: self.shuffle.then(Vec::new),
            repeat: self.repeat,
            ..Queue::default()
        };
    }

    /// Shuffle on: the entries after the current one, in a random order.
    fn shuffle_rest(&mut self, seed: u32) {
        let split = self.current.map_or(0, |current| current + 1);
        let rest = self.entries.split_off(split);
        self.unshuffled = Some(
            self.entries
                .iter()
                .copied()
                .chain(rest.iter().copied())
                .collect(),
        );
        self.entries.extend(shuffled(rest, &mut generator(seed)));
        self.shuffle = true;
    }

    /// Shuffle off: the entries after the current one, in their order
    /// before.
    fn unshuffle(&mut self) {
        let order: HashMap<Entry, usize> = self
            .unshuffled
            .take()
            .unwrap_or_default()
            .into_iter()
            .enumerate()
            .map(|(index, entry)| (entry, index))
            .collect();
        let split = self.current.map_or(0, |current| current + 1);
        let mut rest = self.entries.split_off(split);
        rest.sort_by_key(|entry| order.get(entry).copied().unwrap_or(0));
        self.entries.extend(rest);
        self.shuffle = false;
    }

    /// The queue without the entries `removed` picks. When the current one
    /// goes, the next one left is current, playing as it was; with none
    /// after it, the last one left is, stopped at its start.
    fn without(&mut self, removed: impl Fn(&Entry) -> bool) {
        if !self.entries.iter().any(&removed) {
            return;
        }
        let entries: Vec<Entry> = self
            .entries
            .iter()
            .copied()
            .filter(|entry| !removed(entry))
            .collect();
        if entries.is_empty() {
            return self.clear();
        }
        if let Some(unshuffled) = self.unshuffled.as_mut() {
            unshuffled.retain(|entry| !removed(entry));
        }
        // The first entry left from the current one on
        let next = self.current.and_then(|current| {
            self.entries[current..]
                .iter()
                .copied()
                .find(|entry| !removed(entry))
        });
        self.entries = entries;
        match next {
            Some(next) => self.current = self.position(next),
            None => {
                self.current = Some(self.entries.len() - 1);
                self.restart(false);
            }
        }
    }

    /// The queue to remember, as JSON, or none when nothing's queued: its
    /// tracks in the order they play, the current one, and while shuffled,
    /// their order before, as indexes into `trackIds`.
    pub fn remembered(&self) -> Option<String> {
        let current = self.current?;
        let saved = Saved {
            track_ids: self.entries.iter().map(|entry| entry.track_id).collect(),
            current,
            unshuffled: self.unshuffled.as_ref().map(|unshuffled| {
                unshuffled
                    .iter()
                    .filter_map(|entry| self.position(*entry))
                    .collect()
            }),
        };
        serde_json::to_string(&saved).ok()
    }

    /// A remembered queue, paused at its current entry, with these shuffle
    /// and repeat. Anything that isn't a queue, like what an older version
    /// remembered, gives an empty one.
    pub fn restored(saved: Option<&str>, shuffle: bool, repeat: Repeat) -> Queue {
        let empty = Queue {
            shuffle,
            unshuffled: shuffle.then(Vec::new),
            repeat,
            ..Queue::default()
        };
        let Some(saved) = saved.and_then(parse_saved) else {
            return empty;
        };
        let entries: Vec<Entry> = saved
            .track_ids
            .iter()
            .enumerate()
            .map(|(index, &track_id)| Entry {
                key: index as u64 + 1,
                track_id,
            })
            .collect();
        let unshuffled = shuffle.then(|| match &saved.unshuffled {
            Some(order) => order.iter().map(|&index| entries[index]).collect(),
            None => entries.clone(),
        });
        Queue {
            next_key: entries.len() as u64 + 1,
            current: Some(saved.current),
            entries,
            unshuffled,
            ..empty
        }
    }
}

/// What's remembered of a queue.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Saved {
    track_ids: Vec<i64>,
    current: usize,
    unshuffled: Option<Vec<usize>>,
}

/// A remembered queue, if it is one: tracks' ids, a current one among them,
/// and a shuffled order that has each index once.
fn parse_saved(saved: &str) -> Option<Saved> {
    let value: Value = serde_json::from_str(saved).ok()?;
    let object = value.as_object()?;
    let track_ids: Vec<i64> = object
        .get("trackIds")?
        .as_array()?
        .iter()
        .map(|id| id.as_i64().filter(|id| *id > 0 && *id < 1 << 53))
        .collect::<Option<_>>()?;
    if track_ids.is_empty() {
        return None;
    }
    let index = |value: &Value| {
        value
            .as_u64()
            .map(|index| index as usize)
            .filter(|index| *index < track_ids.len())
    };
    let current = index(object.get("current")?)?;
    let unshuffled = match object.get("unshuffled") {
        None | Some(Value::Null) => None,
        Some(order) => {
            let order: Vec<usize> = order.as_array()?.iter().map(index).collect::<Option<_>>()?;
            let mut seen = vec![false; track_ids.len()];
            if order.len() != track_ids.len()
                || order
                    .iter()
                    .any(|&index| std::mem::replace(&mut seen[index], true))
            {
                return None;
            }
            Some(order)
        }
    };
    Some(Saved {
        track_ids,
        current,
        unshuffled,
    })
}

/// A random order, Fisher–Yates, with the generator's numbers.
fn shuffled(mut items: Vec<Entry>, random: &mut impl FnMut() -> f64) -> Vec<Entry> {
    for i in (1..items.len()).rev() {
        let j = (random() * (i + 1) as f64).floor() as usize;
        items.swap(i, j);
    }
    items
}

/// Numbers from 0 to 1, the same ones for the same seed (mulberry32).
fn generator(seed: u32) -> impl FnMut() -> f64 {
    let mut state = seed;
    move || {
        state = state.wrapping_add(0x6d2b_79f5);
        let mut t = state;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }
}
