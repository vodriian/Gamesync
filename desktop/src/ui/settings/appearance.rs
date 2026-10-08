//! Look and feel: day/night mode, native or Theme look, and Theme palettes.
//!
//! Native looks hide the scheme, contrast, and accent rows. Their saved values
//! stay unchanged and return when Theme is chosen again.
use super::*;
use crate::ui::controls::{self, Segment};
use gamesync_desktop::appearance::{
    catalog, Appearance, AppearanceMode, DarkContrast, LightContrast, Look, NativePlatform,
};
use gpui::{div, px, SharedString};
use gpui_component::{
    checkbox::Checkbox,
    h_flex,
    menu::{DropdownMenu as _, PopupMenuItem},
    v_flex, ActiveTheme as _, StyledExt as _,
};

fn heading(label: &'static str) -> gpui::Div {
    div().font_medium().child(label)
}

fn hex(tokens: &std::collections::BTreeMap<String, String>, key: &str) -> gpui::Hsla {
    let value = u32::from_str_radix(&tokens[key][1..], 16).expect("validated palette color");
    gpui::rgba(value).into()
}

impl SettingsView {
    /// A labeled pop-up whose menu previews each choice immediately.
    fn appearance_popup(
        &self,
        id: &'static str,
        label: &'static str,
        value: String,
        choices: Vec<(String, Appearance)>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .flex_1()
            .min_w_0()
            .gap_2()
            .child(heading(label))
            .child(self.appearance_menu(id, value, choices, cx))
    }

