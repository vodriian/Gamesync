//! Create a disposable library from bundled fixtures for native folder-loading QA.
use anyhow::{Context, Result};
use gamesync_desktop::{
    library::LibraryStore,
    record_store::RecordStore,
    records::{GameData, SteamData},
};
use serde::Deserialize;
use std::{fs, path::PathBuf};

#[derive(Deserialize)]
struct Demo {
    id: u32,
    title: String,
    description: String,
    status: String,
    rating: Option<u8>,
    tags: Vec<String>,
    favorite: bool,
    playtime_minutes: u32,
}

fn main() -> Result<()> {
    let path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .context("Usage: demo_library <new-folder>")?,
    );
    let library = LibraryStore::create(&path, "Evening games")?;
    let store = RecordStore::open(library.root())?;
    let demos: Vec<Demo> = serde_json::from_str(include_str!("../fixtures/games.json"))?;
    for demo in demos {
        let mut game = GameData::new(demo.title);
        game.steam = Some(SteamData {
            app_id: demo.id,
            description: Some(demo.description),
            playtime_minutes: demo.playtime_minutes,
            owned: true,
            extra: Default::default(),
        });
        game.personal.status = demo.status.to_lowercase();
        game.personal.rating = demo.rating;
        game.personal.tags = demo.tags;
        game.personal.favorite = demo.favorite;
        let cover = format!("media/{}.jpg", demo.id);
        fs::copy(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(format!("fixtures/covers/{}.jpg", demo.id)),
            path.join(&cover),
        )?;
        game.personal.cover = Some(cover);
        store.create(game)?;
    }
    println!("Created {}", path.display());
    Ok(())
}
