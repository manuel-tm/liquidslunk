//! The menu bar: Tauri's default menu, with a Controls menu between View and
//! Window, as in Music (backend.md, "The menu bar"). Its items do what the
//! player's buttons do, by calling the open library's player, and they show
//! the player as it is, whenever it says it changed.

use std::sync::{Mutex, PoisonError};

use slunk_player::{Command, SKIP, VOLUME_STEP};
use tauri::menu::{
    CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu,
    WINDOW_SUBMENU_ID,
};
use tauri::{AppHandle, Manager, Wry};

use crate::player::{self, Fade, Repeat};

/// A Controls menu item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Control {
    /// Play or pause
    Toggle,
    Next,
    Previous,
    /// 10 seconds on, as ⇧⌘→ does
    SkipForward,
    SkipBack,
    /// Shuffle on or off
    Shuffle,
    Repeat(Repeat),
    Fade(Fade),
    /// By 10%, as ⌘↑ does
    VolumeUp,
    VolumeDown,
}

impl Control {
    /// Every control, with a menu item each.
    const ALL: [Self; 15] = [
        Self::Toggle,
        Self::Next,
        Self::Previous,
        Self::SkipForward,
        Self::SkipBack,
        Self::Shuffle,
        Self::Repeat(Repeat::Off),
        Self::Repeat(Repeat::All),
        Self::Repeat(Repeat::One),
        Self::Fade(Fade::Off),
        Self::Fade(Fade::Ms10),
        Self::Fade(Fade::Ms50),
        Self::Fade(Fade::Ms200),
        Self::VolumeUp,
        Self::VolumeDown,
    ];

    /// Its menu item's id.
    fn id(self) -> &'static str {
        match self {
            Self::Toggle => "controls.toggle",
            Self::Next => "controls.next",
            Self::Previous => "controls.previous",
            Self::SkipForward => "controls.skipForward",
            Self::SkipBack => "controls.skipBack",
            Self::Shuffle => "controls.shuffle",
            Self::Repeat(Repeat::Off) => "controls.repeat.off",
            Self::Repeat(Repeat::All) => "controls.repeat.all",
            Self::Repeat(Repeat::One) => "controls.repeat.one",
            Self::Fade(Fade::Off) => "controls.fade.off",
            Self::Fade(Fade::Ms10) => "controls.fade.10",
            Self::Fade(Fade::Ms50) => "controls.fade.50",
            Self::Fade(Fade::Ms200) => "controls.fade.200",
            Self::VolumeUp => "controls.volumeUp",
            Self::VolumeDown => "controls.volumeDown",
        }
    }

    /// The control a menu item stands for, if it's one of Controls'.
    fn of_item(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|control| control.id() == id)
    }

    /// What the player does for it. Shuffle turns it to what the menu
    /// doesn't show.
    fn command(self, shown: &Controls) -> Command {
        match self {
            Self::Toggle => Command::Toggle,
            Self::Next => Command::Next,
            Self::Previous => Command::Previous,
            Self::SkipForward => Command::Skip(SKIP),
            Self::SkipBack => Command::Skip(-SKIP),
            Self::Shuffle => Command::SetShuffle(!shown.shuffle),
            Self::Repeat(mode) => Command::SetRepeat(mode.into()),
            Self::Fade(fade) => Command::SetFade(fade.into()),
            Self::VolumeUp => Command::ChangeVolume(VOLUME_STEP),
            Self::VolumeDown => Command::ChangeVolume(-VOLUME_STEP),
        }
    }
}

/// What the Controls menu shows: the player, as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Controls {
    /// Pause instead of Play
    pub playing: bool,
    /// Every item but Fade's is off without it
    pub queued: bool,
    pub shuffle: bool,
    pub repeat: Repeat,
    /// The fade's length, with a library open; Fade is off without one
    pub fade: Option<Fade>,
}

impl Controls {
    /// No library open, as at Welcome.
    pub const NONE: Self = Self {
        playing: false,
        queued: false,
        shuffle: false,
        repeat: Repeat::Off,
        fade: None,
    };
}

/// The Controls menu's items, in the app's state.
pub struct ControlsMenu {
    toggle: MenuItem<Wry>,
    /// Next, Previous, the skips, Volume Up and Volume Down
    others: Vec<MenuItem<Wry>>,
    shuffle: CheckMenuItem<Wry>,
    repeat: Submenu<Wry>,
    repeats: Vec<(Repeat, CheckMenuItem<Wry>)>,
    fade: Submenu<Wry>,
    fades: Vec<(Fade, CheckMenuItem<Wry>)>,
    /// What they show, which a check item is put back to when it's clicked
    shown: Mutex<Controls>,
}

impl ControlsMenu {
    /// Shows the player as it is. On the main thread.
    fn show(&self, controls: Controls) -> tauri::Result<()> {
        *self.shown.lock().unwrap_or_else(PoisonError::into_inner) = controls;
        self.apply(controls)
    }

    fn shown(&self) -> Controls {
        *self.shown.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn apply(&self, controls: Controls) -> tauri::Result<()> {
        let Controls {
            playing,
            queued,
            shuffle,
            repeat,
            fade,
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
        self.fade.set_enabled(fade.is_some())?;
        for (length, item) in &self.fades {
            item.set_checked(Some(*length) == fade)?;
        }
        Ok(())
    }
}

/// Shows the player in the Controls menu, from any thread: the menu is
/// changed on the main thread, in the order it's told.
pub fn show(app: &AppHandle, controls: Controls) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(menu) = handle.try_state::<ControlsMenu>()
            && let Err(error) = menu.show(controls)
        {
            eprintln!("Can't update the Controls menu: {error}");
        }
    });
}

