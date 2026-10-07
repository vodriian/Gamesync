//! Settings → Best on: your PC, tag preferences, PC-only equipment, and
//! optional ProtonDB enrichment. Values live in `BestOnState`, which saves
//! them and recalculates the library.

use crate::{
    model::Library,
    ui::best_on_state::{BestOnGlobal, BestOnState, Enrichment},
};
use gamesync_desktop::{
    smart::{SmartKind, SmartRule},
    suitability::{BestOn, Equipment, Rules},
};
use gpui::{div, prelude::*, Entity, SharedString, Window};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex, ActiveTheme as _, IconName, Selectable as _, Sizable as _, StyledExt as _,
};

/// One-click rules. Nothing applies until the user adds it.
const PC_SUGGESTIONS: [&str; 3] = ["Open World", "FPS", "Story Rich"];
const DECK_SUGGESTIONS: [&str; 3] = ["Casual", "Card Game", "Roguelike Deckbuilder"];
const EQUIPMENT_SUGGESTIONS: [(&str, &[&str]); 2] = [
    ("Racing wheel", &["Racing", "Automobile Sim", "Driving"]),
    ("Flight sticks", &["Flight", "Space Sim"]),
];

#[derive(Clone, Copy, PartialEq)]
enum TagList {
    Pc,
    Deck,
}

pub struct BestOnSettings {
    library: Entity<Library>,
    state: Entity<BestOnState>,
    prefer_pc: Entity<InputState>,
    prefer_deck: Entity<InputState>,
    equipment_name: Entity<InputState>,
    equipment_tags: Entity<InputState>,
    adding_equipment: bool,
}

impl BestOnSettings {
    pub fn new(library: Entity<Library>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.global::<BestOnGlobal>().0.clone();
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        cx.observe(&library, |_, _, cx| cx.notify()).detach();
        let input = |placeholder: &'static str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
        };
        let prefer_pc = input("Type a tag, then press Return", window, cx);
        let prefer_deck = input("Type a tag, then press Return", window, cx);
        for (entity, list) in [(&prefer_pc, TagList::Pc), (&prefer_deck, TagList::Deck)] {
            cx.subscribe_in(
                entity,
                window,
                move |this, input, event, window, cx| match event {
                    // Return adds the exact or first match, else the typed text.
                    InputEvent::PressEnter { .. } => {
                        let typed = input.read(cx).value().trim().to_owned();
                        if typed.is_empty() {
                            return;
                        }
                        let matches = this.matches(list, &typed, cx);
                        let tag = matches
                            .iter()
                            .find(|tag| tag.eq_ignore_ascii_case(&typed))
                            .or(matches.first())
                            .cloned()
                            .unwrap_or(typed);
                        this.add_tag(list, &tag, cx);
                        input.update(cx, |input, cx| input.set_value("", window, cx));
                    }
                    InputEvent::Change => cx.notify(),
                    _ => {}
                },
            )
            .detach();
        }
        Self {
            library,
            state,
            prefer_pc,
            prefer_deck,
            equipment_name: input("Name, for example Moza R5 wheel", window, cx),
            equipment_tags: input("Tags, separated by commas", window, cx),
            adding_equipment: false,
        }
    }

    fn rules(&self, cx: &App) -> Rules {
        self.state.read(cx).rules.clone()
    }

    fn change(&self, cx: &mut Context<Self>, edit: impl FnOnce(&mut Rules)) {
        let mut rules = self.rules(cx);
        edit(&mut rules);
        self.state
            .update(cx, |state, cx| state.set_rules(rules, cx));
    }

    fn list(&self, list: TagList, cx: &App) -> Vec<String> {
        let rules = self.rules(cx);
        match list {
            TagList::Pc => rules.prefer_pc,
            TagList::Deck => rules.prefer_deck,
        }
    }

    /// Library tags, Steam and yours, that contain `query` and are not in
    /// the list yet. Most used first.
    fn matches(&self, list: TagList, query: &str, cx: &App) -> Vec<String> {
        let library = self.library.read(cx);
        let tags = library.games.iter().flat_map(|game| {
            let steam = game
                .record
                .as_ref()
                .and_then(|r| r.game.steam.as_ref()?.metadata.as_ref())
                .map_or(&[][..], |metadata| metadata.tags.as_slice());
            game.tags.iter().chain(steam)
        });
        crate::ui::editor::suggest_tags(tags, &self.list(list, cx), query).0
    }

    /// Where a suggested tag comes from, for its tooltip.
    fn source(&self, tag: &str, cx: &App) -> &'static str {
        let library = self.library.read(cx);
        let mine = library
            .games
            .iter()
            .any(|game| game.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)));
        let steam = library.games.iter().any(|game| {
            game.record
                .as_ref()
                .and_then(|r| r.game.steam.as_ref()?.metadata.as_ref())
                .is_some_and(|m| m.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)))
        });
        match (mine, steam) {
            (true, true) => "Your tag and a Steam tag",
            (true, false) => "Your tag",
            _ => "Steam tag",
        }
    }

    fn add_tag(&self, list: TagList, tag: &str, cx: &mut Context<Self>) {
        let tag = tag.trim().to_owned();
        self.change(cx, |rules| {
            let tags = match list {
                TagList::Pc => &mut rules.prefer_pc,
                TagList::Deck => &mut rules.prefer_deck,
            };
            if !tags.iter().any(|known| known.eq_ignore_ascii_case(&tag)) {
                tags.push(tag);
            }
        });
    }

    fn save_equipment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.equipment_name.read(cx).value().trim().to_owned();
        let tags: Vec<String> = self
            .equipment_tags
            .read(cx)
            .value()
            .split(',')
            .map(|tag| tag.trim().to_owned())
            .filter(|tag| !tag.is_empty())
            .collect();
        if name.is_empty() || tags.is_empty() {
            return;
        }
        self.change(cx, |rules| rules.equipment.push(Equipment { name, tags }));
        self.adding_equipment = false;
        for input in [&self.equipment_name, &self.equipment_tags] {
            input.update(cx, |input, cx| input.set_value("", window, cx));
        }
        cx.notify();
    }
}

