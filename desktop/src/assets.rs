//! Bundled covers and app icons, with gpui-component as the fallback source.
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
/// Embed the shared design artwork; virtual keys keep component icon overrides stable.
const ICONS: &[(&str, &[u8])] = &[
    (
        "icons/play-now/fast-forward.svg",
        include_bytes!("../../Design/resources/icons-new/fast-forward-stroke-rounded.svg"),
    ),
    (
        "icons/play-now/hourglass.svg",
        include_bytes!("../../Design/resources/icons-new/hourglass-stroke-rounded.svg"),
    ),
    (
        "icons/play-now/customize.svg",
        include_bytes!("../../Design/resources/icons-new/customize-stroke-rounded.svg"),
    ),
    (
        "icons/play-now/bookmark-02.svg",
        include_bytes!("../../Design/resources/icons-new/bookmark-02-stroke-rounded.svg"),
    ),
    (
        "icons/play-now/transaction-history.svg",
        include_bytes!("../../Design/resources/icons-new/transaction-history-stroke-rounded.svg"),
    ),
    (
        "icons/play-now/cards-02.svg",
        include_bytes!("../../Design/resources/icons-new/cards-02-stroke-rounded.svg"),
    ),
    (
        "icons/play-now/clock-fading.svg",
        include_bytes!("../../Design/resources/icons-new/clock-fading-stroke-rounded.svg"),
    ),
    (
        "icons/play-now/infinity-01.svg",
        include_bytes!("../../Design/resources/icons-new/infinity-01-stroke-rounded.svg"),
    ),
    (
        "icons/play-now/battery-low.svg",
        include_bytes!("../../Design/resources/icons-new/battery-low-stroke-rounded.svg"),
    ),
    (
        "icons/play-now/battery-medium-01.svg",
        include_bytes!("../../Design/resources/icons-new/battery-medium-01-stroke-rounded.svg"),
    ),
    (
        "icons/play-now/battery-full.svg",
        include_bytes!("../../Design/resources/icons-new/battery-full-stroke-rounded.svg"),
    ),
    (
        "icons/play-now/brain-02.svg",
        include_bytes!("../../Design/resources/icons-new/brain-02-stroke-rounded.svg"),
    ),
    (
        "icons/play-now/gun.svg",
        include_bytes!("../../Design/resources/icons-new/gun-stroke-rounded.svg"),
    ),
    (
        "icons/play-now/airplane-02.svg",
        include_bytes!("../../Design/resources/icons-new/airplane-02-stroke-rounded.svg"),
    ),
    (
        "icons/play-now/car-04.svg",
        include_bytes!("../../Design/resources/icons-new/car-04-stroke-rounded.svg"),
    ),
    (
        "icons/play-now/maps-search.svg",
        include_bytes!("../../Design/resources/icons-new/maps-search-stroke-rounded.svg"),
    ),
    (
        "icons/play-now/horse-saddle.svg",
        include_bytes!("../../Design/resources/icons-new/horse-saddle-stroke-rounded.svg"),
    ),
    (
        "icons/play-now/spades.svg",
        include_bytes!("../../Design/resources/icons-new/spades-stroke-rounded.svg"),
    ),
    (
        "icons/play-now/user-group-03.svg",
        include_bytes!("../../Design/resources/icons-new/user-group-03-stroke-rounded.svg"),
    ),
    (
        "icons/play-now/infinity-square.svg",
        include_bytes!("../../Design/resources/icons-new/infinity-square-stroke-rounded.svg"),
    ),
    (
        "icons/play-now/alien-02.svg",
        include_bytes!("../../Design/resources/icons-new/alien-02-stroke-rounded.svg"),
    ),
    (
        "icons/play-now/refresh-04.svg",
        include_bytes!("../../Design/resources/icons-new/refresh-04-stroke-rounded.svg"),
    ),
    (
        "icons/hugeicons/gameboy-stroke-rounded.svg",
        include_bytes!("../../Design/resources/icons-new/gameboy-stroke-rounded.svg"),
    ),
    (
        "icons/hugeicons/computer-stroke-rounded.svg",
        include_bytes!("../../Design/resources/icons-new/computer-stroke-rounded.svg"),
    ),
    (
        "icons/hugeicons/info-stroke-rounded.svg",
        include_bytes!("../../Design/resources/icons-new/info-stroke-rounded.svg"),
    ),
    (
        "icons/eye-off.svg",
        include_bytes!("../../Design/resources/icons-new/eye-off-stroke-rounded.svg"),
    ),
    (
        "icons/hugeicons/bookshelf-03-stroke-rounded.svg",
        include_bytes!("../../Design/resources/icons-new/bookshelf-03-stroke-rounded.svg"),
    ),
    (
        "icons/hugeicons/tags-stroke-rounded.svg",
        include_bytes!("../../Design/resources/icons-new/tags-stroke-rounded.svg"),
    ),
    (
        "icons/hugeicons/star-square-stroke-rounded.svg",
        include_bytes!("../../Design/resources/icons-new/star-square-stroke-rounded.svg"),
    ),
    (
        "icons/hugeicons/time-04-stroke-rounded.svg",
        include_bytes!("../../Design/resources/icons-new/time-04-stroke-rounded.svg"),
    ),
    (
        "icons/panel-left.svg",
        include_bytes!("../../Design/resources/icons-new/sidebar-left-stroke-rounded.svg"),
    ),
    (
        "icons/layout-dashboard.svg",
        include_bytes!("../../Design/resources/icons-new/dashboard-square-01-stroke-rounded.svg"),
    ),
    (
        "icons/gallery-vertical-end.svg",
        include_bytes!("../../Design/resources/icons-new/cards-02-stroke-rounded.svg"),
    ),
    (
        "icons/menu.svg",
        include_bytes!("../../Design/resources/icons-new/layout-list-stroke-rounded.svg"),
    ),
    (
        "icons/settings-2.svg",
        include_bytes!("../../Design/resources/icons-new/filter-horizontal-stroke-rounded.svg"),
    ),
    (
        "icons/search.svg",
        include_bytes!("../../Design/resources/icons-new/search-01-stroke-rounded.svg"),
    ),
    (
        "icons/info.svg",
        include_bytes!("../../Design/resources/icons-new/info-stroke-rounded.svg"),
    ),
    (
        "icons/panel-bottom.svg",
        include_bytes!("../../Design/resources/icons-new/layout-bottom-stroke-rounded.svg"),
    ),
    (
        "icons/ellipsis.svg",
        include_bytes!(
            "../../Design/resources/icons-new/more-horizontal-square-01-stroke-rounded.svg"
        ),
    ),
    (
        "icons/arrow-left.svg",
        include_bytes!("../../Design/resources/icons-new/arrow-left-02-stroke-rounded.svg"),
    ),
    (
        "icons/settings.svg",
        include_bytes!("../../Design/resources/icons-new/spaceship-stroke-rounded.svg"),
    ),
    (
        "icons/folder-open.svg",
        include_bytes!("../../Design/resources/icons-new/cloud-sync-stroke-rounded.svg"),
    ),
    (
        "icons/palette.svg",
        include_bytes!("../../Design/resources/icons-new/palette-stroke-rounded.svg"),
    ),
    (
        "icons/bot.svg",
        include_bytes!("../../Design/resources/icons-new/bot-message-square-stroke-rounded.svg"),
    ),
    (
        "icons/heart.svg",
        include_bytes!("../../Design/resources/icons-new/heart-stroke-rounded.svg"),
    ),
    (
        "icons/star.svg",
        include_bytes!("../../Design/resources/icons-new/star-stroke-rounded.svg"),
    ),
    (
        "icons/hugeicons/folder-03-stroke-rounded.svg",
        include_bytes!("../../Design/resources/icons-new/folder-03-stroke-rounded.svg"),
    ),
    (
        "icons/hugeicons/home-07-stroke-rounded-solid.svg",
        include_bytes!("../../Design/resources/icons-new/solid/home-07-stroke-rounded-solid.svg"),
    ),
    (
        "icons/hugeicons/game-controller-03-stroke-rounded-solid.svg",
        include_bytes!(
            "../../Design/resources/icons-new/solid/game-controller-03-stroke-rounded-solid.svg"
        ),
    ),
    (
        "icons/hugeicons/heart-stroke-rounded-solid.svg",
        include_bytes!("../../Design/resources/icons-new/solid/heart-stroke-rounded-solid.svg"),
    ),
    (
        "icons/hugeicons/star-stroke-rounded-solid.svg",
        include_bytes!("../../Design/resources/icons-new/solid/star-stroke-rounded-solid.svg"),
    ),
    (
        "icons/hugeicons/home-07-stroke-rounded.svg",
        include_bytes!("../../Design/resources/icons-new/home-07-stroke-rounded.svg"),
    ),
    (
        "icons/hugeicons/game-controller-03-stroke-rounded.svg",
        include_bytes!("../../Design/resources/icons-new/game-controller-03-stroke-rounded.svg"),
    ),
    (
        "icons/hugeicons/heart-stroke-rounded.svg",
        include_bytes!("../../Design/resources/icons-new/heart-stroke-rounded.svg"),
    ),
    (
        "icons/hugeicons/star-stroke-rounded.svg",
        include_bytes!("../../Design/resources/icons-new/star-stroke-rounded.svg"),
    ),
    (
        "icons/hugeicons/folder-01-stroke-rounded.svg",
        include_bytes!("../../Design/resources/icons-new/folder-01-stroke-rounded.svg"),
    ),
    (
        "icons/hugeicons/settings-04-stroke-rounded.svg",
        include_bytes!("../../Design/resources/icons-new/settings-04-stroke-rounded.svg"),
    ),
    (
        "icons/hugeicons/plus-stroke-rounded.svg",
        include_bytes!("../../Design/resources/icons-new/plus-stroke-rounded.svg"),
    ),
    // Override shared paths so buttons and menus use the same chevrons.
    (
        "icons/chevron-down.svg",
        include_bytes!("../../Design/resources/icons-new/chevron-down-stroke-rounded.svg"),
    ),
    (
        "icons/chevron-left.svg",
        include_bytes!("../../Design/resources/icons-new/chevron-left-stroke-rounded.svg"),
    ),
    (
        "icons/chevron-right.svg",
        include_bytes!("../../Design/resources/icons-new/chevron-right-stroke-rounded.svg"),
    ),
    (
        "icons/square-kanban.svg",
        include_bytes!("../../Design/resources/icons-new/kanban-stroke-rounded.svg"),
    ),
    (
        "icons/heart-filled.svg",
        include_bytes!("../../Design/resources/icons-new/solid/heart-stroke-rounded-solid.svg"),
    ),
    (
        "icons/badge-percent.svg",
        include_bytes!("../../Design/resources/icons-new/hot-price-stroke-rounded.svg"),
    ),
];

