//! The engine's rules (backend.md, "The player engine"): decoding every
//! sample of a track's music, joining tracks to the sample, what's up next
//! changing, resampling, the volume and the fades. A fake output keeps every
//! frame the renderer gives it.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use slunk_player::{Engine, Event, Fade, Renderer, Stamped, Track, TrackDecoder};

mod common;
use common::{RATE, TRACKS, fixture, offset, pcm, signal};

/// A whole file, decoded as the engine decodes it.
fn decode(path: &Path) -> Vec<f32> {
    let mut decoder = TrackDecoder::open(path).unwrap();
    let mut out = Vec::new();
    while decoder.next(&mut out).unwrap() {}
    out
}

/// A WAV file of 16-bit interleaved samples.
fn write_wav(path: &Path, rate: u32, channels: u16, samples: &[i16]) {
    let data: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
    bytes.extend_from_slice(&(channels * 2).to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&data);
    fs::write(path, bytes).unwrap();
}

/// An AIFF file of 16-bit interleaved stereo at 44.1 kHz, whose rate is an
/// 80-bit extended float: 2^15 × 0xAC44 / 2^15.
fn write_aiff(path: &Path, samples: &[i16]) {
    let data: Vec<u8> = samples.iter().flat_map(|s| s.to_be_bytes()).collect();
    let frames = (samples.len() / 2) as u32;
    let mut comm = Vec::new();
    comm.extend_from_slice(&2u16.to_be_bytes());
    comm.extend_from_slice(&frames.to_be_bytes());
    comm.extend_from_slice(&16u16.to_be_bytes());
    comm.extend_from_slice(&[0x40, 0x0e, 0xac, 0x44, 0, 0, 0, 0, 0, 0]);
    let mut body = b"AIFF".to_vec();
    body.extend_from_slice(b"COMM");
    body.extend_from_slice(&(comm.len() as u32).to_be_bytes());
    body.extend_from_slice(&comm);
    body.extend_from_slice(b"SSND");
    body.extend_from_slice(&(8 + data.len() as u32).to_be_bytes());
    body.extend_from_slice(&[0; 8]);
    body.extend_from_slice(&data);
    let mut bytes = b"FORM".to_vec();
    bytes.extend_from_slice(&(body.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&body);
    fs::write(path, bytes).unwrap();
}

/// A second of a constant level, 0.5, in stereo.
fn level(dir: &Path) -> PathBuf {
    let path = dir.join("level.wav");
    write_wav(&path, RATE, 2, &vec![16384; RATE as usize * 2]);
    path
}

/// An engine with a fake output, which renders only frames already written,
/// so nothing it keeps is silence for want of samples. It has no latency, so
/// what it renders is heard.
struct Fake {
    engine: Engine,
    renderer: Renderer,
    received: Receiver<Stamped>,
    out: Vec<f32>,
    /// Every epoch's
    events: Vec<Event>,
}

impl Fake {
    fn new(rate: u32) -> Self {
        let (sender, received) = mpsc::channel();
        let (engine, renderer) = Engine::new(rate, move |stamped| {
            let _ = sender.send(stamped);
        });
        Self {
            engine,
            renderer,
            received,
            out: Vec::new(),
            events: Vec::new(),
        }
    }

    fn hear(&mut self) {
        self.events
            .extend(self.received.try_iter().map(|stamped| stamped.event));
    }

    /// Waits for an event, rendering nothing.
    fn wait_for(&mut self, wanted: impl Fn(&Event) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            self.hear();
            if self.events.iter().any(&wanted) {
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        panic!("no such event: {:?}", self.events);
    }

    /// Renders `frames`, once they're written.
    fn render(&mut self, frames: usize) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.renderer.available() < frames && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        let mut block = vec![0.0; frames * 2];
        self.renderer.render(&mut block, 2);
        self.out.extend_from_slice(&block);
        self.hear();
    }

    /// Renders everything to the end.
    fn render_all(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            self.hear();
            if self.events.contains(&Event::Ended) {
                return;
            }
            let available = self.renderer.available().min(512);
            if available == 0 {
                std::thread::sleep(Duration::from_millis(1));
                continue;
            }
            let mut block = vec![0.0; available * 2];
            self.renderer.render(&mut block, 2);
            self.out.extend_from_slice(&block);
        }
        panic!("the engine didn't end");
    }

    /// Lets the decoder write all it will.
    fn settle(&self) {
        std::thread::sleep(Duration::from_millis(300));
    }
}