use gpui::App;

fn group(cx: &App) -> gpui::Div {
    v_flex()
        .p_5()
        .gap_3()
        .rounded(cx.theme().radius_lg)
        .bg(crate::ui::controls::group_surface(cx))
}

fn title(text: &'static str) -> impl IntoElement {
    div().text_base().font_medium().child(text)
}

fn hint(text: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    div()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
}

impl Render for BestOnSettings {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let rules = state.rules.clone();
        let message = state.message.clone();
        v_flex()
            .gap_4()
            .child(hint(self.summary(&rules, cx), cx))
            .when(!message.is_empty(), |column| {
                column.child(div().text_sm().text_color(cx.theme().danger).child(message))
            })
            .child(self.pc_group(&rules, cx))
            .child(self.tag_group(TagList::Pc, &rules, cx))
            .child(self.tag_group(TagList::Deck, &rules, cx))
            .child(self.equipment_group(&rules, cx))
            .child(self.enrichment_group(cx))
    }
}

impl BestOnSettings {
    /// How the current rules sort the library, from the sidebar counts.
    fn summary(&self, rules: &Rules, cx: &App) -> String {
        let library = self.library.read(cx);
        let count = |band: BestOn| {
            library
                .smart
                .iter()
                .filter(|group| group.kind == SmartKind::BestOn)
                .flat_map(|group| &group.values)
                .find(|(rule, _)| *rule == SmartRule::BestOn(band))
                .map_or(0, |(_, count)| *count)
        };
        let counts = format!(
            "{} games on PC, {} on Steam Deck, {} on both, {} need review.",
            count(BestOn::Pc),
            count(BestOn::SteamDeck),
            count(BestOn::Both),
            count(BestOn::NeedsReview)
        );
        if *rules == Rules::default() && !self.state.read(cx).protondb {
            format!("No rules yet. Best on uses Steam data and where you played: {counts}")
        } else {
            format!("With these rules: {counts}")
        }
    }