/// App icons that the bundled component icon set does not include.
pub struct BoardIcon;
impl gpui_component::IconNamed for BoardIcon {
    fn path(self) -> SharedString {
        "icons/square-kanban.svg".into()
    }
}

pub struct DiscountIcon;
impl gpui_component::IconNamed for DiscountIcon {
    fn path(self) -> SharedString {
        "icons/badge-percent.svg".into()
    }
}

/// A filled heart marks a favorite game.
pub struct FavoriteIcon;
impl gpui_component::IconNamed for FavoriteIcon {
    fn path(self) -> SharedString {
        "icons/heart-filled.svg".into()
    }
}

/// Setups in the Best on details: a handheld for Steam Deck, a monitor for
/// PC, and the (i) that opens how fit is calculated.
#[derive(Clone, Copy)]
pub enum SetupIcon {
    SteamDeck,
    Pc,
    Info,
}
impl gpui_component::IconNamed for SetupIcon {
    fn path(self) -> SharedString {
        match self {
            Self::SteamDeck => "icons/hugeicons/gameboy-stroke-rounded.svg",
            Self::Pc => "icons/hugeicons/computer-stroke-rounded.svg",
            Self::Info => "icons/hugeicons/info-stroke-rounded.svg",
        }
        .into()
    }
}