fn started(events: &[Event]) -> Vec<u64> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Started { entry } => Some(*entry),
            _ => None,
        })
        .collect()
}

#[test]
fn flac_and_apple_lossless_decode_to_the_signal() {
    for (index, frames) in TRACKS.iter().enumerate() {
        let expected = signal(offset(index), *frames);
        for extension in ["flac", "m4a"] {
            let decoded = decode(&fixture(&format!("join-{}.{extension}", index + 1)));
            assert!(decoded == expected, "join-{}.{extension}", index + 1);
        }
    }
}

#[test]
fn aac_is_trimmed_by_its_itunsmpb() {
    // afconvert's 2,112 frames of delay, and its padding, are left out
    assert_eq!(decode(&fixture("aac.m4a")).len() / 2, TRACKS[0]);
}

#[test]
fn mp3_is_trimmed_by_its_lame_tag() {
    // 20 frames of 1,152, less the delay and padding the tag says
    assert_eq!(
        decode(&fixture("lame.mp3")).len() / 2,
        20 * 1152 - 576 - 1000
    );
}

#[test]
fn tracks_join_to_the_sample() {
    let mut fake = Fake::new(RATE);
    fake.engine.play(vec![
        Track::new(0, fixture("join-1.flac")),
        Track::new(1, fixture("join-2.m4a")),
        Track::new(2, fixture("join-3.flac")),
    ]);
    fake.render_all();
    assert!(fake.out == signal(0, TRACKS.iter().sum()));
    assert_eq!(started(&fake.events), [0, 1, 2]);
    assert_eq!(fake.engine.underruns(), 0);
}

#[test]
fn wav_and_aiff_decode_to_the_signal_and_join() {
    let dir = tempfile::tempdir().unwrap();
    let wav = dir.path().join("join-1.wav");
    let aiff = dir.path().join("join-2.aiff");
    write_wav(&wav, RATE, 2, &pcm(0, TRACKS[0]));
    write_aiff(&aiff, &pcm(offset(1), TRACKS[1]));
    assert!(decode(&wav) == signal(0, TRACKS[0]));
    assert!(decode(&aiff) == signal(offset(1), TRACKS[1]));

    let mut fake = Fake::new(RATE);
    fake.engine.play(vec![
        Track::new(0, &wav),
        Track::new(1, &aiff),
        Track::new(2, fixture("join-3.flac")),
    ]);
    fake.render_all();
    assert!(fake.out == signal(0, TRACKS.iter().sum()));
}

#[test]
fn what_is_up_next_joins_the_end_of_what_plays() {
    let mut fake = Fake::new(RATE);
    fake.engine
        .play(vec![Track::new(0, fixture("join-1.flac"))]);
    fake.settle();
    fake.engine
        .set_upcoming(0, vec![Track::new(1, fixture("join-2.flac"))]);
    fake.render_all();
    assert!(fake.out == signal(0, TRACKS[0] + TRACKS[1]));
}

#[test]
fn what_was_up_next_is_dropped_unheard() {
    let mut fake = Fake::new(RATE);
    fake.engine.play(vec![
        Track::new(0, fixture("join-1.flac")),
        Track::new(1, fixture("join-2.flac")),
    ]);
    // The decoder has written the second track by now
    fake.settle();
    fake.engine
        .set_upcoming(0, vec![Track::new(2, fixture("join-3.flac"))]);
    fake.settle();
    fake.render_all();
    let mut expected = signal(0, TRACKS[0]);
    expected.extend(signal(offset(2), TRACKS[2]));
    assert!(fake.out == expected);
    assert_eq!(started(&fake.events), [0, 2]);
}

#[test]
fn what_is_up_next_after_another_entry_is_ignored() {
    let mut fake = Fake::new(RATE);
    fake.engine.play(vec![
        Track::new(0, fixture("join-1.flac")),
        Track::new(1, fixture("join-2.flac")),
    ]);
    fake.settle();
    // Out of date: it follows an entry that isn't the one playing
    fake.engine
        .set_upcoming(7, vec![Track::new(2, fixture("join-3.flac"))]);
    fake.render_all();
    assert!(fake.out == signal(0, TRACKS[0] + TRACKS[1]));
    assert_eq!(started(&fake.events), [0, 1]);
}

