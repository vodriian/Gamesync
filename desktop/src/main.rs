mod assets;
mod fixtures;
mod managed_storage;
mod model;
use gamesync_desktop::settings;
mod theme;
mod ui;
mod watcher;

use gpui::{
    px, size, App, AppContext as _, Application, Bounds, KeyBinding, Menu, MenuItem, WindowBounds,
    WindowOptions,
};
use gpui_component::Root;
use ui::app::GameSyncApp;

gpui::actions!(
    gamesync,
    [
        Quit,
        FocusSearch,
        RefreshLibrary,
        SaveDetails,
        ManageCollections,
        OpenSettings
    ]
);

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .filter_module("reqwest", log::LevelFilter::Off)
        .filter_module("hyper", log::LevelFilter::Off)
        .filter_module("hyper_util", log::LevelFilter::Off)
        .init();
    let args: Vec<String> = std::env::args().skip(1).collect();
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
    let sample = args.iter().any(|arg| arg == "--demo");
    let initial_path = if preview {
        None
    } else {
        Some(managed_storage::path(sample)?)
    };
    // Demo data is opt-in. A normal first launch must not look like a real library.
    let demo_requested = args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--demo" | "--stress" | "--missing-covers"));
    let mut games = if demo_requested {
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
    if !demo_requested {
        library.name = "My games".into();
        library.demo = false;
    }
    let initial_theme = settings::load()
        .map(|s| s.theme)
        .unwrap_or_else(|_| theme::SYSTEM_THEME.into());

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
                    titlebar: Some(gpui::TitlebarOptions {
                        title: Some("GameSync — Demo library".into()),
                        ..Default::default()
                    }),
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
                    theme::apply_choice(&initial_theme, window, cx);
                    let view = cx.new(|cx| {
                        GameSyncApp::new(library, initial_path, initial_theme, window, cx)
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
