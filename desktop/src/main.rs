mod assets;
mod dashboard;
mod fixtures;
mod managed_storage;
mod model;
mod sync_runtime;
use gamesync_desktop::settings;
mod system_accent;
mod theme;
mod ui;
mod watcher;

use gpui::{
    px, size, App, AppContext as _, Application, Bounds, KeyBinding, Menu, MenuItem, WindowBounds,
    WindowOptions,
};
use gpui_component::Root;
use ui::app::GameSyncApp;

/// Wayland app ID and X11 class. Must match `StartupWMClass` in the Linux
/// desktop entry so the compositor links windows to the launcher entry.
const APP_ID: &str = "gamesync";

gpui::actions!(
    gamesync,
    [
        Quit,
        FocusSearch,
        RefreshLibrary,
        SaveDetails,
        ManageCollections,
        OpenSettings,
        OpenSyncSettings,
        SyncSteam,
        ConnectSteam
    ]
);

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .filter_module("reqwest", log::LevelFilter::Off)
        .filter_module("hyper", log::LevelFilter::Off)
        .filter_module("hyper_util", log::LevelFilter::Off)
        .init();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // Opening the dedicated prototype bundle from Finder must stay in demo
    // mode, even when Launch Services supplies no command-line flags.
    let prototype_bundle = std::env::current_exe().is_ok_and(|path| {
        path.ancestors()
            .any(|part| part.file_name() == Some(std::ffi::OsStr::new("BestOnDemo.app")))
    });
    let play_now_bundle = std::env::current_exe().is_ok_and(|path| {
        path.ancestors()
            .any(|part| part.file_name() == Some(std::ffi::OsStr::new("PlayNowDemo.app")))
    });
    if play_now_bundle || args.iter().any(|arg| arg == "--play-now-demo") {
        if !args.iter().any(|arg| arg == "--play-now-demo") {
            args.push("--play-now-demo".into());
        }
        // Launch Services drops shell flags and environment. This bundle must
        // never open the user's normal library or load their sync settings.
        if settings::preview_dir().is_none() {
            std::env::set_var(
                "GAMESYNC_PREVIEW_DIR",
                std::env::temp_dir().join("gamesync-play-now-demo"),
            );
        }
    }
    if prototype_bundle && !args.iter().any(|arg| arg == "--best-on-demo") {
        args.push("--best-on-demo".into());
    }
    if args.iter().any(|arg| arg == "--card-proof") {
        return ui::card_proof::run();
    }
    anyhow::ensure!(
        !args.iter().any(|arg| arg == "--library"),
        "Folder opening has been removed. GameSync manages storage for Steam sync."
    );
    let preview = args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--stress" | "--missing-covers" | "--empty"));
    let play_now_demo = args.iter().any(|arg| arg == "--play-now-demo");
    let best_on_demo = args.iter().any(|arg| arg == "--best-on-demo");
    let sample = args.iter().any(|arg| arg == "--demo");
    let initial_path = if play_now_demo {
        Some(managed_storage::play_now_demo_path()?)
    } else if best_on_demo {
        Some(managed_storage::best_on_demo_path()?)
    } else if preview {
        None
    } else {
        Some(managed_storage::path(sample)?)
    };
    // Demo data is opt-in. A normal first launch must not look like a real library.
    let demo_requested = args.iter().any(|arg| {
        matches!(
            arg.as_str(),
            "--demo" | "--stress" | "--missing-covers" | "--best-on-demo" | "--play-now-demo"
        )
    });
    let mut games = if play_now_demo {
        fixtures::play_now_games()?
    } else if best_on_demo {
        fixtures::best_on_games()?
    } else if demo_requested {
        fixtures::games()?
    } else {
        Vec::new()
    };
    if args.iter().any(|arg| arg == "--empty") {
        games.clear();
    }
    if args.iter().any(|arg| arg == "--stress") {
        let original = games.clone();
        if !original.is_empty() {
            games = (0..10_000)
                .map(|index| {
                    let mut game = original[index % original.len()].clone();
                    game.id = uuid::Uuid::from_u128(100_000_000 + index as u128);
                    game.title = format!("{} {:05}", game.title, index);
                    game
                })
                .collect();
        }
    }
    if args.iter().any(|arg| arg == "--missing-covers") {
        for game in &mut games {
            game.cover = "covers/missing.jpg".into();
        }
    }
    let small_window = args.iter().any(|arg| arg == "--small-window");
    let mut library = model::Library::new(games);
    library.best_on_demo = best_on_demo;
    if play_now_demo {
        library.show_play_now();
    }
    if best_on_demo {
        library.name = "Best on · Demo".into();
        library.set_scope(model::Scope::Smart(
            gamesync_desktop::smart::SmartRule::BestOn(
                gamesync_desktop::suitability::BestOn::SteamDeck,
            ),
        ));
    }
    if !demo_requested {
        library.name = "My games".into();
        library.demo = false;
    }
    let initial_theme = settings::load()
        .map(|settings| settings.appearance())
        .unwrap_or_default();
    // Omarchy is the native look on Linux when its theme state exists. The
    // legacy `omarchy_mode` flag is no longer read; Native is the default look.
    let omarchy_mode = initial_theme.native(gamesync_desktop::omarchy::is_available())
        == Some(gamesync_desktop::appearance::NativePlatform::Omarchy);
    let initial_omarchy = omarchy_mode
        .then(gamesync_desktop::omarchy::OmarchyTheme::load_active)
        .transpose()
        .ok()
        .flatten();

    Application::new()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
            gpui_component::init(cx);
            cx.on_window_closed(|cx| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            cx.bind_keys([
                KeyBinding::new("secondary-q", Quit, None),
                KeyBinding::new("secondary-f", FocusSearch, None),
                KeyBinding::new("secondary-r", RefreshLibrary, None),
                KeyBinding::new("secondary-s", SaveDetails, None),
                KeyBinding::new("secondary-,", OpenSettings, None),
            ]);
            cx.set_menus(vec![Menu {
                name: "GameSync".into(),
                items: vec![
                    MenuItem::action("Settings…", OpenSettings),
                    MenuItem::action("Refresh Library", RefreshLibrary),
                    MenuItem::action("Quit GameSync", Quit),
                ],
            }]);
            cx.activate(true);
            let result = cx.open_window(
                WindowOptions {
                    app_id: Some(APP_ID.into()),
                    titlebar: Some(ui::chrome::titlebar_options(library.name.clone())),
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        if small_window {
                            size(px(960.), px(600.))
                        } else {
                            size(px(1280.), px(820.))
                        },
                        cx,
                    ))),
                    window_min_size: Some(size(px(960.), px(600.))),
                    ..Default::default()
                },
                move |window, cx| {
                    if let Some(theme) = &initial_omarchy {
                        theme::apply_omarchy(theme, window, cx);
                    } else {
                        theme::apply_choice(&initial_theme, window, cx);
                    }
                    let view = cx.new(|cx| {
                        GameSyncApp::new(
                            library,
                            initial_path,
                            initial_theme,
                            omarchy_mode,
                            initial_omarchy,
                            window,
                            cx,
                        )
                    });
                    let close_view = view.downgrade();
                    window.on_window_should_close(cx, move |_, cx| {
                        close_view
                            .update(cx, |app, cx| app.can_close(cx))
                            .unwrap_or(true)
                    });
                    // App commands stay available when a popup owns keyboard focus.
                    let settings_view = view.downgrade();
                    cx.on_action(move |_: &OpenSettings, cx| {
                        let _ = settings_view.update(cx, |app, cx| app.open_settings(cx));
                    });
                    let sync_view = view.downgrade();
                    cx.on_action(move |_: &OpenSyncSettings, cx| {
                        let _ = sync_view.update(cx, |app, cx| app.open_sync_settings(cx));
                    });
                    let steam_view = view.downgrade();
                    cx.on_action(move |_: &SyncSteam, cx| {
                        let _ = steam_view.update(cx, |app, cx| app.sync_steam(cx));
                    });
                    let connect_view = view.downgrade();
                    cx.on_action(move |_: &ConnectSteam, cx| {
                        let _ = connect_view.update(cx, |app, cx| app.open_steam_settings(cx));
                    });
                    let refresh_view = view.downgrade();
                    cx.on_action(move |_: &RefreshLibrary, cx| {
                        let _ = refresh_view.update(cx, |app, cx| app.refresh_library(cx));
                    });
                    let quit_view = view.downgrade();
                    cx.on_action(move |_: &Quit, cx| {
                        if quit_view
                            .update(cx, |app, cx| app.can_close(cx))
                            .unwrap_or(true)
                        {
                            cx.quit();
                        }
                    });
                    let weak = view.downgrade();
                    window
                        .observe_window_appearance(move |window, cx| {
                            let _ = weak.update(cx, |app, cx| app.appearance_changed(window, cx));
                        })
                        .detach();
                    cx.new(|cx| Root::new(view, window, cx))
                },
            );
            if let Err(error) = result {
                log::error!("Could not open GameSync: {error}");
                cx.quit();
            }
        });
    Ok(())
}
