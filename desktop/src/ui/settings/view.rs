//! Fixed feedback above Glaze-style groups keeps errors and progress in view.
use super::*;
use gpui::{div, px};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    input::Input,
    v_flex, ActiveTheme as _, Disableable as _, IconName, Selectable as _, Sizable as _,
    StyledExt as _, WindowExt as _,
};

const STEAM_KEY_HELP: (&str, &str) = (
    "steamcommunity.com/dev/apikey",
    "https://steamcommunity.com/dev/apikey",
);

/// "Get your API key from <link>.", as in the AI provider dialog.
fn steam_help(cx: &gpui::App) -> gpui::Div {
    let (label, url) = STEAM_KEY_HELP;
    h_flex()
        .flex_wrap()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child("Get your API key from\u{a0}")
        .child(
            div()
                .id("steam-help-link")
                .text_color(cx.theme().link)
                .cursor_pointer()
                .child(label)
                .on_click(move |_, _, cx| cx.open_url(url)),
        )
        .child(".")
}

/// One labeled input on the group surface, as in the AI provider dialog.
fn dialog_field(label: &'static str, input: &Entity<InputState>, cx: &gpui::App) -> gpui::Div {
    h_flex()
        .gap_3()
        .pl_3()
        .rounded(cx.theme().radius_lg)
        .bg(cx.theme().group_box)
        .child(div().flex_shrink_0().child(label))
        .child(div().flex_1().child(Input::new(input).appearance(false)))
}

