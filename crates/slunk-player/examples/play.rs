//! Plays files in order through the engine, to the default output, for
//! checking it by ear and with a recording of what it plays (backend.md,
//! "The player engine").
//!
//!   cargo run -p slunk-player --example play -- [--volume 0.5] [--fade 10]
//!       [--start SECONDS] a.flac b.flac …
//!
//! `--start` begins the first file that many seconds in, decoding its start
//! and leaving it out. `--fade` is off, 10, 50 or 200 (ms). A file that
//! can't play stops the files after it, as the engine leaves what follows a
//! failure to its owner.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use slunk_player::{Engine, Event, Fade, Output, Track, TrackDecoder, default_config};

fn main() {
    let mut volume = 1.0f32;
    let mut fade = Fade::Ms10;
    let mut start = 0.0f64;
    let mut files = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--volume" => volume = args.next().and_then(|v| v.parse().ok()).unwrap_or(1.0),
            "--fade" => {
                fade = match args.next().as_deref() {
                    Some("off") => Fade::Off,
                    Some("50") => Fade::Ms50,
                    Some("200") => Fade::Ms200,
                    _ => Fade::Ms10,
                }
            }
            "--start" => start = args.next().and_then(|v| v.parse().ok()).unwrap_or(0.0),
            _ => files.push(PathBuf::from(arg)),
        }
    }
    if files.is_empty() {
        eprintln!("usage: play [--volume V] [--fade off|10|50|200] [--start S] FILE…");
        std::process::exit(2);
    }

    let (rate, channels) = default_config().unwrap_or_else(|error| fail(error));
    let (events_in, events) = mpsc::channel();
    let (engine, renderer) = Engine::new(rate, move |stamped| {
        let _ = events_in.send(stamped);
    });
    engine.set_volume(volume);
    engine.set_fade(fade);
    let output = Output::start(renderer).unwrap_or_else(|error| fail(error));
    println!(
        "output: {rate} Hz, {channels} channels; pid {}",
        std::process::id()
    );

    let tracks: Vec<Track> = files
        .iter()
        .enumerate()
        .map(|(index, path)| {
            let mut track = Track::new(index as u64, path);
            if index == 0 && start > 0.0 {
                let file_rate = TrackDecoder::open(path).map(|d| d.rate()).unwrap_or(rate);
                track.start = (start * f64::from(file_rate)) as u64;
            }
            track
        })
        .collect();

    let began = Instant::now();
    engine.play(tracks);
    let mut last_report = Instant::now();
    loop {
        while let Ok(stamped) = events.try_recv() {
            let at = began.elapsed().as_secs_f64();
            match stamped.event {
                Event::Started { entry } => {
                    // The wall clock too, to line a recording up with
                    let epoch = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0.0, |since| since.as_secs_f64() * 1000.0);
                    println!(
                        "{at:9.3} s  started {} (epoch ms {epoch:.1})",
                        files[entry as usize].display()
                    );
                }
                Event::Failed { entry, error } => {
                    println!(
                        "{at:9.3} s  failed {}: {error}",
                        files[entry as usize].display()
                    );
                }
                Event::Ended => {
                    println!("{at:9.3} s  ended; underruns {} frames", engine.underruns());
                    return;
                }
            }
        }
        for error in output.errors() {
            println!(
                "{:9.3} s  output: {}{}",
                began.elapsed().as_secs_f64(),
                error.message,
                if error.lost { " (lost)" } else { "" }
            );
        }
        if last_report.elapsed() >= Duration::from_secs(5) {
            last_report = Instant::now();
            let position = engine.position();
            println!(
                "{:9.3} s  at {:.3} s of entry {:?}; latency {:.1} ms; underruns {}",
                began.elapsed().as_secs_f64(),
                position.frames as f64 / f64::from(position.rate),
                position.entry,
                engine.latency().as_secs_f64() * 1000.0,
                engine.underruns()
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn fail(error: slunk_player::Error) -> ! {
    eprintln!("{error}");
    std::process::exit(1)
}