/// Sidebar icon roles share the same artwork as the Elyx workspace.
#[derive(Clone, Copy)]
pub enum SidebarIcon {
    BestOn,
    Genres,
    Tags,
    Rating,
    Playtime,

    HomeSelected,
    AllGamesSelected,
    FavoritesSelected,
    WishlistSelected,
    CollectionSelected,

    Home,
    AllGames,
    Favorites,
    Wishlist,
    Collection,
    Settings,
    Plus,
}
impl SidebarIcon {
    pub fn selected(self, selected: bool) -> Self {
        if !selected {
            return self;
        }
        match self {
            Self::Home => Self::HomeSelected,
            Self::AllGames => Self::AllGamesSelected,
            Self::Favorites => Self::FavoritesSelected,
            Self::Wishlist => Self::WishlistSelected,
            Self::Collection => Self::CollectionSelected,
            other => other,
        }
    }
}

impl gpui_component::IconNamed for SidebarIcon {
    fn path(self) -> SharedString {
        match self {
            Self::Genres => "icons/hugeicons/bookshelf-03-stroke-rounded.svg",
            Self::BestOn => "icons/hugeicons/computer-stroke-rounded.svg",
            Self::Tags => "icons/hugeicons/tags-stroke-rounded.svg",
            Self::Rating => "icons/hugeicons/star-square-stroke-rounded.svg",
            Self::Playtime => "icons/hugeicons/time-04-stroke-rounded.svg",

            Self::HomeSelected => "icons/hugeicons/home-07-stroke-rounded-solid.svg",
            Self::AllGamesSelected => "icons/hugeicons/game-controller-03-stroke-rounded-solid.svg",
            Self::FavoritesSelected => "icons/hugeicons/heart-stroke-rounded-solid.svg",
            Self::WishlistSelected => "icons/hugeicons/star-stroke-rounded-solid.svg",
            Self::CollectionSelected => "icons/hugeicons/folder-03-stroke-rounded.svg",

            Self::Home => "icons/hugeicons/home-07-stroke-rounded.svg",
            Self::AllGames => "icons/hugeicons/game-controller-03-stroke-rounded.svg",
            Self::Favorites => "icons/hugeicons/heart-stroke-rounded.svg",
            Self::Wishlist => "icons/hugeicons/star-stroke-rounded.svg",
            Self::Collection => "icons/hugeicons/folder-01-stroke-rounded.svg",
            Self::Settings => "icons/hugeicons/settings-04-stroke-rounded.svg",
            Self::Plus => "icons/hugeicons/plus-stroke-rounded.svg",
        }
        .into()
    }
}

