//! Create a disposable bright-artwork fixture for native card-mask checks.
use anyhow::{Context, Result};
use gamesync_desktop::{library::LibraryStore, record_store::RecordStore, records::GameData};
use std::path::PathBuf;
fn main() -> Result<()> {
    let root = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .context("Provide a new test folder")?,
    );
    let library = LibraryStore::create(&root, "Card geometry checks")?;
    let store = RecordStore::open(library.root())?;
    for (index, title, width, height) in [
        (0, "A bright landscape with a deliberately long game title that spans several lines on a physical card", 1200, 600),
        (1, "Bright portrait", 600, 900),
        (2, "Missing artwork", 0, 0),
    ] {
        let mut game = GameData::new(title);
        if width > 0 {
            let cover = format!("media/geometry-{index}.jpg");
            let pixels = image::RgbImage::from_fn(width, height, |x, y| {
                image::Rgb([255, (200 + (x / 24) % 56) as u8, (160 + (y / 16) % 96) as u8])
            });
            pixels.save(root.join(&cover))?;
            game.personal.cover = Some(cover);
        }
        store.create(game)?;
    }
    println!("Created {}", root.display());
    Ok(())
}
