//! Compact native selectors for independently saved day and night palettes.
use super::*;
use gamesync_desktop::appearance::{
    catalog, Appearance, AppearanceMode, DarkContrast, LightContrast,
};
use gpui::{div, px, SharedString};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex,
    menu::{DropdownMenu as _, PopupMenuItem},
    v_flex, ActiveTheme as _, Disableable as _, IconName, Sizable as _,
};
impl SettingsView {
    fn appearance_row(
        &self,
        id: &'static str,
        label: &'static str,
        value: String,
        choices: Vec<(String, Appearance)>,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let target = cx.entity();
        h_flex()
            .justify_between()
            .gap_4()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(div().child(label))
            .child(
                Button::new(id)
                    .ghost()
                    .small()
                    .disabled(disabled)
                    .label(value.clone())
                    .icon(IconName::ChevronDown)
                    .dropdown_menu(move |mut menu, _, _| {
                        for (label, appearance) in &choices {
                            let target = target.clone();
                            let appearance = appearance.clone();
                            menu = menu.item(
                                PopupMenuItem::new(label.clone())
                                    .checked(label == &value)
                                    .on_click(move |_, window, cx| {
                                        target.update(cx, |this, cx| {
                                            this.change_appearance(appearance.clone(), window, cx)
                                        });
                                    }),
                            );
                        }
                        menu
                    }),
            )
    }

