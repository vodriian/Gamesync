//! AI providers. API keys go to the OS credential store; the chosen model,
//! discovered models, and Ollama's address are device settings. A key or
//! address is saved only after a real test lists the provider's models.
use gamesync_desktop::{
    ai::{Connection, ProviderInfo, OLLAMA_ADDRESS, PROVIDERS},
    credentials::{replace_checked, CredentialStore as _, OsCredential},
    settings::{self, AiProvider},
};
use gpui::{div, prelude::*, px, Entity, SharedString, Window};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu as _, PopupMenuItem},
    v_flex, ActiveTheme as _, Disableable as _, IconName, Selectable as _, Sizable as _,
    StyledExt as _, WindowExt as _,
};

/// The only provider without a key. It uses a server address instead.
fn local(index: usize) -> bool {
    PROVIDERS[index].local()
}

pub struct AiSettings {
    /// Added providers, by index in `PROVIDERS`. Loaded once from settings;
    /// keys are not read back, so opening Settings never asks for Keychain access.
    added: Vec<Option<AiProvider>>,
    /// The provider in the add form. None when every provider is added.
    draft: Option<usize>,
    key: Entity<InputState>,
    endpoint: Entity<InputState>,
    /// Models from the last passing test of the current form input. Empty
    /// until a test passes; an input edit clears it.
    draft_models: Vec<String>,
    draft_model: Option<String>,
    /// Ignore a test result after the input changed.
    test_epoch: u64,
    /// Clear the key field on the next render, which has the window.
    clear_key: bool,
    busy: bool,
    message: SharedString,
    /// The app's shared analysis job; Analyze games starts it for the library.
    analysis: Entity<crate::ui::analysis_job::AnalysisJob>,
}

