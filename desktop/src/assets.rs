//! Cover fixtures and the same component icon source used by Eagle.
use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;
const COVERS: &[(&str, &[u8])] = &[
    (
        "covers/1055540.jpg",
        include_bytes!("../fixtures/covers/1055540.jpg"),
    ),
    (
        "covers/1091500.jpg",
        include_bytes!("../fixtures/covers/1091500.jpg"),
    ),
    (
        "covers/1145360.jpg",
        include_bytes!("../fixtures/covers/1145360.jpg"),
    ),
    (
        "covers/1245620.jpg",
        include_bytes!("../fixtures/covers/1245620.jpg"),
    ),
    (
        "covers/2379780.jpg",
        include_bytes!("../fixtures/covers/2379780.jpg"),
    ),
    (
        "covers/367520.jpg",
        include_bytes!("../fixtures/covers/367520.jpg"),
    ),
    (
        "covers/413150.jpg",
        include_bytes!("../fixtures/covers/413150.jpg"),
    ),
    (
        "covers/620.jpg",
        include_bytes!("../fixtures/covers/620.jpg"),
    ),
    (
        "covers/632360.jpg",
        include_bytes!("../fixtures/covers/632360.jpg"),
    ),
    (
        "covers/646570.jpg",
        include_bytes!("../fixtures/covers/646570.jpg"),
    ),
    (
        "covers/753640.jpg",
        include_bytes!("../fixtures/covers/753640.jpg"),
    ),
    (
        "covers/990080.jpg",
        include_bytes!("../fixtures/covers/990080.jpg"),
    ),
];
/// Lucide `square-kanban` (ISC), the icon set gpui-component already bundles.
const ICONS: &[(&str, &[u8])] = &[
    (
        "icons/square-kanban.svg",
        include_bytes!("../icons/square-kanban.svg"),
    ),
    ("icons/house.svg", include_bytes!("../icons/house.svg")),
    (
        "icons/badge-percent.svg",
        include_bytes!("../icons/badge-percent.svg"),
    ),
];

/// App icons that the bundled component icon set does not include.
pub struct BoardIcon;
impl gpui_component::IconNamed for BoardIcon {
    fn path(self) -> SharedString {
        "icons/square-kanban.svg".into()
    }
}

pub struct HomeIcon;
impl gpui_component::IconNamed for HomeIcon {
    fn path(self) -> SharedString {
        "icons/house.svg".into()
    }
}

pub struct DiscountIcon;
impl gpui_component::IconNamed for DiscountIcon {
    fn path(self) -> SharedString {
        "icons/badge-percent.svg".into()
    }
}

pub struct Assets;
impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some((_, bytes)) = COVERS.iter().chain(ICONS).find(|(name, _)| *name == path) {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        gpui_component_assets::Assets.load(path)
    }
    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut names = gpui_component_assets::Assets.list(path)?;
        names.extend(
            COVERS
                .iter()
                .filter(|(name, _)| name.starts_with(path))
                .map(|(name, _)| SharedString::from(*name)),
        );
        Ok(names)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn each_demo_game_has_a_cover() {
        for game in crate::fixtures::games().unwrap() {
            let bytes = Assets.load(&game.cover).unwrap().unwrap();
            assert!(bytes.starts_with(&[0xff, 0xd8]));
        }
    }
}