    fn change_omarchy_mode(&mut self, enabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving_appearance_preference {
            return;
        }
        self.saving_appearance_preference = true;
        self.message = if enabled {
            "Reading the current Omarchy theme…".into()
        } else {
            "Restoring GameSync colors…".into()
        };
        let fallback = self.theme.clone();
        let window = window.window_handle();
        cx.spawn(async move |this, cx| {
            let theme = if enabled {
                cx.background_spawn(async {
                    gamesync_desktop::omarchy::OmarchyTheme::load_active().map(Some)
                })
                .await
            } else {
                Ok(None)
            };
            let result = match theme {
                Ok(theme) => {
                    let saved = cx
                        .background_spawn(async move {
                            settings::update(|settings| settings.omarchy_mode = enabled)
                        })
                        .await;
                    saved.map(|()| theme)
                }
                Err(error) => Err(error),
            };
            match result {
                Ok(theme) => {
                    let applied = theme.clone();
                    let _ = window.update(cx, move |_, window, cx| {
                        if let Some(theme) = &applied {
                            crate::theme::apply_omarchy(theme, window, cx);
                        } else {
                            crate::theme::apply_choice(&fallback, window, cx);
                        }
                    });
                    let _ = this.update(cx, |this, cx| {
                        this.saving_appearance_preference = false;
                        this.omarchy_mode = enabled;
                        this.omarchy_theme_name = theme.as_ref().map(|theme| theme.name.clone());
                        this.message.clear();
                        cx.emit(SettingsEvent::OmarchyMode(enabled, theme));
                        cx.notify();
                    });
                }
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        this.saving_appearance_preference = false;
                        this.message = format!(
                            "Could not {} Omarchy mode: {error:#}",
                            if enabled { "enable" } else { "disable" }
                        );
                        cx.notify();
                    });
                }
            }
        })
        .detach();
        cx.notify();
    }

    pub(super) fn omarchy_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let detail = if self.saving_appearance_preference {
            "Updating the appearance…".to_owned()
        } else if !self.omarchy_available {
            "Omarchy theme state was not found on this device.".to_owned()
        } else if let Some(name) = &self.omarchy_theme_name {
            format!("Following {name}. Theme changes apply automatically.")
        } else {
            "Use the active Omarchy theme and follow changes.".to_owned()
        };
        v_flex()
            .gap_1()
            .child(
                Checkbox::new("omarchy-mode")
                    .label("Sync with Omarchy theme")
                    .checked(self.omarchy_mode)
                    .disabled(
                        (!self.omarchy_available && !self.omarchy_mode)
                            || self.saving_appearance_preference,
                    )
                    .on_click(cx.listener(|this, checked: &bool, window, cx| {
                        this.change_omarchy_mode(*checked, window, cx)
                    })),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(detail),
            )
    }
    fn change_appearance(
        &mut self,
        appearance: Appearance,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.theme = appearance.clone();
        crate::theme::apply_choice(&appearance, window, cx);
        cx.emit(SettingsEvent::Theme(appearance));
        cx.notify();
    }
    pub(super) fn appearance_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let make = |label: &str, change: fn(&mut Appearance)| {
            let mut a = self.theme.clone();
            change(&mut a);
            (label.to_string(), a)
        };
        let mode = match self.theme.mode {
            AppearanceMode::Auto => "Auto",
            AppearanceMode::Light => "Light",
            AppearanceMode::Dark => "Dark",
        };
        let mut group = v_flex().gap_1().child(self.appearance_row(
            "appearance-mode",
            "Appearance",
            mode.into(),
            vec![
                make("Auto", |a| a.mode = AppearanceMode::Auto),
                make("Light", |a| a.mode = AppearanceMode::Light),
                make("Dark", |a| a.mode = AppearanceMode::Dark),
            ],
            self.omarchy_mode,
            cx,
        ));
        for dark in [false, true] {
            let mode = if dark { "dark" } else { "light" };
            let id = if dark {
                &self.theme.dark_scheme
            } else {
                &self.theme.light_scheme
            };
            let choices = catalog()
                .schemes
                .iter()
                .filter(|s| s.modes.iter().any(|m| m == mode))
                .map(|s| {
                    let mut a = self.theme.clone();
                    if dark {
                        a.dark_scheme = s.id.clone().into()
                    } else {
                        a.light_scheme = s.id.clone().into()
                    };
                    (s.name.clone(), a)
                })
                .collect();
            let label = catalog()
                .schemes
                .iter()
                .find(|s| s.id == id.as_str())
                .map_or("Default", |s| s.name.as_str());
            group = group.child(self.appearance_row(
                if dark { "dark-scheme" } else { "light-scheme" },
                if dark {
                    "Dark color scheme"
                } else {
                    "Light color scheme"
                },
                label.into(),
                choices,
                self.omarchy_mode,
                cx,
            ));
            let (value, choices) = if dark {
                (
                    match self.theme.dark_contrast {
                        DarkContrast::Normal => "Normal",
                        DarkContrast::Tonal => "Tonal",
                        DarkContrast::Black => "Black",
                    },
                    vec![
                        make("Normal", |a| a.dark_contrast = DarkContrast::Normal),
                        make("Tonal", |a| a.dark_contrast = DarkContrast::Tonal),
                        make("Black", |a| a.dark_contrast = DarkContrast::Black),
                    ],
                )
            } else {
                (
                    match self.theme.light_contrast {
                        LightContrast::Normal => "Normal",
                        LightContrast::Tonal => "Tonal",
                        LightContrast::Vivid => "Vivid",
                    },
                    vec![
                        make("Normal", |a| a.light_contrast = LightContrast::Normal),
                        make("Tonal", |a| a.light_contrast = LightContrast::Tonal),
                        make("Vivid", |a| a.light_contrast = LightContrast::Vivid),
                    ],
                )
            };
            group = group.child(self.appearance_row(
                if dark {
                    "dark-contrast"
                } else {
                    "light-contrast"
                },
                if dark {
                    "Dark contrast"
                } else {
                    "Light contrast"
                },
                value.into(),
                choices,
                self.omarchy_mode,
                cx,
            ));
        }
        for (id, label, checked) in [
            (
                "interface-accent",
                "Interface accent",
                self.theme.interface_accent,
            ),
            (
                "scheme-accent",
                "Color scheme accent",
                self.theme.scheme_accent,
            ),
        ] {
            group = group.child(
                h_flex().min_h(px(40.)).child(
                    Checkbox::new(SharedString::from(id))
                        .label(label)
                        .checked(checked)
                        .disabled(self.omarchy_mode)
                        .on_click(cx.listener(move |this, value: &bool, window, cx| {
                            let mut a = this.theme.clone();
                            if id == "interface-accent" {
                                a.interface_accent = *value
                            } else {
                                a.scheme_accent = *value
                            };
                            this.change_appearance(a, window, cx);
                        })),
                ),
            );
        }
        group
    }
}
