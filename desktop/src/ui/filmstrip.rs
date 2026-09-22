//! Adapted from Eagle's viewer.rs render_strip and sync_strip (MIT, d1fc38f).
//! Keep its horizontal virtualization, shared image cache and instant hover feedback.
use super::thumb_cache::LruImageCache;
use crate::model::Library;
use gpui::{
    div, image_cache, img, prelude::*, px, size, Entity, ObjectFit, Pixels, ScrollStrategy, Size,
    Window,
};
use gpui_component::{
    h_flex, h_virtual_list, scroll::ScrollableElement as _, ActiveTheme as _,
    VirtualListScrollHandle,
};
use std::rc::Rc;

const CELL: Pixels = px(80.);
const GAP: Pixels = px(6.);
pub struct SelectGame(pub usize);
pub struct Filmstrip {
    library: Entity<Library>,
    cache: Entity<LruImageCache>,
    sizes: Rc<Vec<Size<Pixels>>>,
    scroll: VirtualListScrollHandle,
    selected: Option<uuid::Uuid>,
}
impl gpui::EventEmitter<SelectGame> for Filmstrip {}
impl Filmstrip {
    pub fn new(
        library: Entity<Library>,
        cache: Entity<LruImageCache>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&library, |_, _, cx| cx.notify()).detach();
        Self {
            library,
            cache,
            sizes: Rc::new(Vec::new()),
            scroll: VirtualListScrollHandle::new(),
            selected: None,
        }
    }
}
impl Render for Filmstrip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let library = self.library.read(cx);
        let visible = library.visible.clone();
        if self.sizes.len() != visible.len() {
            self.sizes = Rc::new(vec![size(CELL + GAP, CELL); visible.len()]);
        }
        if self.selected != library.selected {
            self.selected = library.selected;
            if let Some(slot) = visible
                .iter()
                .position(|&index| Some(library.games[index].id) == self.selected)
            {
                self.scroll.scroll_to_item(slot, ScrollStrategy::Center);
            }
        }
        let selected = self.selected;
        h_flex()
            .id("filmstrip")
            .relative()
            .h(px(96.))
            .w_full()
            .px_2()
            .flex_shrink_0()
            .border_t_1()
            .border_color(cx.theme().border.opacity(0.4))
            .bg(cx.theme().muted)
            .child(
                image_cache(self.cache.clone()).size_full().child(
                    h_flex().id("strip-inner").size_full().child(
                        h_virtual_list(
                            cx.entity(),
                            "strip",
                            self.sizes.clone(),
                            move |this, range, _, cx| {
                                let games: Vec<_> = {
                                    let library = this.library.read(cx);
                                    range
                                        .filter_map(|slot| {
                                            visible
                                                .get(slot)
                                                .map(|&index| (slot, library.games[index].clone()))
                                        })
                                        .collect()
                                };
                                games
                                    .into_iter()
                                    .map(|(slot, game)| {
                                        let active = selected == Some(game.id);
                                        div()
                                            .id(slot)
                                            .relative()
                                            .mr(GAP)
                                            .w(CELL)
                                            .h(CELL)
                                            .flex_shrink_0()
                                            .overflow_hidden()
                                            .rounded(px(4.))
                                            .cursor_pointer()
                                            .child(
                                                img(game
                                                    .cover_path
                                                    .map(gpui::ImageSource::from)
                                                    .unwrap_or_else(|| game.cover.into()))
                                                .size_full()
                                                .rounded(px(4.))
                                                .when(!active, |image| image.opacity(0.55))
                                                .object_fit(ObjectFit::Cover)
                                                .with_fallback(|| {
                                                    div().size_full().child("✦").into_any_element()
                                                }),
                                            )
                                            .child(
                                                div()
                                                    .id("thumbnail-highlight")
                                                    .absolute()
                                                    .inset_0()
                                                    .rounded(px(4.))
                                                    .border_4()
                                                    .border_color(if active {
                                                        cx.theme().ring
                                                    } else {
                                                        cx.theme().transparent
                                                    })
                                                    .when(!active, |outline| {
                                                        outline.hover(|style| {
                                                            style.border_color(
                                                                cx.theme().ring.opacity(0.5),
                                                            )
                                                        })
                                                    }),
                                            )
                                            .on_click(cx.listener(move |_, _, _, cx| {
                                                cx.emit(SelectGame(slot))
                                            }))
                                            .into_any_element()
                                    })
                                    .collect()
                            },
                        )
                        .track_scroll(&self.scroll),
                    ),
                ),
            )
            .horizontal_scrollbar(&self.scroll)
    }
}