/// Filled rating mark, shared with the editable star control.
pub struct RatingIcon;
impl gpui_component::IconNamed for RatingIcon {
    fn path(self) -> SharedString {
        "icons/hugeicons/star-stroke-rounded-solid.svg".into()
    }
}

/// GameSync draws Hugeicons line icons at one 1.75 px weight (2026-09-30).
/// The supplied SVGs mix 1.5 and 2 px strokes with filled outlines of 1.5 px
/// strokes, so the weight is set when an icon loads and the shared Design
/// sources stay byte-identical. Solid icons have no line and stay unchanged.
const LINE_WEIGHT: &str = "1.75";
/// A filled outline encodes a 1.5 px stroke. A centered stroke on its boundary
/// widens the band by half its width on each side: 1.5 + 0.25 = 1.75.
const OUTLINE_GROWTH: &str = "0.25";

fn line_weight(svg: &[u8]) -> Option<String> {
    // Line icons share this root signature; solid icons use another exporter.
    const LINE_ROOT: &str = r#"color="currentColor" fill="none""#;
    const STROKE_WIDTH: &str = r#"stroke-width=""#;
    let text = std::str::from_utf8(svg).ok()?;
    if !text.contains(LINE_ROOT) {
        return None;
    }
    if !text.contains(STROKE_WIDTH) {
        return Some(text.replacen(
            LINE_ROOT,
            &format!(
                r#"{LINE_ROOT} stroke="currentColor" stroke-width="{OUTLINE_GROWTH}" stroke-linejoin="round""#
            ),
            1,
        ));
    }
    let mut weighted = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(STROKE_WIDTH) {
        let value = at + STROKE_WIDTH.len();
        let end = value + rest[value..].find('"')?;
        weighted.push_str(&rest[..value]);
        weighted.push_str(LINE_WEIGHT);
        rest = &rest[end..];
    }
    weighted.push_str(rest);
    Some(weighted)
}

