//! The player's commands (backend.md, "The app's player"). Each hands the open
//! library's player a command and returns once it has taken it: what
//! changed comes back through `PlayerState`. The player is in
//! `crate::player`.

use serde::Deserialize;
use slunk_player::Command;
use specta::Type;
use tauri::{AppHandle, Manager};

use crate::error::CommandError;
use crate::library::Libraries;
use crate::player::{Fade, PlayerState, Repeat};

/// Hands the open library's player a command: `noLibrary` without one.
fn tell(app: &AppHandle, command: Command) -> Result<(), CommandError> {
    let library = app.state::<Libraries>().library()?;
    library
        .with_player(|player| player.send(command))
        .ok_or_else(|| CommandError::internal("The library has no player"))
}

/// The player, as it is, for the page as it mounts.
#[tauri::command]
#[specta::specta]
pub async fn player_state(app: AppHandle) -> Result<PlayerState, CommandError> {
    let library = app.state::<Libraries>().library()?;
    library
        .with_player(|player| PlayerState::from(&player.state()))
        .ok_or_else(|| CommandError::internal("The library has no player"))
}

/// What the page remembers, as the library opens.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RestoreArgs {
    /// The library's queue, as `PlayerState.remembered` was, or anything
    /// else, which gives an empty queue
    pub queue: Option<String>,
    /// In hundredths, from 0 to 100
    pub volume: u8,
    pub muted: bool,
    pub shuffle: bool,
    pub repeat: Repeat,
    pub fade: Fade,
}

/// Brings back what the page remembers: the queue, paused at its current
/// track, and the settings. Only the first call for a library does: after
/// it, the player has its own, and a page that mounts again keeps them.
#[tauri::command]
#[specta::specta]
pub async fn player_restore(app: AppHandle, args: RestoreArgs) -> Result<(), CommandError> {
    tell(
        &app,
        Command::Restore {
            queue: args.queue,
            volume: f64::from(args.volume) / 100.0,
            muted: args.muted,
            shuffle: args.shuffle,
            repeat: args.repeat.into(),
            fade: args.fade.into(),
        },
    )
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlayArgs {
    pub track_ids: Vec<i64>,
    /// The index of the track to play first, or with shuffle on and none, a
    /// random one
    #[serde(default)]
    #[specta(optional)]
    pub start: Option<i64>,
}

/// A new queue of these tracks, playing.
#[tauri::command]
#[specta::specta]
pub async fn player_play(app: AppHandle, args: PlayArgs) -> Result<(), CommandError> {
    tell(
        &app,
        Command::Play {
            track_ids: args.track_ids,
            start: args.start,
        },
    )
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackIdsArgs {
    pub track_ids: Vec<i64>,
}

/// Tracks right after the current one; with nothing queued, a queue,
/// paused.
#[tauri::command]
#[specta::specta]
pub async fn player_play_next(app: AppHandle, args: TrackIdsArgs) -> Result<(), CommandError> {
    tell(&app, Command::PlayNext(args.track_ids))
}

/// Tracks at the end of the queue; with nothing queued, a queue, paused.
#[tauri::command]
#[specta::specta]
pub async fn player_add_to_queue(app: AppHandle, args: TrackIdsArgs) -> Result<(), CommandError> {
    tell(&app, Command::Add(args.track_ids))
}

/// Play or pause.
#[tauri::command]
#[specta::specta]
pub async fn player_toggle(app: AppHandle) -> Result<(), CommandError> {
    tell(&app, Command::Toggle)
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SetPlayingArgs {
    pub playing: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn player_set_playing(app: AppHandle, args: SetPlayingArgs) -> Result<(), CommandError> {
    tell(&app, Command::SetPlaying(args.playing))
}

#[tauri::command]
#[specta::specta]
pub async fn player_next(app: AppHandle) -> Result<(), CommandError> {
    tell(&app, Command::Next)
}

/// The previous track, or in a track's first 3 seconds, its start.
#[tauri::command]
#[specta::specta]
pub async fn player_previous(app: AppHandle) -> Result<(), CommandError> {
    tell(&app, Command::Previous)
}

/// An entry, by its key.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EntryArgs {
    pub key: u64,
}

/// Plays an entry: the current one from its start.
#[tauri::command]
#[specta::specta]
pub async fn player_jump(app: AppHandle, args: EntryArgs) -> Result<(), CommandError> {
    tell(&app, Command::Jump(args.key))
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MoveArgs {
    pub key: u64,
    /// Places in the order they play: up when negative
    pub by: i64,
}

#[tauri::command]
#[specta::specta]
pub async fn player_move(app: AppHandle, args: MoveArgs) -> Result<(), CommandError> {
    tell(
        &app,
        Command::Move {
            key: args.key,
            by: args.by,
        },
    )
}

#[tauri::command]
#[specta::specta]
pub async fn player_remove(app: AppHandle, args: EntryArgs) -> Result<(), CommandError> {
    tell(&app, Command::Remove(args.key))
}

/// Empties the queue.
#[tauri::command]
#[specta::specta]
pub async fn player_clear(app: AppHandle) -> Result<(), CommandError> {
    tell(&app, Command::Clear)
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SetShuffleArgs {
    pub shuffle: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn player_set_shuffle(app: AppHandle, args: SetShuffleArgs) -> Result<(), CommandError> {
    tell(&app, Command::SetShuffle(args.shuffle))
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SetRepeatArgs {
    pub repeat: Repeat,
}

#[tauri::command]
#[specta::specta]
pub async fn player_set_repeat(app: AppHandle, args: SetRepeatArgs) -> Result<(), CommandError> {
    tell(&app, Command::SetRepeat(args.repeat.into()))
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SetVolumeArgs {
    /// In hundredths, from 0 to 100
    pub volume: u8,
    pub muted: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn player_set_volume(app: AppHandle, args: SetVolumeArgs) -> Result<(), CommandError> {
    tell(
        &app,
        Command::SetVolume {
            volume: f64::from(args.volume) / 100.0,
            muted: args.muted,
        },
    )
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SetFadeArgs {
    pub fade: Fade,
}

/// How long a fade is where the player cuts the audio.
#[tauri::command]
#[specta::specta]
pub async fn player_set_fade(app: AppHandle, args: SetFadeArgs) -> Result<(), CommandError> {
    tell(&app, Command::SetFade(args.fade.into()))
}
