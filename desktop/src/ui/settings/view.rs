//! Fixed feedback above Glaze-style groups keeps errors and progress in view.
use super::*;
use gpui::{div, px};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex,
    input::Input,
    v_flex, ActiveTheme as _, Disableable as _, Selectable as _, StyledExt as _,
};
impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.clear_key {
            self.clear_key = false;
            self.key = Self::masked_input("Saved securely · enter a replacement", window, cx);
        }
        let source = self.library.read(cx).source.clone();
        let id = source.as_ref().map(|(_, m)| m.library_id);
        if id != self.library_id && !self.busy {
            self.library_id = id;
            self.tested = None;
            self.key_saved = false;
            self.last_sync = None;
            self.message.clear();
            self.key = Self::masked_input("Enter a key to save or replace", window, cx);
            self.profile.update(cx, |input, cx| {
                input.set_value(
                    source
                        .as_ref()
                        .and_then(|(_, m)| m.definitions.steam_account.clone())
                        .unwrap_or_default(),
                    window,
                    cx,
                )
            });
            if let Some(id) = id {
                self.read_connection(id, cx);
            }
        }
        let has_library = source.is_some() && !self.library.read(cx).demo;
        let surface = cx.theme().secondary;
        let group = || v_flex().p_5().gap_4().rounded_lg().bg(surface);
        let appearance = group().child(self.appearance_controls(cx)).child(
            Checkbox::new("reduce-motion")
                .label("Reduce motion")
                .checked(self.reduce_motion)
                .on_click(cx.listener(|this, checked: &bool, _, cx| {
                    this.reduce_motion = *checked;
                    cx.global_mut::<super::super::motion::MotionPreferences>()
                        .reduced = *checked;
                    let value = *checked;
                    cx.spawn(async move |this, cx| {
                        let result = cx
                            .background_spawn(async move {
                                settings::update(|s| s.reduce_motion = value)
                            })
                            .await;
                        if let Err(error) = result {
                            let _ = this.update(cx, |this, cx| {
                                this.message = format!("Could not save motion preference: {error}");
                                cx.notify();
                            });
                        }
                    })
                    .detach();
                    cx.notify();
                })),
        );
        let connection = group()
            .child("Steam profile or ID")
            .child(Input::new(&self.profile).disabled(self.busy || !has_library))
            .child(if self.key_saved {
                "Steam API key saved"
            } else {
                "Steam API key"
            })
            .child(Input::new(&self.key).disabled(self.busy || !has_library))
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        Button::new("test-steam")
                            .label("Test key")
                            .disabled(self.busy || !has_library)
                            .on_click(cx.listener(|this, _, _, cx| this.test_key(cx))),
                    )
                    .child(
                        Button::new("save-steam")
                            .primary()
                            .label("Save key")
                            .disabled(self.busy || !self.tested_matches(cx))
                            .on_click(cx.listener(|this, _, _, cx| this.save_key(cx))),
                    )
                    .child(div().flex_1())
                    .when(self.key_saved, |row| {
                        row.child(
                            Button::new("remove-steam")
                                .label("Remove key")
                                .disabled(self.busy || !has_library)
                                .on_click(cx.listener(|this, _, _, cx| this.remove_key(cx))),
                        )
                    }),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        "Saved keys stay in this device’s secure storage, separate from game data.",
                    ),
            );
        let sync = group()
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("sync-steam")
                            .primary()
                            .label("Sync now")
                            .disabled(self.busy || !has_library || !self.key_saved)
                            .on_click(cx.listener(|this, _, _, cx| this.sync(cx))),
                    )
                    .child(
                        Button::new("cancel-sync")
                            .label("Cancel sync")
                            .disabled(self.cancel.is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(cancel) = &this.cancel {
                                    cancel.store(true, Ordering::Relaxed);
                                    this.message = "Cancelling after the current request…".into();
                                    cx.notify();
                                }
                            })),
                    ),
            )
            .child(settings::sync_label(self.last_sync));
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(gpui_component::TitleBar::new().border_b_0())
            .child(
                v_flex()
                    .p_6()
                    .gap_2()
                    .flex_shrink_0()
                    .child(div().text_xl().font_semibold().child("Settings"))
                    .when(!self.message.is_empty(), |header| {
                        header.child(
                            div()
                                .id("settings-feedback")
                                .max_h(px(96.))
                                .overflow_y_scroll()
                                .text_sm()
                                .child(self.message.clone()),
                        )
                    }),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(
                        v_flex()
                            .w(px(170.))
                            .h_full()
                            .p_2()
                            .gap_1()
                            .flex_shrink_0()
                            .border_r_1()
                            .border_color(cx.theme().border)
                            .children(
                                [Section::General, Section::Appearance, Section::Ai]
                                    .into_iter()
                                    .map(|section| {
                                        Button::new(section.label())
                                            .ghost()
                                            .label(section.label())
                                            .icon(match section {
                                                Section::General => {
                                                    gpui_component::IconName::Settings
                                                }
                                                Section::Appearance => {
                                                    gpui_component::IconName::Palette
                                                }
                                                Section::Ai => gpui_component::IconName::Bot,
                                            })
                                            .justify_start()
                                            .w_full()
                                            .selected(self.section == section)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.section = section;
                                                if !this.busy {
                                                    this.message.clear();
                                                }
                                                cx.notify();
                                            }))
                                    }),
                            ),
                    )
                    .child(
                        v_flex()
                            .id("settings-scroll")
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .h_full()
                            .overflow_y_scroll()
                            .px_6()
                            .pb_6()
                            .gap_6()
                            .child(div().text_lg().font_semibold().child(self.section.label()))
                            .when(self.section == Section::Appearance, |column| {
                                column.child(appearance)
                            })
                            .when(self.section == Section::General, |column| {
                                column.child(connection).child(sync)
                            })
                            .when(self.section == Section::Ai, |column| {
                                column.child(self.ai.clone())
                            }),
                    ),
            )
    }
}