impl SettingsView {
    /// Settings → AI pattern: a saved key is one row with an info button that
    /// opens the editor; without a key, an add form saves after a passing test.
    fn steam_key(&self, has_library: bool, cx: &mut Context<Self>) -> gpui::Div {
        let title = div().font_semibold().child("Steam API key");
        if self.key_saved {
            let row = h_flex()
                .gap_2()
                .pl_4()
                .pr_2()
                .py_1()
                .rounded(cx.theme().radius_lg)
                .bg(cx.theme().group_box)
                .child(div().flex_1().child("Steam"))
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("Key saved"),
                )
                .child(
                    Button::new("steam-info")
                        .ghost()
                        .small()
                        .icon(IconName::Info)
                        .tooltip("Edit Steam connection")
                        .disabled(self.steam_busy(cx) || !has_library)
                        .on_click(
                            cx.listener(|this, _, window, cx| this.open_steam_editor(window, cx)),
                        ),
                );
            return v_flex().gap_2().child(title).child(row);
        }
        let tested = self.tested_matches(cx);
        let form = v_flex()
            .p_5()
            .gap_4()
            .rounded(cx.theme().radius_lg)
            .bg(cx.theme().group_box)
            .child("Steam profile or ID")
            .child(Input::new(&self.profile).disabled(self.steam_busy(cx) || !has_library))
            .child("API key")
            .child(Input::new(&self.key).disabled(self.steam_busy(cx) || !has_library))
            .child(steam_help(cx))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("test-steam")
                            .label("Test key")
                            .disabled(self.steam_busy(cx) || !has_library)
                            .on_click(cx.listener(|this, _, _, cx| this.test_key(cx))),
                    )
                    .when(tested, |row| {
                        row.child(
                            Button::new("save-steam")
                                .primary()
                                .label("Save key")
                                .disabled(self.steam_busy(cx))
                                .on_click(cx.listener(|this, _, _, cx| this.save_key(cx))),
                        )
                    }),
            );
        v_flex().gap_2().child(title).child(form)
    }

    /// Change the profile or key, or remove the key. Save tests first; an
    /// empty key field tests and keeps the saved key.
    fn open_steam_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // The saved key is not read back; an empty field keeps it.
        self.key.update(cx, |input, cx| {
            input.set_placeholder("Saved. Paste a new key to replace it.", window, cx)
        });
        let view = cx.entity();
        let profile = self.profile.clone();
        let key = self.key.clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let remove = view.clone();
            let save = view.clone();
            dialog
                .title("Steam")
                .w(px(500.))
                .close_button(false)
                .button_props(gpui_component::dialog::DialogButtonProps::default().ok_text("Save"))
                .child(
                    v_flex()
                        .gap_3()
                        .child(dialog_field("Steam profile or ID", &profile, cx))
                        .child(dialog_field("API key", &key, cx))
                        .child(steam_help(cx)),
                )
                .footer(move |ok, cancel, window, cx| {
                    let remove = remove.clone();
                    vec![
                        Button::new("steam-remove")
                            .danger()
                            .label("Remove key")
                            .on_click(move |_, window, cx| {
                                window.close_dialog(cx);
                                remove.update(cx, |this, cx| this.remove_key(cx));
                            })
                            .into_any_element(),
                        div().flex_1().into_any_element(),
                        cancel(window, cx),
                        ok(window, cx),
                    ]
                })
                .on_ok(move |_, _, cx| {
                    save.update(cx, |this, cx| this.test_and_save(cx));
                    true
                })
        });
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.clear_key {
            self.clear_key = false;
            self.key = Self::masked_input("Saved securely · enter a replacement", window, cx);
        }
        if self.clear_passphrase {
            self.clear_passphrase = false;
            self.passphrase = Self::masked_input("Sync passphrase", window, cx);
            self.passphrase_confirm = Self::masked_input("Repeat the passphrase", window, cx);
        }
        let source = self.library.read(cx).source.clone();
        let id = source.as_ref().map(|(_, m)| m.library_id);
        if id != self.library_id && !self.steam_busy(cx) {
            self.library_id = id;
            self.tested = None;
            self.key_saved = false;
            self.last_sync = None;
            self.message = self.steam.read(cx).message.clone();
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
        let surface = crate::ui::controls::group_surface(cx);
        let radius = cx.theme().radius_lg;
        let group = || v_flex().p_5().gap_4().rounded(radius).bg(surface);
        let appearance = group().child(self.look_and_feel(cx));
        let connection = self.steam_key(has_library, cx);
        let sync = group()
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("sync-steam")
                            .primary()
                            .label("Sync now")
                            .disabled(self.steam_busy(cx) || !has_library || !self.key_saved)
                            .on_click(cx.listener(|this, _, _, cx| this.sync(cx))),
                    )
                    .child(
                        Button::new("cancel-sync")
                            .label("Cancel sync")
                            .disabled(!self.steam.read(cx).running())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.steam.update(cx, |job, cx| job.cancel(cx));
                            })),
                    ),
            )
            .child(settings::sync_label(self.last_sync))
            .child("Store country for wishlist prices")
            .child(
                h_flex()
                    .gap_2()
                    .child(div().w(px(220.)).child(Input::new(&self.country)))
                    .child(
                        Button::new("save-country")
                            .label("Save country")
                            .on_click(cx.listener(|this, _, _, cx| this.save_country(cx))),
                    ),
            );
        // macOS keeps the toolbar-tab form in both looks; Theme changes only colors.
        let native = crate::ui::controls::MAC_SETTINGS;
        let feedback = (!self.message.is_empty()).then(|| {
            div()
                .id("settings-feedback")
                .max_h(px(96.))
                .overflow_y_scroll()
                .text_sm()
                .child(self.message.clone())
        });
        let body = v_flex()
            .gap_6()
            .when(self.section == Section::Appearance, |column| {
                column.child(if native {
                    self.look_and_feel_form(cx).into_any_element()
                } else {
                    appearance.into_any_element()
                })
            })
            .when(self.section == Section::General, |column| {
                column.child(connection).child(sync)
            })
            .when(self.section == Section::Sync, |column| {
                column.child(self.sync_section(cx))
            })
            .when(self.section == Section::BestOn, |column| {
                column.child(self.best_on.clone())
            })
            .when(self.section == Section::Ai, |column| {
                column.child(self.ai.clone())
            });
        let shell = if native {
            self.native_shell(feedback, body, cx).into_any_element()
        } else {
            self.theme_shell(feedback, body, cx).into_any_element()
        };
        div()
            .size_full()
            .child(shell)
            // AI provider details open in a modal.
            .children(gpui_component::Root::render_dialog_layer(window, cx))
    }
}

