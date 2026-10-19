//! The menu bar: Tauri's default menu, with a Controls menu between View and
//! Window, as in Music (2_player.md §8.2). Its items do what the player's
//! buttons do: a click is a `PlayerControl` event, which the player in the
//! webview acts on. The queue lives there too, so what the items show comes
//! from the player, through `set_controls_menu`.

use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::menu::{
    CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu, WINDOW_SUBMENU_ID,
};
use tauri::{AppHandle, Manager, Wry};
use tauri_specta::Event;

/// How the queue repeats, as `lib/queue.ts` has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Repeat {
    Off,
    All,
    One,
}

/// A Controls menu item that was clicked, for the player to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PlayerControl {
    /// Play or pause
    Toggle,
    Next,
    Previous,
    /// Shuffle on or off
    Shuffle,
    Repeat {
        mode: Repeat,
    },
    /// By 10%, as ⌘↑ does
    VolumeUp,
    VolumeDown,
}

impl PlayerControl {
    /// Every control, with a menu item each.
    const ALL: [Self; 9] = [
        Self::Toggle,
        Self::Next,
        Self::Previous,
        Self::Shuffle,
        Self::Repeat { mode: Repeat::Off },
        Self::Repeat { mode: Repeat::All },
        Self::Repeat { mode: Repeat::One },
        Self::VolumeUp,
        Self::VolumeDown,
    ];

    /// Its menu item's id.
    fn id(self) -> &'static str {
        match self {
            Self::Toggle => "controls.toggle",
            Self::Next => "controls.next",
            Self::Previous => "controls.previous",
            Self::Shuffle => "controls.shuffle",
            Self::Repeat { mode: Repeat::Off } => "controls.repeat.off",
            Self::Repeat { mode: Repeat::All } => "controls.repeat.all",
            Self::Repeat { mode: Repeat::One } => "controls.repeat.one",
            Self::VolumeUp => "controls.volumeUp",
            Self::VolumeDown => "controls.volumeDown",
        }
    }

    /// The control a menu item stands for, if it's one of Controls'.
    fn of_item(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|control| control.id() == id)
    }
}

/// What the Controls menu shows: the player, as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Controls {
    /// Pause instead of Play
    pub playing: bool,
    /// Every item is off without it
    pub queued: bool,
    pub shuffle: bool,
    pub repeat: Repeat,
}

impl Controls {
    /// Nothing queued, as at Welcome.
    const NONE: Self = Self {
        playing: false,
        queued: false,
        shuffle: false,
        repeat: Repeat::Off,
    };
}

/// The Controls menu's items, in the app's state, for `set_controls_menu`.
pub struct ControlsMenu {
    toggle: MenuItem<Wry>,
    /// Next, Previous, Volume Up and Volume Down
    others: Vec<MenuItem<Wry>>,
    shuffle: CheckMenuItem<Wry>,
    repeat: Submenu<Wry>,
    repeats: Vec<(Repeat, CheckMenuItem<Wry>)>,
    /// What they show, which a check item is put back to when it's clicked
    shown: Mutex<Controls>,
}

impl ControlsMenu {
    /// Shows the player as it is.
    pub fn show(&self, controls: Controls) -> tauri::Result<()> {
        *self.shown.lock().unwrap_or_else(PoisonError::into_inner) = controls;
        self.apply(controls)
    }

    /// Shows again what was last shown.
    fn reshow(&self) -> tauri::Result<()> {
        let controls = *self.shown.lock().unwrap_or_else(PoisonError::into_inner);
        self.apply(controls)
    }

    fn apply(&self, controls: Controls) -> tauri::Result<()> {
        let Controls {
            playing,
            queued,
            shuffle,
            repeat,
        } = controls;
        self.toggle
            .set_text(if playing { "Pause" } else { "Play" })?;
        self.toggle.set_enabled(queued)?;
        for item in &self.others {
            item.set_enabled(queued)?;
        }
        self.shuffle.set_enabled(queued)?;
        self.shuffle.set_checked(shuffle)?;
        self.repeat.set_enabled(queued)?;
        for (mode, item) in &self.repeats {
            item.set_checked(*mode == repeat)?;
        }
        Ok(())
    }
}