/// The menu bar: Tauri's default menu, with Controls before Window. Its
/// items are off until a library's player says otherwise.
pub fn build(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let item = |control: Control, text: &str, keys: Option<&str>| {
        MenuItem::with_id(app, control.id(), text, false, keys)
    };
    // Space can't be Play's: it would take Space from every field. The
    // player handles it in the webview (frontend.md, "The player").
    let toggle = item(Control::Toggle, "Play", None)?;
    let next = item(Control::Next, "Next", Some("CmdOrCtrl+Right"))?;
    let previous = item(Control::Previous, "Previous", Some("CmdOrCtrl+Left"))?;
    let skip_forward = item(
        Control::SkipForward,
        "Skip Forward",
        Some("Shift+CmdOrCtrl+Right"),
    )?;
    let skip_back = item(Control::SkipBack, "Skip Back", Some("Shift+CmdOrCtrl+Left"))?;
    let volume_up = item(Control::VolumeUp, "Volume Up", Some("CmdOrCtrl+Up"))?;
    let volume_down = item(Control::VolumeDown, "Volume Down", Some("CmdOrCtrl+Down"))?;
    let shuffle = CheckMenuItem::with_id(
        app,
        Control::Shuffle.id(),
        "Shuffle",
        false,
        false,
        None::<&str>,
    )?;
    let check = |control: Control, text: &str, checked: bool| {
        CheckMenuItem::with_id(app, control.id(), text, true, checked, None::<&str>)
    };
    let repeats = [
        (Repeat::Off, "Off"),
        (Repeat::All, "All"),
        (Repeat::One, "One"),
    ]
    .into_iter()
    .map(|(mode, text)| {
        Ok((
            mode,
            check(Control::Repeat(mode), text, mode == Repeat::Off)?,
        ))
    })
    .collect::<tauri::Result<Vec<_>>>()?;
    let repeat = Submenu::with_items(app, "Repeat", false, &items(&repeats))?;
    let fades = [
        (Fade::Off, "Off"),
        (Fade::Ms10, "10 ms"),
        (Fade::Ms50, "50 ms"),
        (Fade::Ms200, "200 ms"),
    ]
    .into_iter()
    .map(|(fade, text)| Ok((fade, check(Control::Fade(fade), text, fade == Fade::Ms10)?)))
    .collect::<tauri::Result<Vec<_>>>()?;
    let fade = Submenu::with_items(app, "Fade", false, &items(&fades))?;
    let controls = Submenu::with_items(
        app,
        "Controls",
        true,
        &[
            &toggle,
            &next,
            &previous,
            &skip_forward,
            &skip_back,
            &PredefinedMenuItem::separator(app)?,
            &shuffle,
            &repeat,
            &fade,
            &PredefinedMenuItem::separator(app)?,
            &volume_up,
            &volume_down,
        ],
    )?;

    let menu = Menu::default(app)?;
    let window = menu
        .items()?
        .iter()
        .position(|item| item.id() == WINDOW_SUBMENU_ID)
        .unwrap_or(menu.items()?.len());
    menu.insert(&controls, window)?;
    app.manage(ControlsMenu {
        toggle,
        others: vec![
            next,
            previous,
            skip_forward,
            skip_back,
            volume_up,
            volume_down,
        ],
        shuffle,
        repeat,
        repeats,
        fade,
        fades,
        shown: Mutex::new(Controls::NONE),
    });
    Ok(menu)
}

fn items<T>(items: &[(T, CheckMenuItem<Wry>)]) -> Vec<&dyn IsMenuItem<Wry>> {
    items
        .iter()
        .map(|(_, item)| item as &dyn IsMenuItem<Wry>)
        .collect()
}

/// A click on a Controls item: the player is told. A check item checks or
/// unchecks itself when it's clicked, so it's put back as it was, until the
/// player says what's changed.
pub fn on_event(app: &AppHandle, event: MenuEvent) {
    let Some(control) = Control::of_item(event.id().as_ref()) else {
        return;
    };
    let Some(menu) = app.try_state::<ControlsMenu>() else {
        return;
    };
    let shown = menu.shown();
    if let Err(error) = menu.apply(shown) {
        eprintln!("Can't update the Controls menu: {error}");
    }
    player::tell(app, control.command(&shown));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_control_has_its_own_item() {
        for control in Control::ALL {
            assert_eq!(Control::of_item(control.id()), Some(control));
        }
        assert_eq!(Control::of_item("controls.stop"), None);
    }

    #[test]
    fn shuffle_turns_to_what_the_menu_doesnt_show() {
        let shown = Controls {
            shuffle: true,
            ..Controls::NONE
        };
        assert_eq!(Control::Shuffle.command(&shown), Command::SetShuffle(false));
        assert_eq!(
            Control::Fade(Fade::Ms50).command(&shown),
            Command::SetFade(slunk_player::Fade::Ms50)
        );
        assert_eq!(Control::SkipBack.command(&shown), Command::Skip(-10.0));
    }
}
