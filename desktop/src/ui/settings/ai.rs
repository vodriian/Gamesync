//! AI providers. API keys go to the OS credential store; the chosen model and
//! Ollama's address are device settings. Key tests and model lists are still
//! simulated, because GameSync makes no AI requests yet.
use gamesync_desktop::{
    credentials::{CredentialStore as _, OsCredential},
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

struct Provider {
    /// Stable id for settings and the credential entry. Never rename.
    id: &'static str,
    name: &'static str,
    /// Sample lists that stand in for model discovery until real tests exist.
    models: &'static [&'static str],
    /// Where to get a key, as (label, URL).
    help: (&'static str, &'static str),
}

const PROVIDERS: [Provider; 5] = [
    Provider {
        id: "openai",
        name: "OpenAI",
        models: &["gpt-5", "gpt-5-mini", "gpt-4.1"],
        help: (
            "platform.openai.com",
            "https://platform.openai.com/api-keys",
        ),
    },
    Provider {
        id: "claude",
        name: "Claude",
        models: &["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-4-5"],
        help: (
            "console.anthropic.com",
            "https://console.anthropic.com/settings/keys",
        ),
    },
    Provider {
        id: "grok",
        name: "Grok",
        models: &["grok-4", "grok-3-mini"],
        help: ("console.x.ai", "https://console.x.ai"),
    },
    Provider {
        id: "gemini",
        name: "Gemini",
        models: &["gemini-2.5-pro", "gemini-2.5-flash"],
        help: ("aistudio.google.com", "https://aistudio.google.com/apikey"),
    },
    Provider {
        id: "ollama",
        name: "Ollama",
        models: &["llama3.2", "qwen2.5", "mistral"],
        help: ("ollama.com", "https://ollama.com/download"),
    },
];
/// The only provider without a key. It uses a server address instead.
const OLLAMA: usize = 4;
const OLLAMA_ADDRESS: &str = "http://localhost:11434";

pub struct AiSettings {
    /// Added providers, by index in `PROVIDERS`. Loaded once from settings;
    /// keys are not read back, so opening Settings never asks for Keychain access.
    added: Vec<Option<AiProvider>>,
    /// The provider in the add form. None when every provider is added.
    draft: Option<usize>,
    key: Entity<InputState>,
    endpoint: Entity<InputState>,
    /// The last simulated test passed for the current form input.
    tested: bool,
    draft_model: Option<usize>,
    /// Clear the key field on the next render, which has the window.
    clear_key: bool,
    busy: bool,
    message: SharedString,
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
                    this.tested = false;
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
        Self {
            added,
            draft,
            key,
            endpoint,
            tested: false,
            draft_model: None,
            clear_key: false,
            busy: false,
            message: "".into(),
        }
    }

    fn choose_draft(&mut self, index: usize, cx: &mut Context<Self>) {
        self.draft = Some(index);
        self.draft_model = None;
        self.clear_key = true;
        self.message = "".into();
        cx.notify();
    }

    fn test_draft(&mut self, cx: &mut Context<Self>) {
        self.tested = true;
        self.draft_model.get_or_insert(0);
        self.message = "Simulated test passed. No connection was made.".into();
        cx.notify();
    }

    /// Save the key first, then the settings entry, so a listed provider
    /// always has its key.
    fn add_draft(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.draft else { return };
        if self.busy || !self.tested {
            return;
        }
        let provider = &PROVIDERS[index];
        let id = provider.id;
        let key = self.key.read(cx).value().trim().to_owned();
        let entry = AiProvider {
            model: self
                .draft_model
                .map(|model| provider.models[model].to_owned()),
            endpoint: (index == OLLAMA).then(|| self.endpoint.read(cx).value().trim().to_owned()),
        };
        self.busy = true;
        self.message = "Saving…".into();
        cx.spawn(async move |this, cx| {
            let saved = entry.clone();
            let result = cx
                .background_spawn(async move {
                    if index != OLLAMA {
                        OsCredential::ai(id)?.set(&key)?;
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
                        this.message = if index == OLLAMA {
                            "Ollama added.".into()
                        } else {
                            "Key saved in secure storage.".into()
                        };
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

    fn set_model(&mut self, index: usize, model: &'static str, cx: &mut Context<Self>) {
        let Some(entry) = self.added[index].as_mut() else {
            return;
        };
        entry.model = Some(model.to_owned());
        let id = PROVIDERS[index].id;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    settings::update(|settings| {
                        if let Some(entry) = settings.ai_providers.get_mut(id) {
                            entry.model = Some(model.to_owned());
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

    /// Replace the key, or Ollama's address. An empty field keeps what is saved.
    fn save_edit(&mut self, index: usize, value: String, cx: &mut Context<Self>) {
        let value = value.trim().to_owned();
        if value.is_empty() || self.busy {
            return;
        }
        let id = PROVIDERS[index].id;
        let address = (index == OLLAMA).then(|| value.clone());
        self.busy = true;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    if index == OLLAMA {
                        settings::update(|settings| {
                            if let Some(entry) = settings.ai_providers.get_mut(id) {
                                entry.endpoint = Some(value);
                            }
                        })
                    } else {
                        OsCredential::ai(id)?.set(&value)
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(()) => match (this.added[index].as_mut(), address) {
                        (Some(entry), Some(address)) => {
                            entry.endpoint = Some(address);
                            this.message = "Server address saved.".into();
                        }
                        _ => this.message = "Key replaced in secure storage.".into(),
                    },
                    Err(error) => this.message = error.to_string().into(),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
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
                    if index != OLLAMA {
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
        let local = index == OLLAMA;
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
        let Provider { name, help, .. } = PROVIDERS[index];
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
        let button = Button::new(id)
            .label(selected.clone().unwrap_or_else(|| "Choose a model".into()))
            .icon(IconName::ChevronDown);
        // A row picker stays quiet; the add form's picker is a visible field.
        let button = if draft {
            button.outline()
        } else {
            button.ghost().small()
        };
        button.dropdown_menu(move |mut menu, _, _| {
            for (model, name) in PROVIDERS[index].models.iter().enumerate() {
                let target = target.clone();
                menu = menu.item(
                    PopupMenuItem::new(*name)
                        .checked(selected.as_deref() == Some(*name))
                        .on_click(move |_, _, cx| {
                            target.update(cx, |this, cx| {
                                if draft {
                                    this.draft_model = Some(model);
                                    cx.notify();
                                } else {
                                    this.set_model(index, PROVIDERS[index].models[model], cx);
                                }
                            });
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

    fn add_form(&self, index: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let local = index == OLLAMA;
        let input = if local { &self.endpoint } else { &self.key };
        let ready = !input.read(cx).value().trim().is_empty();
        let model = self.draft_model.map(|model| PROVIDERS[index].models[model]);
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
            .when(self.tested, |form| {
                form.child("Model").child(
                    h_flex()
                        .gap_2()
                        .child(self.model_picker("ai-draft-model", index, model, true, cx))
                        .child(
                            Button::new("add-ai")
                                .primary()
                                .label(format!("Add {}", PROVIDERS[index].name))
                                .disabled(self.busy)
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
            self.tested = false;
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
            .children(add)
            .when(!self.message.is_empty(), |page| {
                page.child(div().text_sm().child(self.message.clone()))
            })
    }
}