impl AiSettings {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let key = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder("Add API key")
        });
        let endpoint = cx.new(|cx| InputState::new(window, cx).default_value(OLLAMA_ADDRESS));
        for input in [&key, &endpoint] {
            // An edit invalidates the last test.
            cx.subscribe(input, |this, _, event, cx| {
                if matches!(event, InputEvent::Change) {
                    this.draft_models.clear();
                    this.draft_model = None;
                    this.test_epoch += 1;
                    cx.notify();
                }
            })
            .detach();
        }
        let saved = settings::load()
            .map(|settings| settings.ai_providers)
            .unwrap_or_default();
        let added: Vec<_> = PROVIDERS
            .iter()
            .map(|provider| saved.get(provider.id).cloned())
            .collect();
        let draft = added.iter().position(Option::is_none);
        let analysis = cx
            .global::<crate::ui::analysis_job::AnalysisGlobal>()
            .0
            .clone();
        cx.observe(&analysis, |_, _, cx| cx.notify()).detach();
        Self {
            analysis,
            added,
            draft,
            key,
            endpoint,
            draft_models: Vec::new(),
            draft_model: None,
            test_epoch: 0,
            clear_key: false,
            busy: false,
            message: "".into(),
        }
    }

    fn choose_draft(&mut self, index: usize, cx: &mut Context<Self>) {
        self.draft = Some(index);
        self.draft_models.clear();
        self.draft_model = None;
        self.test_epoch += 1;
        self.clear_key = true;
        self.message = "".into();
        cx.notify();
    }

    fn draft_input(&self, index: usize, cx: &Context<Self>) -> String {
        let input = if local(index) {
            &self.endpoint
        } else {
            &self.key
        };
        input.read(cx).value().trim().to_owned()
    }

    /// A real request: list the provider's models with the entered key or address.
    fn test_draft(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.draft else { return };
        if self.busy {
            return;
        }
        let id = PROVIDERS[index].id;
        let value = self.draft_input(index, cx);
        let epoch = self.test_epoch;
        self.busy = true;
        self.message = "Testing…".into();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let connection = if local(index) {
                        Connection::new(id, Some(&value), None)?
                    } else {
                        Connection::new(id, None, Some(value))?
                    };
                    connection.models()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                if this.test_epoch != epoch || this.draft != Some(index) {
                    this.message = "".into();
                } else {
                    match result {
                        Ok(models) => {
                            this.message =
                                format!("Test passed. {} models found.", models.len()).into();
                            this.draft_models = models;
                        }
                        Err(error) => this.message = format!("{error:#}").into(),
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Save the key first, then the settings entry, so a listed provider
    /// always has its key.
    fn add_draft(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.draft else { return };
        if self.busy || self.draft_models.is_empty() || self.draft_model.is_none() {
            return;
        }
        let id = PROVIDERS[index].id;
        let value = self.draft_input(index, cx);
        let entry = AiProvider {
            model: self.draft_model.clone(),
            endpoint: local(index).then(|| value.clone()),
            models: self.draft_models.clone(),
        };
        self.busy = true;
        self.message = "Saving…".into();
        cx.spawn(async move |this, cx| {
            let saved = entry.clone();
            let result = cx
                .background_spawn(async move {
                    if !local(index) {
                        OsCredential::ai(id)?.set(&value)?;
                    }
                    settings::update(|settings| {
                        settings.ai_providers.insert(id.to_owned(), saved);
                    })
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(()) => {
                        this.added[index] = Some(entry);
                        this.message = if local(index) {
                            "Ollama added.".into()
                        } else {
                            "Key saved in secure storage.".into()
                        };
                        this.draft_models.clear();
                        this.draft_model = None;
                        this.clear_key = true;
                        this.draft = this.added.iter().position(Option::is_none);
                    }
                    Err(error) => this.message = error.to_string().into(),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn set_model(&mut self, index: usize, model: String, cx: &mut Context<Self>) {
        let Some(entry) = self.added[index].as_mut() else {
            return;
        };
        entry.model = Some(model.clone());
        let id = PROVIDERS[index].id;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    settings::update(|settings| {
                        if let Some(entry) = settings.ai_providers.get_mut(id) {
                            entry.model = Some(model);
                        }
                    })
                })
                .await;
            if let Err(error) = result {
                let _ = this.update(cx, |this, cx| {
                    this.message = error.to_string().into();
                    cx.notify();
                });
            }
        })
        .detach();
        cx.notify();
    }

    /// Test a new key or address, then save it with the models it lists.
    /// An empty field keeps what is saved. A failed test changes nothing.
    fn save_edit(&mut self, index: usize, value: String, cx: &mut Context<Self>) {
        let value = value.trim().to_owned();
        if value.is_empty() || self.busy {
            return;
        }
        self.busy = true;
        self.message = "Testing…".into();
        self.refresh(index, Some(value), cx);
        cx.notify();
    }

    /// List models again with the saved key, or with `replacement` before it
    /// is saved. Reading a saved key can show an OS prompt; only explicit actions call this.
    fn refresh(&mut self, index: usize, replacement: Option<String>, cx: &mut Context<Self>) {
        let Some(entry) = self.added[index].clone() else {
            return;
        };
        let id = PROVIDERS[index].id;
        self.busy = true;
        if replacement.is_none() {
            self.message = "Refreshing models…".into();
        }
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let mut entry = entry;
                    match (local(index), replacement) {
                        (true, address) => {
                            if let Some(address) = address {
                                entry.endpoint = Some(address);
                            }
                            entry.models = Connection::new(id, entry.endpoint.as_deref(), None)?
                                .models()?;
                        }
                        (false, Some(key)) => {
                            let mut models = Vec::new();
                            replace_checked(&OsCredential::ai(id)?, &key, |key| {
                                models = Connection::new(id, None, Some(key.to_owned()))?
                                    .models()?;
                                Ok(())
                            })?;
                            entry.models = models;
                        }
                        (false, None) => entry.models = Connection::saved(id, &entry)?.models()?,
                    }
                    let saved = entry.clone();
                    settings::update(|settings| {
                        if let Some(current) = settings.ai_providers.get_mut(id) {
                            current.endpoint = saved.endpoint;
                            current.models = saved.models;
                        }
                    })?;
                    anyhow::Ok(entry)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(entry) => {
                        let count = entry.models.len();
                        let kept = entry
                            .model
                            .as_ref()
                            .is_none_or(|model| entry.models.contains(model));
                        this.added[index] = Some(entry);
                        this.message = if kept {
                            format!("Test passed. {count} models found.")
                        } else {
                            format!("Test passed. {count} models found. The chosen model is not listed; choose another.")
                        }
                        .into();
                    }
                    Err(error) => this.message = format!("{error:#}").into(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Remove the key first; if that fails, the provider stays listed.
    fn delete(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let id = PROVIDERS[index].id;
        self.busy = true;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    if !local(index) {
                        OsCredential::ai(id)?.remove()?;
                    }
                    settings::update(|settings| {
                        settings.ai_providers.remove(id);
                    })
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(()) => {
                        this.added[index] = None;
                        this.message = format!("{} removed.", PROVIDERS[index].name).into();
                        if this.draft.is_none() {
                            this.draft = Some(index);
                        }
                    }
                    Err(error) => this.message = error.to_string().into(),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn open_editor(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let local = local(index);
        let input = if local {
            let address = self.added[index]
                .as_ref()
                .and_then(|entry| entry.endpoint.clone())
                .unwrap_or_else(|| OLLAMA_ADDRESS.to_owned());
            cx.new(|cx| InputState::new(window, cx).default_value(address))
        } else {
            // The saved key is not read back; an empty field keeps it.
            cx.new(|cx| {
                InputState::new(window, cx)
                    .masked(true)
                    .placeholder("Saved. Paste a new key to replace it.")
            })
        };
        let this = cx.entity();
        let ProviderInfo { name, help, .. } = PROVIDERS[index];
        let (help_label, help_url) = help;
        window.open_dialog(cx, move |dialog, _, cx| {
            let field = h_flex()
                .gap_3()
                .pl_3()
                .rounded(cx.theme().radius_lg)
                .bg(cx.theme().group_box)
                .child(div().flex_shrink_0().child(if local {
                    "Server address"
                } else {
                    "API key"
                }))
                .child(div().flex_1().child(Input::new(&input).appearance(false)));
            let help = h_flex()
                .flex_wrap()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(if local {
                    "Get Ollama from\u{a0}"
                } else {
                    "Get your API key from\u{a0}"
                })
                .child(
                    div()
                        .id("ai-help-link")
                        .text_color(cx.theme().link)
                        .cursor_pointer()
                        .child(help_label)
                        .on_click(move |_, _, cx| cx.open_url(help_url)),
                )
                .child(".");
            let delete = this.clone();
            let save = this.clone();
            let value = input.clone();
            dialog
                .title(name)
                .w(px(460.))
                .close_button(false)
                .child(v_flex().gap_3().child(field).child(help))
                .footer(move |ok, cancel, window, cx| {
                    let delete = delete.clone();
                    vec![
                        Button::new("ai-delete")
                            .danger()
                            .label("Delete")
                            .on_click(move |_, window, cx| {
                                window.close_dialog(cx);
                                delete.update(cx, |this, cx| this.delete(index, cx));
                            })
                            .into_any_element(),
                        div().flex_1().into_any_element(),
                        cancel(window, cx),
                        ok(window, cx),
                    ]
                })
                .on_ok(move |_, _, cx| {
                    let value = value.read(cx).value().to_string();
                    save.update(cx, |this, cx| this.save_edit(index, value, cx));
                    true
                })
        });
    }

    fn model_picker(
        &self,
        id: impl Into<gpui::ElementId>,
        index: usize,
        selected: Option<&str>,
        draft: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let target = cx.entity();
        let selected = selected.map(str::to_owned);
        let models = if draft {
            self.draft_models.clone()
        } else {
            self.added[index]
                .as_ref()
                .map(|entry| entry.models.clone())
                .unwrap_or_default()
        };
        let button = Button::new(id)
            .label(selected.clone().unwrap_or_else(|| "Choose a model".into()))
            .icon(IconName::ChevronDown)
            .disabled(self.busy);
        // A row picker stays quiet; the add form's picker is a visible field.
        let button = if draft {
            button.outline()
        } else {
            button.ghost().small()
        };
        button.dropdown_menu(move |mut menu, _, _| {
            menu = menu.scrollable(true).max_h(px(360.));
            for model in &models {
                let target = target.clone();
                let name = model.clone();
                menu = menu.item(
                    PopupMenuItem::new(model.clone())
                        .checked(selected.as_deref() == Some(model.as_str()))
                        .on_click(move |_, _, cx| {
                            let name = name.clone();
                            target.update(cx, |this, cx| {
                                if draft {
                                    this.draft_model = Some(name);
                                    cx.notify();
                                } else {
                                    this.set_model(index, name, cx);
                                }
                            });
                        }),
                );
            }
            if !draft {
                // Lists saved before real tests are empty; this fills them.
                let target = target.clone();
                menu = menu
                    .separator()
                    .item(
                        PopupMenuItem::new("Refresh models").on_click(move |_, _, cx| {
                            target.update(cx, |this, cx| {
                                if !this.busy {
                                    this.refresh(index, None, cx);
                                    cx.notify();
                                }
                            })
                        }),
                    );
            }
            menu
        })
    }

    fn provider_row(
        &self,
        index: usize,
        entry: &AiProvider,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let name = PROVIDERS[index].name;
        h_flex()
            .gap_2()
            .pl_4()
            .pr_2()
            .py_1()
            .rounded(cx.theme().radius_lg)
            .bg(cx.theme().group_box)
            .child(div().flex_1().child(name))
            .child(self.model_picker(
                ("ai-row-model", index),
                index,
                entry.model.as_deref(),
                false,
                cx,
            ))
            .child(
                Button::new(("ai-row-info", index))
                    .ghost()
                    .small()
                    .icon(IconName::Info)
                    .tooltip(format!("Edit {name}"))
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.open_editor(index, window, cx)),
                    ),
            )
    }

    /// Analyze games: add Play now data to every game except hidden ones.
    /// Shown only when a provider has a chosen model.
    fn analyze_games(&self, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::ui::analysis_job::Scope;
        let job = self.analysis.read(cx);
        let muted = cx.theme().muted_foreground;
        let detail = if job.running() || !job.status().is_empty() {
            job.status().to_owned()
        } else {
            job.last_run(cx).map_or_else(
                || "Not run yet".to_owned(),
                |time| format!("Last: {}", settings::age_label(time.max(0) as u64)),
            )
        };
        let analysis = self.analysis.clone();
        let action = if job.running() {
            Button::new("analyze-games-cancel")
                .small()
                .label("Cancel")
                .disabled(job.cancelling())
                .on_click(move |_, _, cx| analysis.update(cx, |job, cx| job.cancel(cx)))
        } else if job.can_retry() {
            Button::new("analyze-games-retry")
                .small()
                .label("Retry")
                .on_click(move |_, _, cx| analysis.update(cx, |job, cx| job.retry(cx)))
        } else {
            Button::new("analyze-games")
                .small()
                .outline()
                .label("Analyze")
                .on_click(move |_, window, cx| {
                    analysis.update(cx, |job, cx| job.confirm(Scope::Library, window, cx))
                })
        };
        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .gap_3()
                    .pl_4()
                    .pr_2()
                    .py_2()
                    .rounded(cx.theme().radius_lg)
                    .bg(cx.theme().group_box)
                    .child(
                        gpui_component::Icon::new(crate::assets::PlayIcon("ai-beautify"))
                            .size(px(20.))
                            .text_color(cx.theme().primary),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child("Analyze games")
                            .child(div().text_xs().text_color(muted).child(detail)),
                    )
                    .child(action),
            )
            .child(div().text_sm().text_color(muted).child(
                "Adds Play now data to every game except hidden ones. Games with a current suggestion are skipped. You pay your provider for usage.",
            ))
    }

    fn add_form(&self, index: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let local = local(index);
        let input = if local { &self.endpoint } else { &self.key };
        let ready = !input.read(cx).value().trim().is_empty();
        let tested = !self.draft_models.is_empty();
        v_flex()
            .p_5()
            .gap_4()
            .rounded(cx.theme().radius_lg)
            .bg(cx.theme().group_box)
            .child(div().font_semibold().child(PROVIDERS[index].name))
            .child(if local { "Server address" } else { "API key" })
            .child(Input::new(input))
            .child(
                h_flex().child(
                    Button::new("test-ai")
                        .label(if local { "Test connection" } else { "Test key" })
                        .disabled(!ready || self.busy)
                        .on_click(cx.listener(|this, _, _, cx| this.test_draft(cx))),
                ),
            )
            .when(tested, |form| {
                form.child("Model").child(
                    h_flex()
                        .gap_2()
                        .child(self.model_picker(
                            "ai-draft-model",
                            index,
                            self.draft_model.as_deref(),
                            true,
                            cx,
                        ))
                        .child(
                            Button::new("add-ai")
                                .primary()
                                .label(format!("Add {}", PROVIDERS[index].name))
                                .disabled(self.busy || self.draft_model.is_none())
                                .on_click(cx.listener(|this, _, _, cx| this.add_draft(cx))),
                        ),
                )
            })
    }
}

impl Render for AiSettings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.clear_key) {
            self.key
                .update(cx, |input, cx| input.set_value("", window, cx));
            self.draft_models.clear();
            self.draft_model = None;
        }
        let rows: Vec<_> = self
            .added
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| entry.clone().map(|entry| (index, entry)))
            .collect();
        let muted = cx.theme().muted_foreground;
        let list = v_flex()
            .gap_2()
            .child(div().font_semibold().child("Providers"))
            .when(rows.is_empty(), |list| {
                list.child(
                    div()
                        .text_sm()
                        .text_color(muted)
                        .child("No providers yet. Add one below."),
                )
            })
            .children(
                rows.iter()
                    .map(|(index, entry)| self.provider_row(*index, entry, cx))
                    .collect::<Vec<_>>(),
            );
        let ready = rows.iter().any(|(_, entry)| entry.model.is_some())
            && self.analysis.read(cx).available(cx);
        let analyze = ready.then(|| self.analyze_games(cx));
        let add = self.draft.map(|draft| {
            let choices: Vec<_> = PROVIDERS
                .iter()
                .enumerate()
                .filter(|(index, _)| self.added[*index].is_none())
                .map(|(index, provider)| {
                    Button::new(provider.id)
                        .label(provider.name)
                        .selected(draft == index)
                        .on_click(cx.listener(move |this, _, _, cx| this.choose_draft(index, cx)))
                })
                .collect();
            v_flex()
                .gap_2()
                .child(div().font_semibold().child("Add a provider"))
                .child(h_flex().flex_wrap().gap_2().children(choices))
                .child(self.add_form(draft, cx))
        });
        v_flex()
            .gap_6()
            .child(list)
            .children(analyze)
            .children(add)
            .when(!self.message.is_empty(), |page| {
                page.child(div().text_sm().child(self.message.clone()))
            })
    }
}
