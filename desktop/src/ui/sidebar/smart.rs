//! Sidebar smart collections. Values come from `Library::smart`; nothing here writes
//! library data. Only the expanded groups are saved, in device settings.
use super::*;
use gamesync_desktop::smart::SmartKind;
use std::collections::BTreeSet;

/// Text groups (genres and tags) show this many values before **Show all**.
const SHORT_LIST: usize = 12;

pub(super) struct SmartState {
    pub open: bool,
    groups_open: BTreeSet<SmartKind>,
    show_all: BTreeSet<SmartKind>,
    save: Option<gpui::Task<()>>,
}

impl SmartState {
    pub fn new() -> Self {
        let groups_open = crate::settings::load()
            .map(|settings| settings.smart_groups_open.into_iter().collect())
            .unwrap_or_default();
        Self {
            open: true,
            groups_open,
            show_all: BTreeSet::new(),
            save: None,
        }
    }
}

impl LibrarySidebar {
    pub(super) fn smart_section(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let groups = self.library.read(cx).smart.clone();
        let header = Button::new("smart-section")
            .text_color(cx.theme().sidebar_foreground)
            .ghost()
            .small()
            .label("Smart collections")
            .icon(if self.smart.open {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            })
            .on_click(cx.listener(|this, _, _, cx| {
                this.smart.open = !this.smart.open;
                cx.notify();
            }));
        let mut section = v_flex()
            .mt_4()
            .gap_1()
            .flex_shrink_0()
            .child(h_flex().child(header));
        if !self.smart.open {
            return section.into_any_element();
        }
        if groups.iter().all(|group| group.values.is_empty()) {
            return section
                .child(
                    div()
                        .px_2()
                        .text_xs()
                        .text_color(cx.theme().sidebar_foreground.opacity(0.65))
                        .child("Sync Steam or add tags and ratings to fill this section."),
                )
                .into_any_element();
        }
        for group in groups.into_iter().filter(|group| !group.values.is_empty()) {
            let kind = group.kind;
            let open = self.smart.groups_open.contains(&kind);
            section = section.child(
                Button::new(gpui::SharedString::from(format!("smart-group-{kind:?}")))
                    .text_color(cx.theme().sidebar_foreground)
                    .ghost()
                    .small()
                    .justify_start()
                    .ml_2()
                    .icon(if open {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .label(kind.label())
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_group(kind, cx))),
            );
            if !open {
                continue;
            }
            let banded = matches!(kind, SmartKind::Rating | SmartKind::Playtime);
            let all = banded || self.smart.show_all.contains(&kind);
            let total = group.values.len();
            let shown = if all { total } else { total.min(SHORT_LIST) };
            for (rule, count) in group.values.into_iter().take(shown) {
                section = section.child(
                    self.scope_row(Scope::Smart(rule), None, count, cx)
                        .pl_10()
                        .text_color(cx.theme().sidebar_foreground.opacity(0.9)),
                );
            }
            if !banded && total > SHORT_LIST {
                let expanded = self.smart.show_all.contains(&kind);
                section = section.child(
                    Button::new(gpui::SharedString::from(format!("smart-more-{kind:?}")))
                        .text_color(cx.theme().sidebar_foreground.opacity(0.75))
                        .ghost()
                        .xsmall()
                        .ml_8()
                        .justify_start()
                        .label(if expanded {
                            "Show fewer".to_string()
                        } else {
                            format!("Show all {total}")
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if !this.smart.show_all.remove(&kind) {
                                this.smart.show_all.insert(kind);
                            }
                            cx.notify();
                        })),
                );
            }
        }
        section.into_any_element()
    }

    /// Saves are chained so the newest state is the final disk write.
    fn toggle_group(&mut self, kind: SmartKind, cx: &mut Context<Self>) {
        if !self.smart.groups_open.remove(&kind) {
            self.smart.groups_open.insert(kind);
        }
        let open: Vec<_> = self.smart.groups_open.iter().copied().collect();
        let previous = self.smart.save.take();
        self.smart.save = Some(cx.spawn(async move |this, cx| {
            if let Some(previous) = previous {
                previous.await;
            }
            let result = cx
                .background_spawn(
                    async move { crate::settings::update(|s| s.smart_groups_open = open) },
                )
                .await;
            if let Err(error) = result {
                let _ = this.update(cx, |this, cx| {
                    this.message = format!("Could not save sidebar layout: {error}");
                    cx.notify();
                });
            }
        }));
        cx.notify();
    }
}
