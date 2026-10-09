//! Library navigation follows the shared Elyx Sidebar. Statuses stay on the board;
//! the sidebar keeps scopes, collections, and computed smart collections.

use crate::assets::SidebarIcon;
use crate::model::{Library, Scope};
use gpui::{div, percentage, prelude::*, px, Animation, AnimationExt as _, Entity, Window};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputState},
    menu::{ContextMenuExt, PopupMenuItem},
    v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _,
};
mod collections;
mod smart;

pub struct LibrarySidebar {
    library: Entity<Library>,
    sync: Entity<crate::sync_runtime::SyncState>,
    steam: Entity<super::steam_job::SteamJob>,
    play_now: Entity<super::play_now::PlayNowView>,
    collections_open: bool,
    collections_motion: super::motion::Motion,
    focus: gpui::FocusHandle,
    restore_focus: bool,
    edit: Option<collections::NameEdit>,
    smart: smart::SmartState,
    busy: bool,
    message: String,
    toast: Option<String>,
    toast_icon: IconName,
    toast_task: Option<gpui::Task<()>>,
}

impl LibrarySidebar {
    pub fn new(
        library: Entity<Library>,
        sync: Entity<crate::sync_runtime::SyncState>,
        play_now: Entity<super::play_now::PlayNowView>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&library, |_, _, cx| cx.notify()).detach();
        cx.observe(&sync, |_, _, cx| cx.notify()).detach();
        let steam = cx.global::<super::steam_job::SteamGlobal>().0.clone();
        cx.observe(&steam, |_, _, cx| cx.notify()).detach();
        cx.observe(&play_now, |_, _, cx| cx.notify()).detach();
        Self {
            library,
            sync,
            steam,
            play_now,
            collections_open: true,
            collections_motion: super::motion::Motion::new(1.),
            focus: cx.focus_handle(),
            restore_focus: false,
            edit: None,
            smart: smart::SmartState::new(),
            busy: false,
            message: String::new(),
            toast: None,
            toast_icon: IconName::CircleCheck,
            toast_task: None,
        }
    }

    pub(super) fn show_toast(&mut self, message: &str, cx: &mut Context<Self>) {
        self.message.clear();
        self.toast = Some(message.into());
        self.toast_icon = IconName::CircleCheck;
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

    pub(super) fn show_sync_result(&mut self, message: &str, cx: &mut Context<Self>) {
        self.show_toast(message, cx);
        // A sync can finish with cancellation or partial failures.
        self.toast_icon = IconName::Info;
    }

    fn row(&self, scope: Scope, icon: SidebarIcon, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.library.read(cx).count(&scope);
        let selected = !self.library.read(cx).home
            && !self.library.read(cx).play_now
            && self.library.read(cx).scope == scope;
        self.scope_row(scope, Some(Icon::new(icon.selected(selected))), count, cx)
    }

    fn steam_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let job = self.steam.read(cx);
        let running = job.running();
        let library = self.library.read(cx);
        let unavailable = library.demo || library.source.is_none();
        let connected = library
            .source
            .as_ref()
            .is_some_and(|(_, manifest)| manifest.definitions.steam_account.is_some());
        let tooltip = if library.demo {
            "Steam sync is unavailable in the sample library".to_owned()
        } else if library.source.is_none() {
            "Your games are still loading…".to_owned()
        } else if running {
            format!("Syncing Steam… {}", job.message)
        } else if !connected {
            "Connect Steam in Settings".to_owned()
        } else {
            "Sync Steam".to_owned()
        };
        let icon = Icon::new(SidebarIcon::Sync).size(px(18.));
        // Only the artwork rotates. The button and its hit area stay still.
        let icon = if running && !super::motion::reduced(cx) {
            icon.with_animation(
                "steam-sync-rotation",
                Animation::new(std::time::Duration::from_secs(1)).repeat(),
                |icon, progress| icon.rotate(percentage(progress)),
            )
            .into_any_element()
        } else {
            icon.into_any_element()
        };
        Button::new("sidebar-sync-steam")
            .ghost()
            .small()
            .text_color(if running {
                cx.theme().primary
            } else {
                cx.theme().sidebar_foreground
            })
            .child(icon)
            .tooltip(tooltip)
            .disabled(running || unavailable)
            .on_click(|_, window, cx| window.dispatch_action(Box::new(crate::SyncSteam), cx))
    }

    /// Callers pass `count` so smart rows can use cached counts.
    fn scope_row(
        &self,
        scope: Scope,
        icon: Option<Icon>,
        count: usize,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let active = !self.library.read(cx).home
            && !self.library.read(cx).play_now
            && self.library.read(cx).scope == scope;
        let label = match &scope {
            Scope::Smart(gamesync_desktop::smart::SmartRule::Rating(band)) => {
                smart::rating_label(*band)
            }
            _ => div()
                .truncate()
                .child(self.library.read(cx).scope_label(&scope))
                .into_any_element(),
        };
        self.row_base(format!("scope-{scope:?}"), active, cx)
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
            .when_some(icon, |row, icon| row.child(icon.size(px(18.))))
            .child(div().flex_1().min_w_0().truncate().child(label))
            .child(div().text_xs().child(count.to_string()))
    }

    fn home_row(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.library.read(cx).home;
        self.row_base("home".into(), active, cx)
            .on_click(cx.listener(|this, _, _, cx| {
                this.library.update(cx, |lib, cx| {
                    lib.show_home();
                    cx.notify();
                });
            }))
            .child(Icon::new(SidebarIcon::Home.selected(active)).size(px(18.)))
            .child(div().flex_1().child("Home"))
    }

    fn play_now_row(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.library.read(cx).play_now;
        self.row_base("play-now".into(), active, cx)
            .on_click(cx.listener(|this, _, _, cx| {
                this.library.update(cx, |lib, cx| {
                    lib.show_play_now();
                    cx.notify();
                });
            }))
            .child(Icon::new(crate::assets::PlayIcon("cards-02")).size(px(18.)))
            .child(div().flex_1().child("Play now"))
            .when_some(self.play_now.read(cx).timer_label(), |row, timer| {
                row.child(
                    h_flex()
                        .gap_1()
                        .child(Icon::new(crate::assets::PlayIcon("hourglass")).size(px(14.)))
                        .child(div().text_xs().font_family("monospace").child(timer)),
                )
            })
    }

    fn row_base(
        &self,
        id: String,
        active: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        h_flex()
            .id(gpui::SharedString::from(id))
            .h_8()
            .flex_shrink_0()
            .px(px(10.))
            .py_1()
            .gap_x(px(10.))
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
    }
}

