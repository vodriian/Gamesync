//! Eagle's sidebar row geometry, with game statuses in place of asset folders.

use crate::model::{Library, Scope};
use gpui::{div, prelude::*, px, Entity, Window};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputState},
    menu::{ContextMenuExt, PopupMenuItem},
    v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
};
mod collections;

pub struct LibrarySidebar {
    library: Entity<Library>,
    status_open: bool,
    collections_open: bool,
    status_motion: super::motion::Motion,
    collections_motion: super::motion::Motion,
    focus: gpui::FocusHandle,
    restore_focus: bool,
    edit: Option<collections::NameEdit>,
    busy: bool,
    message: String,
    toast: Option<String>,
    toast_task: Option<gpui::Task<()>>,
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
            focus: cx.focus_handle(),
            restore_focus: false,
            edit: None,
            busy: false,
            message: String::new(),
            toast: None,
            toast_task: None,
        }
    }

    pub(super) fn show_toast(&mut self, message: &str, cx: &mut Context<Self>) {
        self.message.clear();
        self.toast = Some(message.into());
        // Replacing the task gives each new message its full display time.
        self.toast_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(3))
                .await;
            let _ = this.update(cx, |this, cx| {
                this.toast = None;
                cx.notify();
            });
        }));
        cx.notify();
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
            .when(
                matches!(scope, Scope::Collection(_)) && !self.busy && self.edit.is_none(),
                |row| {
                    row.drag_over::<super::grid::DraggedGame>(|style, _, _, cx| {
                        style
                            .bg(cx.theme().primary)
                            .text_color(cx.theme().primary_foreground)
                    })
                },
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.library.update(cx, |lib, cx| {
                    lib.set_scope(scope.clone());
                    cx.notify();
                });
            }))
            .child(Icon::new(icon).size_4())
            .child(div().flex_1().min_w_0().truncate().child(label))
            .child(div().text_xs().child(count.to_string()))
    }
}

impl Render for LibrarySidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.restore_focus {
            self.restore_focus = false;
            window.focus(&self.focus);
        }
        v_flex()
            .relative()
            .track_focus(&self.focus)
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
                    .child(self.row(Scope::Favorites, IconName::Star, cx))
                    .when(self.library.read(cx).show_hidden_games, |column| {
                        column.child(self.row(Scope::Hidden, IconName::EyeOff, cx))
                    }),
            )
            .child(
                v_flex()
                    .id("sidebar-sections")
                    .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                        if event.keystroke.key == "escape" && this.edit.is_some() && !this.busy {
                            this.edit = None;
                            this.restore_focus = true;
                            this.message.clear();
                            cx.stop_propagation();
                            cx.notify();
                        }
                    }))
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
                                    .disabled(self.edit.is_some())
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
                                    .tooltip("New collection")
                                    .disabled(self.busy || self.edit.is_some())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        if this.library.read(cx).write_issue.is_some()
                                            || this.library.read(cx).source.is_none()
                                        {
                                            window.dispatch_action(
                                                Box::new(crate::ManageCollections),
                                                cx,
                                            );
                                        } else {
                                            this.begin_name(None, window, cx);
                                        }
                                    })),
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
                                    * self.collections_motion.value()
                                    + if self.edit.as_ref().is_some_and(|edit| edit.id.is_some()) {
                                        36.
                                    } else {
                                        0.
                                    },
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
                                        if self
                                            .edit
                                            .as_ref()
                                            .is_some_and(|edit| edit.id == Some(c.id))
                                        {
                                            self.name_editor(cx)
                                        } else {
                                            self.collection_row(c.id, cx)
                                        }
                                    }),
                            ),
                    )
                    .when(
                        self.edit.as_ref().is_some_and(|edit| edit.id.is_none()),
                        |view| view.child(self.name_editor(cx)),
                    )
                    .when(!self.message.is_empty(), |view| {
                        view.child(div().px_2().text_xs().child(self.message.clone()))
                    }),
            )
            .when_some(self.toast.as_ref(), |sidebar, message| {
                sidebar.child(
                    h_flex()
                        .absolute()
                        .bottom_3()
                        .left_3()
                        .right_3()
                        .px_3()
                        .py_2()
                        .gap_2()
                        .rounded_lg()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().popover)
                        .text_color(cx.theme().popover_foreground)
                        .shadow_sm()
                        .text_xs()
                        .child(Icon::new(IconName::CircleCheck).size_3())
                        .child(message.clone()),
                )
            })
    }
}
