mod assets;
mod fixtures;
mod model;
mod theme;
mod ui;

use gpui::{
    px, size, App, AppContext as _, Application, Bounds, KeyBinding, Menu, MenuItem, WindowBounds,
    WindowOptions,
};
use gpui_component::Root;
use ui::app::GameSyncApp;

gpui::actions!(gamesync, [Quit, FocusSearch]);

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut games = fixtures::games()?;
    if args.iter().any(|arg| arg == "--empty") {
        games.clear();
    }
    if args.iter().any(|arg| arg == "--stress") {
        let original = games.clone();
        if !original.is_empty() {
            games = (0..10_000)
                .map(|index| {
                    let mut game = original[index % original.len()].clone();
                    game.id = 100_000_000 + index as u32;
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
    let library = model::Library::new(games);

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
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.bind_keys([
                KeyBinding::new("secondary-q", Quit, None),
                KeyBinding::new("secondary-f", FocusSearch, None),
            ]);
            cx.set_menus(vec![Menu {
                name: "GameSync".into(),
                items: vec![MenuItem::action("Quit GameSync", Quit)],
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
                        size(px(1280.), px(820.)),
                        cx,
                    ))),
                    window_min_size: Some(size(px(960.), px(600.))),
                    ..Default::default()
                },
                move |window, cx| {
                    theme::apply_choice(theme::SYSTEM_THEME, window, cx);
                    let view = cx.new(|cx| GameSyncApp::new(library, window, cx));
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
