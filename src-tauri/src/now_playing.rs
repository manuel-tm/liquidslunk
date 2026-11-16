//! Now Playing and the media keys (backend.md, "Now Playing"): the player's
//! track in Control Center, and the buttons there, the keyboard's media keys
//! and headphones' buttons calling the player. macOS's MediaPlayer
//! framework, through objc2-media-player, on the main thread. Elsewhere,
//! there's none.

use std::path::PathBuf;

use tauri::AppHandle;

/// What Now Playing shows of the current track.
#[derive(Debug, Clone, PartialEq)]
pub struct Info {
    pub title: String,
    /// Who performs it, as the player names them, joined with commas
    pub artist: String,
    /// The release
    pub album: String,
    /// In seconds
    pub duration: Option<f64>,
    /// The release's cover, as a file
    pub cover: Option<PathBuf>,
    /// Seconds heard, from which macOS moves the time itself while it plays
    pub elapsed: f64,
    pub playing: bool,
}

/// Shows a track, or nothing with nothing queued.
pub fn show(app: &AppHandle, info: Option<Info>) {
    #[cfg(target_os = "macos")]
    let _ = app.run_on_main_thread(move || macos::show(info));
    #[cfg(not(target_os = "macos"))]
    let _ = (app, info);
}

/// Has the player take Now Playing's buttons and the media keys: play,
/// pause, play/pause, next and previous. The skips and seeks are off, so Now
/// Playing shows previous and next. On the main thread, once.
pub fn setup(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    macos::setup(app);
    #[cfg(not(target_os = "macos"))]
    let _ = app;
}

#[cfg(target_os = "macos")]
mod macos {
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::ptr::NonNull;

    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::NSImage;
    use objc2_core_foundation::CGSize;
    use objc2_foundation::{NSDictionary, NSNumber, NSString};
    use objc2_media_player::{
        MPMediaItemArtwork, MPMediaItemPropertyAlbumTitle, MPMediaItemPropertyArtist,
        MPMediaItemPropertyArtwork, MPMediaItemPropertyPlaybackDuration, MPMediaItemPropertyTitle,
        MPNowPlayingInfoCenter, MPNowPlayingInfoPropertyElapsedPlaybackTime,
        MPNowPlayingInfoPropertyPlaybackRate, MPNowPlayingPlaybackState, MPRemoteCommand,
        MPRemoteCommandCenter, MPRemoteCommandEvent, MPRemoteCommandHandlerStatus,
    };
    use slunk_player::Command;
    use tauri::AppHandle;

    use super::Info;

    thread_local! {
        /// The cover shown, with its file, so it's read once a track
        static ARTWORK: RefCell<Option<(PathBuf, Retained<MPMediaItemArtwork>)>> =
            const { RefCell::new(None) };
    }

    pub fn show(info: Option<Info>) {
        if MainThreadMarker::new().is_none() {
            return;
        }
        // SAFETY: on the main thread, with the keys and values Apple
        // documents for each
        unsafe {
            let center = MPNowPlayingInfoCenter::defaultCenter();
            let Some(info) = info else {
                center.setNowPlayingInfo(None);
                center.setPlaybackState(MPNowPlayingPlaybackState::Stopped);
                ARTWORK.with_borrow_mut(|artwork| *artwork = None);
                return;
            };
            let mut keys: Vec<&NSString> = Vec::new();
            let mut values: Vec<Retained<AnyObject>> = Vec::new();
            let mut add = |key: &'static NSString, value: Retained<AnyObject>| {
                keys.push(key);
                values.push(value);
            };
            add(MPMediaItemPropertyTitle, text(&info.title));
            add(MPMediaItemPropertyArtist, text(&info.artist));
            add(MPMediaItemPropertyAlbumTitle, text(&info.album));
            if let Some(duration) = info.duration {
                add(MPMediaItemPropertyPlaybackDuration, number(duration));
            }
            add(
                MPNowPlayingInfoPropertyElapsedPlaybackTime,
                number(info.elapsed),
            );
            add(
                MPNowPlayingInfoPropertyPlaybackRate,
                number(if info.playing { 1.0 } else { 0.0 }),
            );
            if let Some(artwork) = info.cover.as_deref().and_then(artwork) {
                add(
                    MPMediaItemPropertyArtwork,
                    Retained::into_super(Retained::into_super(artwork)),
                );
            }
            let values: Vec<&AnyObject> = values.iter().map(|value| &**value).collect();
            let dictionary = NSDictionary::from_slices(&keys, &values);
            center.setNowPlayingInfo(Some(&dictionary));
            center.setPlaybackState(if info.playing {
                MPNowPlayingPlaybackState::Playing
            } else {
                MPNowPlayingPlaybackState::Paused
            });
        }
    }

    fn text(text: &str) -> Retained<AnyObject> {
        Retained::into_super(Retained::into_super(NSString::from_str(text)))
    }

    fn number(value: f64) -> Retained<AnyObject> {
        Retained::into_super(Retained::into_super(Retained::into_super(
            NSNumber::new_f64(value),
        )))
    }

    /// The cover's artwork, read from its file once per cover.
    fn artwork(path: &std::path::Path) -> Option<Retained<MPMediaItemArtwork>> {
        let cached = ARTWORK.with_borrow(|artwork| {
            artwork
                .as_ref()
                .filter(|(shown, _)| shown == path)
                .map(|(_, artwork)| artwork.clone())
        });
        if cached.is_some() {
            return cached;
        }
        let image = NSImage::initWithContentsOfFile(
            NSImage::alloc(),
            &NSString::from_str(&path.to_string_lossy()),
        )?;
        let size: CGSize = image.size();
        let handler = RcBlock::new(move |_size: CGSize| NonNull::from(&*image));
        // SAFETY: the handler returns the image it holds, which lives as long
        // as the artwork does
        let artwork = unsafe {
            MPMediaItemArtwork::initWithBoundsSize_requestHandler(
                MPMediaItemArtwork::alloc(),
                size,
                &handler,
            )
        };
        ARTWORK.with_borrow_mut(|shown| *shown = Some((path.to_path_buf(), artwork.clone())));
        Some(artwork)
    }

    pub fn setup(app: &AppHandle) {
        if MainThreadMarker::new().is_none() {
            return;
        }
        // SAFETY: on the main thread; each handler only hands the player a
        // command
        unsafe {
            let center = MPRemoteCommandCenter::sharedCommandCenter();
            let commands: [(Retained<MPRemoteCommand>, Command); 5] = [
                (center.playCommand(), Command::SetPlaying(true)),
                (center.pauseCommand(), Command::SetPlaying(false)),
                (center.togglePlayPauseCommand(), Command::Toggle),
                (center.nextTrackCommand(), Command::Next),
                (center.previousTrackCommand(), Command::Previous),
            ];
            for (remote, command) in commands {
                let app = app.clone();
                let handler = RcBlock::new(move |_event: NonNull<MPRemoteCommandEvent>| {
                    if crate::player::tell(&app, command.clone()) {
                        MPRemoteCommandHandlerStatus::Success
                    } else {
                        MPRemoteCommandHandlerStatus::NoActionableNowPlayingItem
                    }
                });
                remote.setEnabled(true);
                remote.addTargetWithHandler(&handler);
            }
            center.skipForwardCommand().setEnabled(false);
            center.skipBackwardCommand().setEnabled(false);
            center.seekForwardCommand().setEnabled(false);
            center.seekBackwardCommand().setEnabled(false);
            center.changePlaybackPositionCommand().setEnabled(false);
        }
    }
}
