//! The app (design §5.2): the Tauri builder, with its plugins, the open
//! library in its state, the file protocol and the commands. The domain is in
//! `crates/slunk`, which doesn't depend on Tauri.

use tauri::{Manager, RunEvent};

mod commands;
mod error;
mod library;
mod protocol;
mod settings;

use library::Libraries;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let commands = commands::builder();
    #[cfg(debug_assertions)]
    commands
        .export(
            specta_typescript::Typescript::default(),
            concat!(env!("CARGO_MANIFEST_DIR"), "/../src/bindings.ts"),
        )
        .expect("failed to write the TypeScript bindings");

    // One window: a second launch focuses it instead (design §6.4)
    let builder = tauri::Builder::default().plugin(tauri_plugin_single_instance::init(
        |app, _arguments, _folder| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        },
    ));
    // Screenshots and checks drive the app through it. Never ship a build with
    // the feature on.
    #[cfg(feature = "webdriver")]
    let builder = builder.plugin(tauri_plugin_wdio_webdriver::init());

    let app = builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .register_asynchronous_uri_scheme_protocol(protocol::SCHEME, protocol::handle)
        .invoke_handler(commands.invoke_handler())
        .setup(move |app| {
            commands.mount_events(app);
            app.manage(Libraries::new(&app.path().app_config_dir()?));
            #[cfg(debug_assertions)]
            open_library_argument(app.state::<Libraries>().inner());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building liquidslunk");

    app.run(|app, event| {
        if let RunEvent::Exit = event {
            app.state::<Libraries>().close();
        }
    });
}

/// `--library <path>`, in debug builds only, opens a library without touching
/// the settings, for scripts and screenshots (design §6.5).
#[cfg(debug_assertions)]
fn open_library_argument(libraries: &Libraries) {
    let mut arguments = std::env::args().skip_while(|argument| argument != "--library");
    let Some(path) = arguments.nth(1) else {
        return;
    };
    match libraries.open(std::path::Path::new(&path), false, |_| {}) {
        Ok(opened) => eprintln!("Opened {}", opened.library.path),
        Err(error) => eprintln!("Can't open {path}: {error}"),
    }
}