const SECTIONS: [Section; 5] = [
    Section::General,
    Section::Sync,
    Section::Appearance,
    Section::BestOn,
    Section::Ai,
];

fn section_icon(section: Section) -> gpui_component::Icon {
    match section {
        Section::General => gpui_component::Icon::from(gpui_component::IconName::Settings),
        Section::Sync => gpui_component::Icon::from(gpui_component::IconName::FolderOpen),
        Section::Appearance => gpui_component::Icon::from(gpui_component::IconName::Palette),
        Section::BestOn => gpui_component::Icon::new(crate::assets::SetupIcon::Pc),
        Section::Ai => gpui_component::Icon::from(gpui_component::IconName::Bot),
    }
}

impl SettingsView {
    fn select_section(&mut self, section: Section, cx: &mut Context<Self>) {
        self.section = section;
        self.stop_confirm = false;
        if !self.steam_busy(cx) {
            self.message.clear();
        }
        cx.notify();
    }

    /// macOS Settings window: title row, toolbar tabs, then one centered pane.
    fn native_shell(
        &self,
        feedback: Option<impl IntoElement>,
        body: impl IntoElement,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let tabs = h_flex()
            .justify_center()
            .gap_1()
            .pb_2()
            .border_b_1()
            .border_color(theme.border)
            .children(SECTIONS.into_iter().map(|section| {
                let selected = self.section == section;
                let tint = if selected {
                    theme.primary
                } else {
                    theme.muted_foreground
                };
                div()
                    .id(section.label())
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_0p5()
                    .min_w(px(68.))
                    .px_2()
                    .py_1()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .when(selected, |tab| tab.bg(theme.secondary_hover))
                    .hover(|tab| tab.bg(theme.secondary_hover))
                    .child(section_icon(section).size(px(22.)).text_color(tint))
                    .child(
                        div()
                            .text_xs()
                            .text_color(if selected {
                                theme.primary
                            } else {
                                theme.foreground
                            })
                            .child(section.label()),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.select_section(section, cx)))
            }));
        v_flex()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .text_size(px(13.))
            .child(
                // Center the title on the window, not on the space after the controls.
                crate::ui::chrome::compact_titlebar(cx)
                    .bg(gpui::transparent_black())
                    .pl(px(0.))
                    .justify_center()
                    .child(
                        div()
                            .flex_1()
                            .text_center()
                            .font_semibold()
                            .child("Settings"),
                    ),
            )
            .child(tabs)
            .child(
                v_flex()
                    .id("settings-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .items_center()
                    .child(
                        // A definite width: with w_full + max_w, wrapped text was
                        // measured narrower than drawn, so the scroll range was short.
                        // The window's 700 px minimum keeps this width in view.
                        v_flex()
                            .w(px(640.))
                            .px_8()
                            .py_6()
                            .gap_4()
                            .children(feedback)
                            .child(body),
                    ),
            )
    }

    /// Theme look: left navigation and grouped panels from the Elyx design.
    fn theme_shell(
        &self,
        feedback: Option<impl IntoElement>,
        body: impl IntoElement,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(crate::ui::chrome::compact_titlebar(cx))
            .child(
                v_flex()
                    .p_6()
                    .gap_2()
                    .flex_shrink_0()
                    .child(div().text_xl().font_semibold().child("Settings"))
                    .children(feedback),
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
                            .children(SECTIONS.into_iter().map(|section| {
                                Button::new(section.label())
                                    .ghost()
                                    .label(section.label())
                                    .icon(section_icon(section))
                                    .justify_start()
                                    .w_full()
                                    .selected(self.section == section)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.select_section(section, cx)
                                    }))
                            })),
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
                            .child(body),
                    ),
            )
    }
}