pub struct Assets;
impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some((_, bytes)) = ICONS.iter().find(|(name, _)| *name == path) {
            return Ok(Some(match line_weight(bytes) {
                Some(weighted) => Cow::Owned(weighted.into_bytes()),
                None => Cow::Borrowed(bytes),
            }));
        }
        if let Some((_, bytes)) = COVERS.iter().find(|(name, _)| *name == path) {
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
    fn hidden_and_discount_roles_load_supplied_artwork() {
        use gpui_component::IconNamed;

        for (path, expected) in [
            (
                gpui_component::IconName::EyeOff.path(),
                include_bytes!("../../Design/resources/icons-new/eye-off-stroke-rounded.svg")
                    .as_slice(),
            ),
            (
                DiscountIcon.path(),
                include_bytes!("../../Design/resources/icons-new/hot-price-stroke-rounded.svg")
                    .as_slice(),
            ),
        ] {
            let actual = Assets.load(path.as_ref()).unwrap().unwrap();
            assert_eq!(actual.as_ref(), line_weight(expected).unwrap().as_bytes());
        }
    }

    #[test]
    fn line_icons_load_at_one_weight() {
        let load = |path: &str| {
            String::from_utf8(Assets.load(path).unwrap().unwrap().into_owned()).unwrap()
        };
        // Supplied at 1.5 and 2 px strokes.
        for path in [
            "icons/hugeicons/home-07-stroke-rounded.svg",
            "icons/hugeicons/folder-01-stroke-rounded.svg",
            "icons/folder-open.svg",
        ] {
            let svg = load(path);
            assert!(svg.contains(r#"stroke-width="1.75""#), "{path}");
            assert!(!svg.contains(r#"stroke-width="1.5""#), "{path}");
            assert!(!svg.contains(r#"stroke-width="2""#), "{path}");
        }
        // Filled outlines of 1.5 px strokes: smart collections and the open folder.
        for path in [
            "icons/hugeicons/bookshelf-03-stroke-rounded.svg",
            "icons/hugeicons/tags-stroke-rounded.svg",
            "icons/hugeicons/star-square-stroke-rounded.svg",
            "icons/hugeicons/time-04-stroke-rounded.svg",
            "icons/hugeicons/folder-03-stroke-rounded.svg",
        ] {
            let svg = load(path);
            assert_eq!(svg.matches(r#"stroke-width="0.25""#).count(), 1, "{path}");
            assert!(svg.contains(r#"stroke="currentColor""#), "{path}");
        }
        // Solid icons have no line to weight.
        assert_eq!(
            load("icons/hugeicons/home-07-stroke-rounded-solid.svg").as_bytes(),
            include_bytes!(
                "../../Design/resources/icons-new/solid/home-07-stroke-rounded-solid.svg"
            )
        );
    }

    #[test]
    fn each_demo_game_has_a_cover() {
        for game in crate::fixtures::games().unwrap() {
            let bytes = Assets.load(&game.cover).unwrap().unwrap();
            assert!(bytes.starts_with(&[0xff, 0xd8]));
        }
    }
}

/// Approved recommendation icons from the canonical design source.
#[derive(Clone, Copy)]
pub struct PlayIcon(pub &'static str);
impl gpui_component::IconNamed for PlayIcon {
    fn path(self) -> SharedString {
        format!("icons/play-now/{}.svg", self.0).into()
    }
}