#[test]
fn what_is_up_next_follows_the_first_entry_before_it_opens() {
    let mut fake = Fake::new(RATE);
    fake.engine.play(vec![
        Track::new(0, fixture("join-1.flac")),
        Track::new(1, fixture("join-3.flac")),
    ]);
    fake.engine
        .set_upcoming(0, vec![Track::new(2, fixture("join-2.flac"))]);
    fake.render_all();
    assert!(fake.out == signal(0, TRACKS[0] + TRACKS[1]));
    assert_eq!(started(&fake.events), [0, 2]);
}

#[test]
fn a_track_repeated_joins_its_own_start() {
    let mut fake = Fake::new(RATE);
    fake.engine.play(vec![
        Track::new(0, fixture("join-2.flac")),
        Track::new(1, fixture("join-2.flac")),
    ]);
    fake.render_all();
    let mut expected = signal(offset(1), TRACKS[1]);
    expected.extend(signal(offset(1), TRACKS[1]));
    assert!(fake.out == expected);
}

fn failed(events: &[Event]) -> Vec<u64> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Failed { entry, .. } => Some(*entry),
            _ => None,
        })
        .collect()
}

#[test]
fn a_track_that_fails_stops_what_was_to_follow_it() {
    let mut fake = Fake::new(RATE);
    fake.engine.play(vec![
        Track::new(0, fixture("join-1.flac")),
        Track::new(1, fixture("missing.flac")),
        Track::new(2, fixture("join-2.flac")),
    ]);
    fake.render_all();
    assert!(fake.out == signal(0, TRACKS[0]));
    assert_eq!(started(&fake.events), [0]);
    assert_eq!(failed(&fake.events), [1]);
}

#[test]
fn what_follows_a_failure_joins_where_it_would_have_played() {
    let mut fake = Fake::new(RATE);
    fake.engine.play(vec![
        Track::new(0, fixture("join-1.flac")),
        Track::new(1, fixture("missing.flac")),
    ]);
    fake.wait_for(|event| matches!(event, Event::Failed { entry: 1, .. }));
    fake.engine
        .set_upcoming(0, vec![Track::new(2, fixture("generate.ts"))]);
    fake.wait_for(|event| matches!(event, Event::Failed { entry: 2, .. }));
    fake.engine
        .set_upcoming(0, vec![Track::new(3, fixture("join-2.flac"))]);
    fake.render_all();
    assert!(fake.out == signal(0, TRACKS[0] + TRACKS[1]));
    assert_eq!(started(&fake.events), [0, 3]);
    assert_eq!(failed(&fake.events), [1, 2]);
}

#[test]
fn a_first_track_that_fails_takes_no_upcoming() {
    let mut fake = Fake::new(RATE);
    fake.engine
        .play(vec![Track::new(0, fixture("missing.flac"))]);
    fake.wait_for(|event| matches!(event, Event::Failed { entry: 0, .. }));
    // Nothing plays to follow
    fake.engine
        .set_upcoming(0, vec![Track::new(1, fixture("join-1.flac"))]);
    fake.settle();
    fake.hear();
    assert_eq!(fake.renderer.available(), 0);
    assert!(started(&fake.events).is_empty());
}