/// The menu bar: Tauri's default menu, with Controls before Window. Its
/// items are off until the player says something is queued.
pub fn build(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let item = |control: PlayerControl, text: &str, keys: Option<&str>| {
        MenuItem::with_id(app, control.id(), text, false, keys)
    };
    // Space can't be Play's: it would take Space from every field. The
    // player handles it in the webview (2_player.md §4.4).
    let toggle = item(PlayerControl::Toggle, "Play", None)?;
    let next = item(PlayerControl::Next, "Next", Some("CmdOrCtrl+Right"))?;
    let previous = item(PlayerControl::Previous, "Previous", Some("CmdOrCtrl+Left"))?;
    let volume_up = item(PlayerControl::VolumeUp, "Volume Up", Some("CmdOrCtrl+Up"))?;
    let volume_down = item(
        PlayerControl::VolumeDown,
        "Volume Down",
        Some("CmdOrCtrl+Down"),
    )?;
    let shuffle = CheckMenuItem::with_id(
        app,
        PlayerControl::Shuffle.id(),
        "Shuffle",
        false,
        false,
        None::<&str>,
    )?;
    let repeats = [
        (Repeat::Off, "Off"),
        (Repeat::All, "All"),
        (Repeat::One, "One"),
    ]
    .into_iter()
    .map(|(mode, text)| {
        let id = PlayerControl::Repeat { mode }.id();
        let item = CheckMenuItem::with_id(app, id, text, true, mode == Repeat::Off, None::<&str>)?;
        Ok((mode, item))
    })
    .collect::<tauri::Result<Vec<_>>>()?;
    let repeat = Submenu::with_items(
        app,
        "Repeat",
        false,
        &repeats
            .iter()
            .map(|(_, item)| item as &dyn tauri::menu::IsMenuItem<Wry>)
            .collect::<Vec<_>>(),
    )?;
    let controls = Submenu::with_items(
        app,
        "Controls",
        true,
        &[
            &toggle,
            &next,
            &previous,
            &PredefinedMenuItem::separator(app)?,
            &shuffle,
            &repeat,
            &PredefinedMenuItem::separator(app)?,
            &volume_up,
            &volume_down,
        ],
    )?;

    let menu = Menu::default(app)?;
    let items = menu.items()?;
    let window = items
        .iter()
        .position(|item| item.id() == WINDOW_SUBMENU_ID)
        .unwrap_or(items.len());
    menu.insert(&controls, window)?;
    app.manage(ControlsMenu {
        toggle,
        others: vec![next, previous, volume_up, volume_down],
        shuffle,
        repeat,
        repeats,
        shown: Mutex::new(Controls::NONE),
    });
    Ok(menu)
}

/// A click on a Controls item: the player is told. A check item checks or
/// unchecks itself when it's clicked, so it's put back as it was, until the
/// player says what's changed.
pub fn on_event(app: &AppHandle, event: MenuEvent) {
    let Some(control) = PlayerControl::of_item(event.id().as_ref()) else {
        return;
    };
    if let Some(menu) = app.try_state::<ControlsMenu>()
        && let Err(error) = menu.reshow()
    {
        eprintln!("Can't update the Controls menu: {error}");
    }
    if let Err(error) = control.emit(app) {
        eprintln!("Can't send {control:?} to the player: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_control_has_its_own_item() {
        for control in PlayerControl::ALL {
            assert_eq!(PlayerControl::of_item(control.id()), Some(control));
        }
        assert_eq!(PlayerControl::of_item("controls.stop"), None);
    }

    #[test]
    fn a_control_is_tagged_with_its_type() {
        let repeat = serde_json::to_value(PlayerControl::Repeat { mode: Repeat::All }).unwrap();
        assert_eq!(
            repeat,
            serde_json::json!({ "type": "repeat", "mode": "all" })
        );
        let volume = serde_json::to_value(PlayerControl::VolumeUp).unwrap();
        assert_eq!(volume, serde_json::json!({ "type": "volumeUp" }));
    }
}
