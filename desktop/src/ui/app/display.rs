//! One display menu and one query model for all library presentations.
use super::*;
use crate::settings::{GroupBy, SortBy};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};

fn item(
    app: &Entity<GameSyncApp>,
    label: impl Into<gpui::SharedString>,
    checked: bool,
    persist: bool,
    change: impl Fn(&mut Library) + 'static,
) -> PopupMenuItem {
    let app = app.clone();
    PopupMenuItem::new(label)
        .checked(checked)
        .on_click(move |_, _, cx| {
            app.update(cx, |app, cx| {
                app.library.update(cx, |lib, cx| {
                    change(lib);
                    lib.recompute();
                    cx.notify();
                });
                if persist {
                    let display = app.library.read(cx).display.clone();
                    // Serialize rapid choices so the newest choice is the final disk write.
                    let previous = app.display_save.take();
                    app.display_pending += 1;
                    app.display_save = Some(cx.spawn(async move |app, cx| {
                        if let Some(previous) = previous {
                            previous.await;
                        }
                        let result = cx
                            .background_spawn(async move {
                                crate::settings::update(|s| s.library_display = display)
                            })
                            .await;
                        let _ = app.update(cx, |app, cx| {
                            app.display_pending = app.display_pending.saturating_sub(1);
                            if let Err(error) = result {
                                app.notice = format!("Could not save library display: {error}");
                            }
                            cx.notify();
                        });
                    }));
                }
                cx.notify();
            });
        })
}

impl GameSyncApp {
    pub(super) fn display_control(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let app = cx.entity();
        let lib = self.library.read(cx);
        let active = lib.filter_status.is_some()
            || lib.filter_collection.is_some()
            || lib.filter_favorites
            || lib.display != crate::settings::LibraryDisplay::default();
        Button::new("library-display")
            .ghost()
            .small()
            .icon(IconName::Settings2)
            .rounded_full()
            .w(px(36.))
            .h(px(38.))
            .selected(active)
            .tooltip("Filter, sort and group")
            .dropdown_menu(move |menu, window, cx| {
                let lib = app.read(cx).library.read(cx);
                let display = lib.display.clone();
                let status = lib.filter_status.clone();
                let collection = lib.filter_collection;
                let favorites = lib.filter_favorites;
                let statuses = lib.statuses.clone();
                let collections = lib
                    .source
                    .as_ref()
                    .map(|(_, m)| m.definitions.collections.clone())
                    .unwrap_or_default();
                let sort_app = app.clone();
                let group_app = app.clone();
                let filter_app = app.clone();
                menu.submenu("Filter", window, cx, move |menu, window, cx| {
                    let status = status.clone();
                    let statuses = statuses.clone();
                    let collections = collections.clone();
                    let status_app = filter_app.clone();
                    let collection_app = filter_app.clone();
                    menu.submenu("Status", window, cx, move |mut menu, _, _| {
                        menu = menu.item(item(
                            &status_app,
                            "All statuses",
                            status.is_none(),
                            false,
                            |l| l.filter_status = None,
                        ));
                        for s in &statuses {
                            let key = s.key.clone();
                            menu = menu.item(item(
                                &status_app,
                                s.label.clone(),
                                status.as_ref() == Some(&s.key),
                                false,
                                move |l| l.filter_status = Some(key.clone()),
                            ));
                        }
                        menu
                    })
                    .submenu("Collection", window, cx, move |mut menu, _, _| {
                        menu = menu.item(item(
                            &collection_app,
                            "All collections",
                            collection.is_none(),
                            false,
                            |l| l.filter_collection = None,
                        ));
                        for c in collections.iter().filter(|c| !c.archived) {
                            let id = c.id;
                            menu = menu.item(item(
                                &collection_app,
                                c.name.clone(),
                                collection == Some(id),
                                false,
                                move |l| l.filter_collection = Some(id),
                            ));
                        }
                        menu
                    })
                    .item(item(
                        &filter_app,
                        "Favorites only",
                        favorites,
                        false,
                        move |l| l.filter_favorites = !favorites,
                    ))
                    .separator()
                    .item(item(
                        &filter_app,
                        "Clear filters",
                        false,
                        false,
                        |l| {
                            l.filter_status = None;
                            l.filter_collection = None;
                            l.filter_favorites = false;
                        },
                    ))
                })
                .submenu("Sort by", window, cx, move |mut menu, _, _| {
                    for (label, sort) in [
                        ("Name", SortBy::Name),
                        ("Status", SortBy::Status),
                        ("Hours", SortBy::Hours),
                        ("Collection", SortBy::Collection),
                    ] {
                        menu = menu.item(item(
                            &sort_app,
                            label,
                            display.sort == sort,
                            true,
                            move |l| l.display.sort = sort,
                        ));
                    }
                    menu.separator()
                        .item(item(
                            &sort_app,
                            "Ascending",
                            !display.descending,
                            true,
                            |l| l.display.descending = false,
                        ))
                        .item(item(
                            &sort_app,
                            "Descending",
                            display.descending,
                            true,
                            |l| l.display.descending = true,
                        ))
                })
                .submenu("Group by", window, cx, move |mut menu, _, _| {
                    for (label, group) in [
                        ("None", GroupBy::None),
                        ("Status", GroupBy::Status),
                        ("Collections", GroupBy::Collections),
                    ] {
                        menu = menu.item(item(
                            &group_app,
                            label,
                            display.group == group,
                            true,
                            move |l| l.display.group = group,
                        ));
                    }
                    menu
                })
            })
    }
}
