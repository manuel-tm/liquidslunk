//! liquidslunk's audio engine (backend.md, "The player engine"): it decodes
//! each track, writes its samples right after the last one's, so tracks join
//! to the sample, and plays them to the system's output. It has no Tauri
//! dependency, and only reads files.

mod convert;
mod decode;
mod engine;
mod output;
mod player;
pub mod queue;
mod render;
mod stream;

pub use decode::TrackDecoder;
pub use engine::{Engine, Event, Fade, Position, Stamped, Track};
pub use output::{Output, OutputError, default_config};
pub use player::{Command, Device, Failure, Library, Options, Player, State, Update, VOLUME_STEP};
pub use render::Renderer;

/// Why a track can't play.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// The file is missing, or can't be read.
    #[error("its file can't be read: {0}")]
    Open(String),
    /// Its format isn't one the engine decodes.
    #[error("its format can't be played: {0}")]
    Unsupported(String),
    /// It broke off while it decoded.
    #[error("it can't be decoded: {0}")]
    Decode(String),
    /// There's no output to play to.
    #[error("there's no audio output: {0}")]
    Output(String),
}