#[test]
fn events_are_stamped_with_their_play() {
    let (sender, received) = mpsc::channel();
    let (engine, mut renderer) = Engine::new(RATE, move |stamped| {
        let _ = sender.send(stamped);
    });
    engine.play(vec![Track::new(0, fixture("join-1.flac"))]);
    assert_eq!(engine.epoch(), 1);
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut block = vec![0.0; 1024];
    let first = loop {
        renderer.render(&mut block, 2);
        if let Ok(stamped) = received.try_recv() {
            break stamped;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(
        first,
        Stamped {
            epoch: 1,
            event: Event::Started { entry: 0 }
        }
    );
    engine.stop();
    assert_eq!(engine.epoch(), 2);
}

#[test]
fn a_track_at_another_rate_is_resampled_without_a_gap() {
    let dir = tempfile::tempdir().unwrap();
    // A tenth of a second of 1 kHz at 48 kHz, between two at 44.1 kHz
    let other = dir.path().join("48k.wav");
    let tone: Vec<i16> = (0..4800)
        .flat_map(|frame| {
            let value = (8192.0
                * (2.0 * std::f64::consts::PI * 1000.0 * f64::from(frame) / 48000.0).sin())
                as i16;
            [value, value]
        })
        .collect();
    write_wav(&other, 48000, 2, &tone);

    let mut fake = Fake::new(RATE);
    fake.engine.play(vec![
        Track::new(0, fixture("join-1.flac")),
        Track::new(1, &other),
        Track::new(2, fixture("join-3.flac")),
    ]);
    fake.render_all();
    // 4,800 frames at 48 kHz are 4,410 at 44.1 kHz, exactly where the next
    // track starts
    let resampled = 4410;
    assert_eq!(fake.out.len() / 2, TRACKS[0] + resampled + TRACKS[2]);
    assert!(fake.out[..TRACKS[0] * 2] == signal(0, TRACKS[0])[..]);
    assert!(fake.out[(TRACKS[0] + resampled) * 2..] == signal(offset(2), TRACKS[2])[..]);
    // In between, the tone, away from where the resampler starts and ends
    for frame in 200..resampled - 200 {
        let expected = 0.25 * (2.0 * std::f64::consts::PI * 1000.0 * frame as f64 / 44100.0).sin();
        let got = f64::from(fake.out[(TRACKS[0] + frame) * 2]);
        assert!(
            (got - expected).abs() < 0.002,
            "frame {frame}: {got} for {expected}"
        );
    }
}

#[test]
fn pausing_fades_out_and_playing_fades_in() {
    let dir = tempfile::tempdir().unwrap();
    for (fade, frames) in [(Fade::Ms10, 441), (Fade::Ms50, 2205)] {
        let mut fake = Fake::new(RATE);
        fake.engine.set_fade(fade);
        fake.engine.play(vec![Track::new(0, level(dir.path()))]);
        fake.render(1000);
        fake.engine.set_playing(false);
        fake.render(frames + 100);
        let paused = &fake.out[2000..];
        for (index, sample) in paused.iter().step_by(2).enumerate() {
            let expected = 0.5 * (1.0 - (index + 1) as f32 / frames as f32).max(0.0);
            assert!((sample - expected).abs() < 1e-4, "{fade:?}, frame {index}");
        }
        fake.engine.set_playing(true);
        let from = fake.out.len();
        fake.render(frames + 100);
        for (index, sample) in fake.out[from..].iter().step_by(2).enumerate() {
            let expected = 0.5 * ((index + 1) as f32 / frames as f32).min(1.0);
            assert!((sample - expected).abs() < 1e-4, "{fade:?}, frame {index}");
        }
    }
}

#[test]
fn with_fades_off_a_pause_cuts() {
    let dir = tempfile::tempdir().unwrap();
    let mut fake = Fake::new(RATE);
    fake.engine.set_fade(Fade::Off);
    fake.engine.play(vec![Track::new(0, level(dir.path()))]);
    fake.render(1000);
    fake.engine.set_playing(false);
    fake.render(100);
    assert!(fake.out[2000..].iter().all(|sample| *sample == 0.0));
    fake.engine.set_playing(true);
    fake.render(100);
    assert!(fake.out[2200..].iter().all(|sample| *sample == 0.5));
}

#[test]
fn the_volume_moves_without_a_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut fake = Fake::new(RATE);
    fake.engine.play(vec![Track::new(0, level(dir.path()))]);
    fake.render(1000);
    fake.engine.set_volume(0.5);
    fake.render(1000);
    let step = fake
        .out
        .windows(3)
        .step_by(2)
        .map(|pair| (pair[2] - pair[0]).abs())
        .fold(0.0, f32::max);
    assert!(step <= 0.5 / 441.0 + 1e-6, "a step of {step}");
    assert_eq!(*fake.out.last().unwrap(), 0.25);
}

#[test]
fn a_new_queue_fades_out_what_plays_and_starts_at_its_first_sample() {
    let dir = tempfile::tempdir().unwrap();
    let mut fake = Fake::new(RATE);
    fake.engine.play(vec![Track::new(0, level(dir.path()))]);
    fake.render(1000);
    fake.engine
        .play(vec![Track::new(1, fixture("join-1.flac"))]);
    fake.settle();
    fake.render_all();
    let after = &fake.out[2000..];
    // The level fades out over 10 ms
    for (index, sample) in after[..441 * 2].iter().step_by(2).enumerate() {
        let expected = 0.5 * (1.0 - (index + 1) as f32 / 441.0);
        assert!((sample - expected).abs() < 1e-4, "frame {index}");
    }
    // Then the track, from its first sample, unfaded
    assert!(after[441 * 2..] == signal(0, TRACKS[0])[..]);
    assert_eq!(started(&fake.events), [0, 1]);
}
