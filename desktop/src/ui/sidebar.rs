//! Eagle's sidebar row geometry, with game statuses in place of asset folders.

use crate::model::{Library, Scope};
use gpui::{div, prelude::*, px, Entity, Window};
use gpui_component::{h_flex, v_flex, ActiveTheme as _, Icon, IconName, StyledExt as _};

pub struct LibrarySidebar {
    library: Entity<Library>,
}

impl LibrarySidebar {
    pub fn new(library: Entity<Library>, cx: &mut Context<Self>) -> Self {
        cx.observe(&library, |_, _, cx| cx.notify()).detach();
        Self { library }
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
            .border_r_1()
            .border_color(cx.theme().border)
            .child(
                v_flex()
                    .p_4()
                    .gap_1()
                    .child(div().font_semibold().child("GameSync"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
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
                div()
                    .px_4()
                    .pt_5()
                    .pb_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Status"),
            )
            .child(
                v_flex()
                    .id("status-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_2()
                    .gap_1()
                    .children(
                        self.library
                            .read(cx)
                            .statuses
                            .clone()
                            .into_iter()
                            .map(|status| {
                                let icon = match status.key.as_str() {
                                    "completed" => IconName::CircleCheck,
                                    "dropped" => IconName::CircleX,
                                    _ => IconName::Folder,
                                };
                                self.row(Scope::Status(status.key), icon, cx)
                            }),
                    ),
            )
            .child(
                v_flex()
                    .p_4()
                    .gap_1()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.library.read(cx).name.clone())
                    .child(if self.library.read(cx).demo {
                        "Sample games. No account connected."
                    } else {
                        "Local library"
                    }),
            )
    }
}
