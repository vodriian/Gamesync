//! Eagle's sidebar row geometry, with game statuses in place of asset folders.

use crate::model::{Library, Scope};
use gpui::{div, prelude::*, px, Entity, Window};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex, v_flex, ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _,
};

pub struct LibrarySidebar {
    library: Entity<Library>,
    status_open: bool,
    collections_open: bool,
    status_motion: super::motion::Motion,
    collections_motion: super::motion::Motion,
}

impl LibrarySidebar {
    pub fn new(library: Entity<Library>, cx: &mut Context<Self>) -> Self {
        cx.observe(&library, |_, _, cx| cx.notify()).detach();
        Self {
            library,
            status_open: true,
            collections_open: true,
            status_motion: super::motion::Motion::new(1.),
            collections_motion: super::motion::Motion::new(1.),
        }
    }

    fn row(&self, scope: Scope, icon: IconName, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.library.read(cx).scope == scope;
        let count = self.library.read(cx).count(&scope);
        let label = self.library.read(cx).scope_label(&scope);
        h_flex()
            .id(gpui::SharedString::from(format!("scope-{scope:?}")))
            .h_8()
            .flex_shrink_0()
            .px_2()
            .py_1()
            .gap_x_2()
            .rounded(cx.theme().radius)
            .text_sm()
            .cursor_pointer()
            .when(active, |row| {
                row.font_medium()
                    .bg(cx.theme().sidebar_accent)
                    .text_color(cx.theme().sidebar_accent_foreground)
            })
            .when(!active, |row| {
                row.hover(|style| style.bg(cx.theme().sidebar_accent.opacity(0.18)))
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.library.update(cx, |lib, cx| {
                    lib.set_scope(scope.clone());
                    cx.notify();
                });
            }))
            .child(Icon::new(icon).size_4())
            .child(div().flex_1().child(label))
            .child(div().text_xs().child(count.to_string()))
    }
}

impl Render for LibrarySidebar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w(px(255.))
            .h_full()
            .flex_shrink_0()
            .bg(cx.theme().sidebar)
            .text_color(cx.theme().sidebar_foreground)
            .child(
                v_flex()
                    .p_4()
                    .gap_1()
                    .child(
                        h_flex()
                            .justify_between()
                            .child(div().font_semibold().child("GameSync"))
                            .child(
                                Button::new("settings")
                                    .text_color(cx.theme().sidebar_foreground)
                                    .ghost()
                                    .small()
                                    .icon(IconName::Settings)
                                    .tooltip("Settings")
                                    .on_click(|_, window, cx| {
                                        window.dispatch_action(Box::new(crate::OpenSettings), cx)
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().sidebar_foreground.opacity(0.65))
                            .child("A little room for your games"),
                    ),
            )
            .child(
                v_flex()
                    .px_2()
                    .gap_1()
                    .child(self.row(Scope::All, IconName::LayoutDashboard, cx))
                    .child(self.row(Scope::Favorites, IconName::Star, cx)),
            )
            .child(
                v_flex()
                    .id("sidebar-sections")
                    .mt_6()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_2()
                    .gap_1()
                    .child(
                        h_flex().child(
                            Button::new("status-section")
                                .text_color(cx.theme().sidebar_foreground)
                                .ghost()
                                .small()
                                .label("Status")
                                .icon(if self.status_open {
                                    IconName::ChevronDown
                                } else {
                                    IconName::ChevronRight
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.status_open = !this.status_open;
                                    this.status_motion
                                        .set(if this.status_open { 1. } else { 0. }, cx);
                                })),
                        ),
                    )
                    .child(
                        v_flex()
                            .h(px(self.library.read(cx).statuses.len() as f32
                                * 36.
                                * self.status_motion.value()))
                            .overflow_hidden()
                            .gap_1()
                            .flex_shrink_0()
                            .children(self.library.read(cx).statuses.clone().into_iter().map(
                                |status| {
                                    let icon = match status.key.as_str() {
                                        "completed" => IconName::CircleCheck,
                                        "dropped" => IconName::CircleX,
                                        _ => IconName::Folder,
                                    };
                                    self.row(Scope::Status(status.key), icon, cx)
                                },
                            )),
                    )
                    .child(
                        h_flex()
                            .pt_4()
                            .gap_1()
                            .child(
                                Button::new("collections-section")
                                    .text_color(cx.theme().sidebar_foreground)
                                    .ghost()
                                    .small()
                                    .label("Collections")
                                    .icon(if self.collections_open {
                                        IconName::ChevronDown
                                    } else {
                                        IconName::ChevronRight
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.collections_open = !this.collections_open;
                                        this.collections_motion
                                            .set(if this.collections_open { 1. } else { 0. }, cx);
                                    })),
                            )
                            .child(div().flex_1())
                            .child(
                                Button::new("add-collection")
                                    .text_color(cx.theme().sidebar_foreground)
                                    .ghost()
                                    .small()
                                    .icon(IconName::Plus)
                                    .tooltip("Add or manage collections")
                                    .on_click(|_, window, cx| {
                                        window
                                            .dispatch_action(Box::new(crate::ManageCollections), cx)
                                    }),
                            ),
                    )
                    .child(
                        v_flex()
                            .h(px(
                                self.library.read(cx).source.as_ref().map_or(0, |(_, m)| {
                                    m.definitions
                                        .collections
                                        .iter()
                                        .filter(|c| !c.archived)
                                        .count()
                                }) as f32
                                    * 36.
                                    * self.collections_motion.value(),
                            ))
                            .overflow_hidden()
                            .gap_1()
                            .flex_shrink_0()
                            .children(
                                self.library
                                    .read(cx)
                                    .source
                                    .clone()
                                    .into_iter()
                                    .flat_map(|(_, m)| m.definitions.collections)
                                    .filter(|c| !c.archived)
                                    .map(|c| {
                                        self.row(Scope::Collection(c.id), IconName::Folder, cx)
                                    }),
                            ),
                    ),
            )
    }
}