    /// A pop-up whose menu applies each choice immediately.
    fn appearance_menu(
        &self,
        id: &'static str,
        value: String,
        choices: Vec<(String, Appearance)>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let target = cx.entity();
        controls::popup(id, value.clone(), false, cx).dropdown_menu(move |mut menu, _, _| {
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
        })
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
            match theme {
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
                        // The app saves the chosen look with the rest of the appearance.
                        cx.emit(SettingsEvent::Theme(this.theme.clone()));
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

    fn change_look(&mut self, look: Look, window: &mut Window, cx: &mut Context<Self>) {
        if self.theme.look == look {
            return;
        }
        let mut appearance = self.theme.clone();
        appearance.look = look;
        if self.native_platform() == Some(NativePlatform::Omarchy) {
            self.theme = appearance;
            self.change_omarchy_mode(look == Look::Native, window, cx);
        } else {
            self.change_appearance(appearance, window, cx);
        }
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

    fn native_platform(&self) -> Option<NativePlatform> {
        NativePlatform::current(self.omarchy_available || self.omarchy_mode)
    }

    fn mode_choice(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let segments = [
            ("Auto", AppearanceMode::Auto),
            ("Light", AppearanceMode::Light),
            ("Dark", AppearanceMode::Dark),
        ]
        .into_iter()
        .map(|(label, mode)| {
            Segment::new(
                label,
                self.theme.mode == mode,
                cx.listener(move |this, _, window, cx| {
                    let mut a = this.theme.clone();
                    a.mode = mode;
                    this.change_appearance(a, window, cx);
                }),
            )
        })
        .collect();
        v_flex()
            .flex_1()
            .gap_2()
            .child(heading("Mode"))
            // Omarchy decides light or dark from its own theme.
            .child(controls::segmented(
                "appearance-mode",
                segments,
                self.omarchy_mode,
                cx,
            ))
    }

    fn look_choice(&self, platform: NativePlatform, cx: &mut Context<Self>) -> impl IntoElement {
        let segments = [(platform.label(), Look::Native), ("Theme", Look::Theme)]
            .into_iter()
            .map(|(label, look)| {
                Segment::new(
                    label,
                    self.theme.look == look,
                    cx.listener(move |this, _, window, cx| this.change_look(look, window, cx)),
                )
            })
            .collect();
        v_flex()
            .flex_1()
            .gap_2()
            .child(heading("Appearance"))
            .child(controls::segmented(
                "appearance-look",
                segments,
                self.saving_appearance_preference,
                cx,
            ))
            .when(platform == NativePlatform::Omarchy, |column| {
                let detail = if self.saving_appearance_preference {
                    "Updating the appearance…".to_owned()
                } else if let Some(name) = self
                    .omarchy_theme_name
                    .as_ref()
                    .filter(|_| self.omarchy_mode)
                {
                    format!("Following {name}. Theme changes apply automatically.")
                } else if !self.omarchy_available {
                    "Omarchy theme state was not found on this device.".to_owned()
                } else {
                    "Omarchy follows the active Omarchy theme.".to_owned()
                };
                column.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(detail),
                )
            })
    }

    /// The current scheme and contrast names for one mode, with every choice.
    #[allow(clippy::type_complexity)]
    fn palette_choices(
        &self,
        dark: bool,
    ) -> (
        String,
        Vec<(String, Appearance)>,
        String,
        Vec<(String, Appearance)>,
    ) {
        let make = |label: &str, change: fn(&mut Appearance)| {
            let mut a = self.theme.clone();
            change(&mut a);
            (label.to_string(), a)
        };
        let mode = if dark { "dark" } else { "light" };
        let id = if dark {
            &self.theme.dark_scheme
        } else {
            &self.theme.light_scheme
        };
        let schemes = catalog()
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
        let scheme = catalog()
            .schemes
            .iter()
            .find(|s| s.id == id.as_str())
            .map_or("Default", |s| s.name.as_str());
        let (contrast, contrasts) = if dark {
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
        (scheme.into(), schemes, contrast.into(), contrasts)
    }

    /// Scheme and contrast pop-ups for one mode, side by side.
    fn palette_row(&self, dark: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let (scheme, schemes, contrast, contrasts) = self.palette_choices(dark);
        h_flex()
            .gap_6()
            .items_start()
            .child(self.appearance_popup(
                if dark { "dark-scheme" } else { "light-scheme" },
                if dark {
                    "Dark color scheme"
                } else {
                    "Light color scheme"
                },
                scheme,
                schemes,
                cx,
            ))
            .child(self.appearance_popup(
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
                contrast,
                contrasts,
                cx,
            ))
    }

    fn save_reduce_motion(&mut self, value: bool, cx: &mut Context<Self>) {
        self.reduce_motion = value;
        cx.global_mut::<crate::ui::motion::MotionPreferences>()
            .reduced = value;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { settings::update(|s| s.reduce_motion = value) })
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
    }

    fn set_show_hidden_games(&mut self, value: bool, cx: &mut Context<Self>) {
        self.saving_hidden_preference = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { settings::update(|s| s.show_hidden_games = value) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.saving_hidden_preference = false;
                match result {
                    Ok(()) => this.library.update(cx, |library, cx| {
                        library.set_show_hidden_games(value);
                        cx.notify();
                    }),
                    Err(error) => {
                        this.message = format!("Could not save sidebar preference: {error}")
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn reduce_motion_row(&self, cx: &mut Context<Self>) -> impl IntoElement {
        controls::switch_row(
            "Reduce motion",
            controls::switch("reduce-motion", self.reduce_motion, false, cx).on_click(
                cx.listener(|this, _, _, cx| this.save_reduce_motion(!this.reduce_motion, cx)),
            ),
        )
    }

    fn hidden_games_row(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let shown = self.library.read(cx).show_hidden_games;
        controls::switch_row(
            "Show hidden games in sidebar",
            controls::switch(
                "show-hidden-games",
                shown,
                self.saving_hidden_preference,
                cx,
            )
            .on_click(cx.listener(move |this, _, _, cx| this.set_show_hidden_games(!shown, cx))),
        )
    }

    /// One Mode preview: a miniature window in that mode's colors. Auto shows the
    /// light and dark halves side by side, as macOS System Settings does.
    fn mode_tile(&self, mode: AppearanceMode, cx: &mut Context<Self>) -> impl IntoElement {
        const W: f32 = 68.;
        const H: f32 = 44.;
        const RADIUS: f32 = 6.;
        // Window panel's leading edge; Auto splits here: light sidebar, dark window.
        const SPLIT: f32 = 30.;
        let colors = |dark: bool| {
            let pick = |palette: &gamesync_desktop::appearance::Palette| {
                let content = &palette.content;
                (
                    hex(content, "background-secondary"),
                    hex(content, "background-primary"),
                    hex(content, "text-normal"),
                )
            };
            if self.theme.look == Look::Native {
                pick(&gamesync_desktop::native_palette::macos(dark, None))
            } else {
                let mut a = self.theme.clone();
                a.mode = if dark {
                    AppearanceMode::Dark
                } else {
                    AppearanceMode::Light
                };
                pick(a.palette(dark))
            }
        };
        let accent = cx.theme().primary;
        // GPUI clips children to rectangles, so every layer that meets a tile
        // corner rounds that corner itself.
        let scene = move |(sidebar, window, text): (gpui::Hsla, gpui::Hsla, gpui::Hsla)| {
            let dot = |color: u32| div().size(px(4.)).rounded_full().bg(gpui::rgb(color));
            div()
                .absolute()
                .top_0()
                .left_0()
                .w(px(W))
                .h(px(H))
                .rounded(px(RADIUS))
                .bg(sidebar)
                .child(
                    h_flex()
                        .absolute()
                        .top(px(5.))
                        .left(px(6.))
                        .gap(px(2.))
                        .child(dot(0xff5f57))
                        .child(dot(0xfebc2e))
                        .child(dot(0x28c840)),
                )
                .child(
                    div()
                        .absolute()
                        .top(px(15.))
                        .left(px(6.))
                        .w(px(18.))
                        .h(px(4.))
                        .rounded(px(2.))
                        .bg(accent),
                )
                .child(
                    v_flex()
                        .absolute()
                        .top(px(10.))
                        .left(px(SPLIT))
                        .w(px(W - SPLIT))
                        .h(px(H - 10.))
                        .rounded_tl(px(4.))
                        .rounded_br(px(RADIUS))
                        .bg(window)
                        .p(px(5.))
                        .gap(px(3.))
                        .child(
                            div()
                                .w(px(24.))
                                .h(px(3.))
                                .rounded_full()
                                .bg(text.opacity(0.5)),
                        )
                        .child(
                            div()
                                .w(px(16.))
                                .h(px(3.))
                                .rounded_full()
                                .bg(text.opacity(0.25)),
                        ),
                )
        };
        let half = |dark: bool| {
            div()
                .relative()
                .w(px(if dark { W - SPLIT } else { SPLIT }))
                .h_full()
                .overflow_hidden()
                .map(|half| {
                    if dark {
                        half.rounded_r(px(RADIUS))
                    } else {
                        half.rounded_l(px(RADIUS))
                    }
                })
                .child(scene(colors(dark)).when(dark, |scene| scene.left(px(-SPLIT))))
        };
        let preview = match mode {
            AppearanceMode::Auto => h_flex().size_full().child(half(false)).child(half(true)),
            mode => div()
                .relative()
                .size_full()
                .rounded(px(RADIUS))
                .overflow_hidden()
                .child(scene(colors(mode == AppearanceMode::Dark))),
        };
        let selected = self.theme.mode == mode;
        let disabled = self.omarchy_mode;
        let theme = cx.theme();
        v_flex()
            .id(match mode {
                AppearanceMode::Auto => "mode-auto",
                AppearanceMode::Light => "mode-light",
                AppearanceMode::Dark => "mode-dark",
            })
            .items_center()
            .gap_1()
            .when(disabled, |tile| tile.opacity(0.5))
            .when(!disabled, |tile| tile.cursor_pointer())
            .child(
                div()
                    .p(px(2.))
                    .rounded(px(RADIUS + 3.))
                    .border_2()
                    .border_color(if selected {
                        theme.primary
                    } else {
                        theme.transparent
                    })
                    .child(
                        div()
                            .relative()
                            .w(px(W))
                            .h(px(H))
                            .child(preview)
                            // An overlay keeps the hairline from shrinking the preview.
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .rounded(px(RADIUS))
                                    .border_1()
                                    .border_color(theme.border),
                            ),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .when(selected, |label| label.font_semibold())
                    .text_color(if selected {
                        theme.foreground
                    } else {
                        theme.muted_foreground
                    })
                    .child(match mode {
                        AppearanceMode::Auto => "Auto",
                        AppearanceMode::Light => "Light",
                        AppearanceMode::Dark => "Dark",
                    }),
            )
            .when(!disabled, |tile| {
                tile.on_click(cx.listener(move |this, _, window, cx| {
                    let mut a = this.theme.clone();
                    a.mode = mode;
                    this.change_appearance(a, window, cx);
                }))
            })
    }

    /// macOS Settings form: trailing-aligned labels, controls in one column.
    /// The native look hides the Theme palette rows; their saved values stay.
    pub(super) fn look_and_feel_form(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let platform = self.native_platform();
        let theme_look = self.theme.look == Look::Theme || platform.is_none();
        let shown = self.library.read(cx).show_hidden_games;
        let checkbox = |id: &'static str, label: &'static str, checked: bool| {
            Checkbox::new(id).label(label).checked(checked)
        };
        v_flex()
            .gap_3()
            .child(
                controls::form_row(
                    "Mode:",
                    h_flex()
                        .gap_3()
                        .child(self.mode_tile(AppearanceMode::Light, cx))
                        .child(self.mode_tile(AppearanceMode::Dark, cx))
                        .child(self.mode_tile(AppearanceMode::Auto, cx)),
                )
                // Align the label with the tiles, not with their captions.
                .items_start(),
            )
            .when_some(platform, |form, platform| {
                let looks = [(platform.label(), Look::Native), ("Theme", Look::Theme)]
                    .into_iter()
                    .map(|(label, look)| {
                        Segment::new(
                            label,
                            self.theme.look == look,
                            cx.listener(move |this, _, window, cx| {
                                this.change_look(look, window, cx)
                            }),
                        )
                    })
                    .collect();
                form.child(controls::form_row(
                    "Appearance:",
                    controls::segmented(
                        "appearance-look",
                        looks,
                        self.saving_appearance_preference,
                        cx,
                    ),
                ))
            })
            .when(theme_look, |form| {
                let mut form = form.child(controls::form_separator(cx));
                for dark in [false, true] {
                    let (scheme, schemes, contrast, contrasts) = self.palette_choices(dark);
                    let (scheme_id, contrast_id, scheme_label, contrast_label) = if dark {
                        (
                            "dark-scheme",
                            "dark-contrast",
                            "Dark scheme:",
                            "Dark contrast:",
                        )
                    } else {
                        (
                            "light-scheme",
                            "light-contrast",
                            "Light scheme:",
                            "Light contrast:",
                        )
                    };
                    form = form
                        .child(controls::form_row(
                            scheme_label,
                            self.appearance_menu(scheme_id, scheme, schemes, cx),
                        ))
                        .child(controls::form_row(
                            contrast_label,
                            self.appearance_menu(contrast_id, contrast, contrasts, cx),
                        ));
                }
                let accent = |id, label, on: bool, change: fn(&mut Appearance, bool)| {
                    checkbox(id, label, on).on_click(cx.listener(
                        move |this, checked: &bool, window, cx| {
                            let mut a = this.theme.clone();
                            change(&mut a, *checked);
                            this.change_appearance(a, window, cx);
                        },
                    ))
                };
                form.child(
                    controls::form_row(
                        "Accent:",
                        v_flex()
                            .gap_2()
                            .child(accent(
                                "scheme-accent",
                                "Color scheme accent",
                                self.theme.scheme_accent,
                                |a, on| a.scheme_accent = on,
                            ))
                            .child(accent(
                                "interface-accent",
                                "Interface accent",
                                self.theme.interface_accent,
                                |a, on| a.interface_accent = on,
                            )),
                    )
                    .items_start(),
                )
            })
            .child(controls::form_separator(cx))
            .child(controls::form_row(
                "Sidebar:",
                checkbox("show-hidden-games", "Show hidden games", shown).on_click(cx.listener(
                    |this, checked: &bool, _, cx| this.set_show_hidden_games(*checked, cx),
                )),
            ))
            .child(controls::form_row(
                "Motion:",
                checkbox("reduce-motion", "Reduce motion", self.reduce_motion).on_click(
                    cx.listener(|this, checked: &bool, _, cx| {
                        this.save_reduce_motion(*checked, cx)
                    }),
                ),
            ))
    }

    fn accent_row(
        &self,
        id: &'static str,
        label: &'static str,
        on: bool,
        change: fn(&mut Appearance, bool),
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        controls::switch_row(
            label,
            controls::switch(SharedString::from(id), on, false, cx).on_click(cx.listener(
                move |this, _, window, cx| {
                    let mut a = this.theme.clone();
                    change(&mut a, !on);
                    this.change_appearance(a, window, cx);
                },
            )),
        )
    }

    pub(super) fn look_and_feel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let platform = self.native_platform();
        let theme_look = self.theme.look == Look::Theme || platform.is_none();
        v_flex()
            .gap_6()
            .child(
                h_flex()
                    .gap_6()
                    .items_start()
                    .child(self.mode_choice(cx))
                    .map(|row| match platform {
                        Some(platform) => row.child(self.look_choice(platform, cx)),
                        // Keep the two-column grid when no native look exists.
                        None => row.child(div().flex_1()),
                    }),
            )
            .when(theme_look, |group| {
                group
                    .child(self.palette_row(false, cx))
                    .child(self.palette_row(true, cx))
            })
            .child(
                v_flex()
                    .gap_1()
                    .child(heading("Additional settings"))
                    .child(div().h(px(4.)))
                    .child(self.reduce_motion_row(cx))
                    .child(self.hidden_games_row(cx))
                    .when(theme_look, |rows| {
                        rows.child(self.accent_row(
                            "scheme-accent",
                            "Color scheme accent",
                            self.theme.scheme_accent,
                            |a, on| a.scheme_accent = on,
                            cx,
                        ))
                        .child(self.accent_row(
                            "interface-accent",
                            "Interface accent",
                            self.theme.interface_accent,
                            |a, on| a.interface_accent = on,
                            cx,
                        ))
                    }),
            )
    }
}