impl Render for LibrarySidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let review = self.sync.read(cx).conflicts.len();
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
                    .px_2()
                    .pt_4()
                    .gap(px(2.))
                    .child(self.home_row(cx))
                    .child(self.play_now_row(cx))
                    .child(self.row(Scope::All, SidebarIcon::AllGames, cx))
                    .child(self.row(Scope::Favorites, SidebarIcon::Favorites, cx))
                    .child(self.row(Scope::Wishlist, SidebarIcon::Wishlist, cx))
                    .when(self.library.read(cx).show_hidden_games, |column| {
                        column.child(self.scope_row(
                            Scope::Hidden,
                            Some(Icon::new(IconName::EyeOff)),
                            self.library.read(cx).count(&Scope::Hidden),
                            cx,
                        ))
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
                        h_flex()
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
                                    .icon(SidebarIcon::Plus)
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
                    .child(self.smart_section(cx))
                    .when(!self.message.is_empty(), |view| {
                        view.child(div().px_2().text_xs().child(self.message.clone()))
                    }),
            )
            .child(
                h_flex()
                    .flex_shrink_0()
                    .px_4()
                    .py_3()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .text_color(cx.theme().sidebar_foreground.opacity(0.6))
                            .child("GameSync"),
                    )
                    .child(div().flex_1())
                    // Conflicting edits from another device wait for a choice.
                    .when(review > 0, |footer| {
                        footer.child(
                            Button::new("sync-review")
                                .ghost()
                                .small()
                                .text_color(cx.theme().warning)
                                .label(format!("{review} to review"))
                                .tooltip("Changes from another device need a choice")
                                .on_click(|_, window, cx| {
                                    window.dispatch_action(Box::new(crate::OpenSyncSettings), cx)
                                }),
                        )
                    })
                    .child(self.steam_button(cx))
                    .child(
                        Button::new("settings")
                            .text_color(cx.theme().sidebar_foreground)
                            .ghost()
                            .small()
                            .child(Icon::new(SidebarIcon::Settings).size(px(18.)))
                            .tooltip("Settings")
                            .on_click(|_, window, cx| {
                                window.dispatch_action(Box::new(crate::OpenSettings), cx)
                            }),
                    ),
            )
            .when_some(self.toast.as_ref(), |sidebar, message| {
                sidebar.child(
                    h_flex()
                        .absolute()
                        .bottom(px(64.))
                        .left_3()
                        .right_3()
                        .px_3()
                        .py_2()
                        .gap_2()
                        .rounded(cx.theme().radius_lg)
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().popover)
                        .text_color(cx.theme().popover_foreground)
                        .shadow_sm()
                        .text_xs()
                        .child(Icon::new(self.toast_icon.clone()).size_3())
                        .child(message.clone()),
                )
            })
    }
}
