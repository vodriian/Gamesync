//! Sidebar smart collections. Values come from `Library::smart`; nothing here writes
//! library data. Only the expanded groups are saved, in device settings.
use super::*;
use gamesync_desktop::smart::{RatingBand, SmartKind};
use std::collections::BTreeSet;

/// Text groups (genres and tags) show this many values before **Show all**.
const SHORT_LIST: usize = 12;

pub(super) struct SmartState {
    pub open: bool,
    tags_open: bool,
    groups_open: BTreeSet<SmartKind>,
    show_all: BTreeSet<SmartKind>,
    save: Option<gpui::Task<()>>,
}

impl SmartState {
    pub fn new() -> Self {
        let groups_open: BTreeSet<_> = crate::settings::load()
            .map(|settings| settings.smart_groups_open.into_iter().collect())
            .unwrap_or_default();
        Self {
            open: true,
            tags_open: false,
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
        for group in &groups {
            if group.kind == SmartKind::SteamTag {
                section = section.child(self.smart_heading(
                    None,
                    "Tags",
                    Some(SidebarIcon::Tags),
                    self.smart.tags_open,
                    false,
                    cx,
                ));
                if self.smart.tags_open {
                    for tags in groups
                        .iter()
                        .filter(|g| matches!(g.kind, SmartKind::SteamTag | SmartKind::MyTag))
                    {
                        section = section.child(self.smart_group(tags, true, cx));
                    }
                }
            } else if group.kind != SmartKind::MyTag && !group.values.is_empty() {
                section = section.child(self.smart_group(group, false, cx));
            }
        }
        section.into_any_element()
    }

    /// Parent Tags only controls visibility; provider and personal scopes stay distinct.
    fn smart_heading(
        &self,
        kind: Option<SmartKind>,
        label: &'static str,
        icon: Option<SidebarIcon>,
        open: bool,
        nested: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        self.row_base(format!("smart-heading-{label}"), false, cx)
            .pl(px(if nested { 30. } else { 10. }))
            .gap(px(10.))
            .child(
                Icon::new(if open {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                })
                .size(px(12.))
                .text_color(cx.theme().sidebar_foreground.opacity(0.6)),
            )
            .when_some(icon, |row, icon| {
                row.child(
                    Icon::new(icon)
                        .size(px(18.))
                        .text_color(cx.theme().sidebar_foreground.opacity(0.7)),
                )
            })
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| {
                if let Some(kind) = kind {
                    this.toggle_group(kind, cx);
                } else {
                    this.smart.tags_open = !this.smart.tags_open;
                    cx.notify();
                }
            }))
    }

    fn smart_group(
        &self,
        group: &crate::model::SmartGroup,
        nested: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let kind = group.kind;
        let open = self.smart.groups_open.contains(&kind);
        let icon = match kind {
            SmartKind::BestOn => Some(SidebarIcon::BestOn),
            SmartKind::Genre => Some(SidebarIcon::Genres),
            SmartKind::Rating => Some(SidebarIcon::Rating),
            SmartKind::Playtime => Some(SidebarIcon::Playtime),
            SmartKind::SteamTag | SmartKind::MyTag => None,
        };
        let mut section = v_flex().gap(px(2.)).child(self.smart_heading(
            Some(kind),
            kind.label(),
            icon,
            open,
            nested,
            cx,
        ));
        if !open {
            return section.into_any_element();
        }
        let banded = matches!(
            kind,
            SmartKind::BestOn | SmartKind::Rating | SmartKind::Playtime
        );
        let all = banded || self.smart.show_all.contains(&kind);
        let total = group.values.len();
        let shown = if all { total } else { total.min(SHORT_LIST) };
        for (rule, count) in group.values.iter().take(shown) {
            section = section.child(
                self.scope_row(Scope::Smart(rule.clone()), None, *count, cx)
                    .pl(px(if nested { 52. } else { 40. }))
                    .text_color(cx.theme().sidebar_foreground.opacity(0.9)),
            );
        }
        if total == 0 {
            section = section.child(
                div()
                    .pl(px(52.))
                    .py_1()
                    .text_xs()
                    .text_color(cx.theme().sidebar_foreground.opacity(0.6))
                    .child("No tags yet"),
            );
        }
        if !banded && total > SHORT_LIST {
            let expanded = self.smart.show_all.contains(&kind);
            section = section.child(
                Button::new(gpui::SharedString::from(format!("smart-more-{kind:?}")))
                    .text_color(cx.theme().sidebar_foreground.opacity(0.75))
                    .ghost()
                    .xsmall()
                    .ml(px(if nested { 52. } else { 40. }))
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
        section.into_any_element()
    }

    /// Saves are chained so the newest state is the final disk write.
    fn toggle_group(&mut self, kind: SmartKind, cx: &mut Context<Self>) {
        if !self.smart.groups_open.remove(&kind) {
            self.smart.groups_open.insert(kind);
        }
        // The new prototype enum must not make older app settings unreadable.
        if kind == SmartKind::BestOn {
            cx.notify();
            return;
        }
        let open: Vec<_> = self
            .smart
            .groups_open
            .iter()
            .copied()
            .filter(|kind| *kind != SmartKind::BestOn)
            .collect();
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

/// Retain the existing 1–2 bucket while making its range visible without text stars.
pub(super) fn rating_label(band: RatingBand) -> gpui::AnyElement {
    let stars = |filled: usize, size: f32| {
        h_flex().gap(px(1.)).children((0..5).map(move |index| {
            if index < filled {
                Icon::new(crate::assets::RatingIcon)
            } else {
                Icon::new(IconName::Star)
            }
            .size(px(size))
        }))
    };
    match band {
        RatingBand::Five => stars(5, 16.).into_any_element(),
        RatingBand::Four => stars(4, 16.).into_any_element(),
        RatingBand::Three => stars(3, 16.).into_any_element(),
        RatingBand::OneToTwo => h_flex()
            .gap(px(4.))
            .child(stars(1, 12.))
            .child("–")
            .child(stars(2, 12.))
            .into_any_element(),
        RatingBand::Unrated => h_flex()
            .gap(px(6.))
            .child(stars(0, 12.))
            .child(div().text_xs().child("Unrated"))
            .into_any_element(),
    }
}