    fn pc_group(&self, rules: &Rules, cx: &mut Context<Self>) -> impl IntoElement {
        let high_end = rules.high_end_pc;
        group(cx)
            .child(title("My PC"))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("pc-standard")
                            .outline()
                            .label("Standard")
                            .selected(!high_end)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.change(cx, |rules| rules.high_end_pc = false)
                            })),
                    )
                    .child(
                        Button::new("pc-high-end")
                            .outline()
                            .label("High-end")
                            .selected(high_end)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.change(cx, |rules| rules.high_end_pc = true)
                            })),
                    ),
            )
            .child(hint(
                "High-end means 4K, HDR, high refresh rate, and room for mods. PC fit starts at 85 instead of 75.",
                cx,
            ))
    }

    fn tag_group(&self, list: TagList, rules: &Rules, cx: &mut Context<Self>) -> impl IntoElement {
        let (heading, explain, tags, input, suggestions, id) = match list {
            TagList::Pc => (
                "Prefer PC for",
                "Games with any of these tags get +15 on PC and −15 on Steam Deck. Steam tags and your tags both count.",
                rules.prefer_pc.clone(),
                self.prefer_pc.clone(),
                PC_SUGGESTIONS,
                "pc",
            ),
            TagList::Deck => (
                "Prefer Steam Deck for",
                "Games with any of these tags get +15 on Steam Deck and −15 on PC. A game in both lists keeps its Steam fit.",
                rules.prefer_deck.clone(),
                self.prefer_deck.clone(),
                DECK_SUGGESTIONS,
                "deck",
            ),
        };
        // Typing lists matching library tags; an empty field shows starters.
        let query = input.read(cx).value().trim().to_owned();
        let open: Vec<(String, Option<&'static str>)> = if query.is_empty() {
            suggestions
                .into_iter()
                .filter(|tag| !tags.iter().any(|known| known.eq_ignore_ascii_case(tag)))
                .map(|tag| (tag.to_owned(), None))
                .collect()
        } else {
            self.matches(list, &query, cx)
                .into_iter()
                .map(|tag| {
                    let source = self.source(&tag, cx);
                    (tag, Some(source))
                })
                .collect()
        };
        let unknown = !query.is_empty()
            && open.is_empty()
            && !tags.iter().any(|known| known.eq_ignore_ascii_case(&query));
        group(cx)
            .child(title(heading))
            .child(hint(explain, cx))
            .when(!tags.is_empty(), |group| {
                group.child(
                    h_flex()
                        .flex_wrap()
                        .gap_2()
                        .children(tags.iter().enumerate().map(|(index, tag)| {
                            let tag = tag.clone();
                            Button::new(SharedString::from(format!("{id}-tag-{index}")))
                                .xsmall()
                                .outline()
                                .label(tag.clone())
                                .icon(IconName::Close)
                                .tooltip(format!("Remove {tag}"))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let tag = tag.clone();
                                    this.change(cx, |rules| match list {
                                        TagList::Pc => {
                                            rules.prefer_pc.retain(|known| *known != tag)
                                        }
                                        TagList::Deck => {
                                            rules.prefer_deck.retain(|known| *known != tag)
                                        }
                                    })
                                }))
                        })),
                )
            })
            .child(Input::new(&input).small())
            .when(!open.is_empty(), |group| {
                group.child(h_flex().flex_wrap().gap_1().children(open.into_iter().map(
                    |(tag, source)| {
                        let input = input.clone();
                        Button::new(SharedString::from(format!("{id}-suggest-{tag}")))
                            .xsmall()
                            .outline()
                            .icon(IconName::Plus)
                            .label(tag.clone())
                            .when_some(source, |button, source| button.tooltip(source))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.add_tag(list, &tag, cx);
                                input.update(cx, |input, cx| input.set_value("", window, cx));
                            }))
                    },
                )))
            })
            .when(unknown, |group| {
                group.child(hint(
                    "No game in your library has this tag. Press Return to add it anyway.",
                    cx,
                ))
            })
            .when(list == TagList::Pc, |group| {
                group.child(
                    Checkbox::new("prefer-pc-mods")
                        .label("Games with mod support (Steam Workshop)")
                        .checked(rules.prefer_pc_mods)
                        .on_click(cx.listener(|this, checked: &bool, _, cx| {
                            let value = *checked;
                            this.change(cx, |rules| rules.prefer_pc_mods = value)
                        })),
                )
            })
    }

    fn equipment_group(&self, rules: &Rules, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let rows = rules.equipment.iter().enumerate().map(|(index, item)| {
            h_flex()
                .gap_3()
                .px_3()
                .py_2()
                .rounded(theme.radius)
                .border_1()
                .border_color(theme.border)
                .bg(theme.background)
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_1()
                        .child(div().text_sm().font_medium().child(item.name.clone()))
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(item.tags.join(" · ")),
                        ),
                )
                .child(
                    Button::new(("remove-equipment", index))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Close)
                        .tooltip(format!("Remove {}", item.name))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.change(cx, |rules| {
                                if index < rules.equipment.len() {
                                    rules.equipment.remove(index);
                                }
                            })
                        })),
                )
                .into_any_element()
        });
        let rows: Vec<_> = rows.collect();
        let open: Vec<_> = EQUIPMENT_SUGGESTIONS
            .into_iter()
            .filter(|(name, _)| !rules.equipment.iter().any(|item| item.name == *name))
            .collect();
        group(cx)
            .child(title("Equipment · PC only"))
            .child(hint(
                "Games with these Steam tags need equipment that connects to your PC. They show PC only, whatever Steam says.",
                cx,
            ))
            .children(rows)
            .when(rules.equipment.is_empty() && !open.is_empty(), |group| {
                group.child(h_flex().flex_wrap().gap_1().children(open.into_iter().map(
                    |(name, tags)| {
                        Button::new(SharedString::from(format!("equipment-{name}")))
                            .xsmall()
                            .outline()
                            .icon(IconName::Plus)
                            .label(name)
                            .tooltip(tags.join(", "))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.change(cx, |rules| {
                                    rules.equipment.push(Equipment {
                                        name: name.into(),
                                        tags: tags.iter().map(|tag| (*tag).into()).collect(),
                                    })
                                })
                            }))
                    },
                )))
            })
            .map(|group| {
                if self.adding_equipment {
                    group.child(
                        v_flex()
                            .gap_2()
                            .child(Input::new(&self.equipment_name).small())
                            .child(Input::new(&self.equipment_tags).small())
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(Button::new("equipment-cancel").label("Cancel").on_click(
                                        cx.listener(|this, _, _, cx| {
                                            this.adding_equipment = false;
                                            cx.notify();
                                        }),
                                    ))
                                    .child(
                                        Button::new("equipment-save")
                                            .primary()
                                            .label("Add")
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.save_equipment(window, cx)
                                            })),
                                    ),
                            ),
                    )
                } else {
                    group.child(
                        h_flex().child(
                            Button::new("equipment-add")
                                .outline()
                                .icon(IconName::Plus)
                                .label("Add equipment")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.adding_equipment = true;
                                    cx.notify();
                                })),
                        ),
                    )
                }
            })
    }

    fn enrichment_group(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let enabled = state.protondb;
        let enrichment = state.enrichment.clone();
        let data = state.data.clone();
        let library = self.library.read(cx);
        let covered = data.as_ref().map_or(0, |data| {
            library
                .games
                .iter()
                .filter_map(|game| game.record.as_ref()?.game.steam.as_ref())
                .filter(|steam| data.apps.contains_key(&steam.app_id))
                .count()
        });
        let theme = cx.theme();
        let mb = |bytes: u64| bytes.div_ceil(1024 * 1024);
        let (status, failed) = match (&enrichment, &data) {
            _ if !enabled => (
                "Off. Turning it on downloads about 70 MB, then checks for new data once a month."
                    .to_owned(),
                false,
            ),
            (Enrichment::Downloading(read, Some(total)), _) => (
                format!("Downloading ProtonDB data · {} of {} MB", mb(*read), mb(*total)),
                false,
            ),
            (Enrichment::Downloading(read, None), _) => {
                (format!("Downloading ProtonDB data · {} MB", mb(*read)), false)
            }
            (Enrichment::Failed(error), Some(data)) => (
                format!(
                    "Could not update ProtonDB data: {error}. The {} data still applies.",
                    data.export_label()
                ),
                true,
            ),
            (Enrichment::Failed(error), None) => {
                (format!("Could not download ProtonDB data: {error}"), true)
            }
            (Enrichment::Idle, Some(data)) => (
                format!(
                    "Updated {} · {covered} of your games {} enough Deck reports. Checks for new data each month.",
                    data.export_label(),
                    if covered == 1 { "has" } else { "have" }
                ),
                false,
            ),
            (Enrichment::Idle, None) => ("No ProtonDB data yet.".to_owned(), false),
        };
        let downloading = matches!(enrichment, Enrichment::Downloading(..));
        group(cx)
            .child(title("Enrichment"))
            .child(hint(
                "Add community reports to Steam Deck fit. They can lower fit for heavy games and rate games Valve has not checked.",
                cx,
            ))
            .child(
                Checkbox::new("protondb")
                    .label("ProtonDB Steam Deck reports")
                    .checked(enabled)
                    .on_click(cx.listener(|this, checked: &bool, _, cx| {
                        let value = *checked;
                        this.state.update(cx, |state, cx| state.set_protondb(value, cx));
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(if failed { theme.danger } else { theme.muted_foreground })
                    .child(status),
            )
            .when(enabled, |group| {
                group
                    .child(h_flex().child(if downloading {
                        Button::new("protondb-cancel")
                            .outline()
                            .label("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.state.update(cx, |state, cx| state.cancel(cx))
                            }))
                    } else {
                        Button::new("protondb-update")
                            .outline()
                            .label(if failed { "Try again" } else { "Update now" })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.state.update(cx, |state, cx| state.update_now(cx))
                            }))
                    }))
                    .child(hint(gamesync_desktop::protondb::CREDIT, cx))
            })
    }
}
