//! UI-only connection preview. No credentials, network calls, or disk writes.
use gpui::{div, prelude::*, Entity, Window};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu as _, PopupMenuItem},
    v_flex, ActiveTheme as _, Disableable as _, IconName, Selectable as _, StyledExt as _,
};

const PROVIDERS: [&str; 5] = ["OpenAI", "Claude", "Grok", "Gemini", "Ollama"];
const OLLAMA: usize = 4;

/// Sample lists that stand in for model discovery until real tests exist.
const MODELS: [&[&str]; 5] = [
    &["gpt-5", "gpt-5-mini", "gpt-4.1"],
    &["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-4-5"],
    &["grok-4", "grok-3-mini"],
    &["gemini-2.5-pro", "gemini-2.5-flash"],
    &["llama3.2", "qwen2.5", "mistral"],
];

struct Connection {
    key: Entity<InputState>,
    tested: bool,
    added: bool,
    model: Option<usize>,
    message: &'static str,
}

impl Connection {
    /// Models show only after a passing test or for an added key.
    fn models_ready(&self) -> bool {
        self.tested || self.added
    }
}

pub struct AiSettings {
    connections: Vec<Connection>,
    provider: usize,
    endpoint: Entity<InputState>,
}

/// An edit invalidates the last simulated test for that provider.
fn reset_on_change(input: &Entity<InputState>, index: usize, cx: &mut Context<AiSettings>) {
    cx.subscribe(input, move |this, _, event, cx| {
        if matches!(event, InputEvent::Change) {
            this.connections[index].tested = false;
            this.connections[index].message = "";
            cx.notify();
        }
    })
    .detach();
}

fn key_input(
    placeholder: &'static str,
    window: &mut Window,
    cx: &mut Context<AiSettings>,
) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .masked(true)
            .placeholder(placeholder)
    })
}

impl AiSettings {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let endpoint =
            cx.new(|cx| InputState::new(window, cx).default_value("http://localhost:11434"));
        reset_on_change(&endpoint, OLLAMA, cx);
        let connections = (0..PROVIDERS.len())
            .map(|index| {
                let key = key_input("Add API key", window, cx);
                reset_on_change(&key, index, cx);
                Connection {
                    key,
                    tested: false,
                    added: false,
                    model: None,
                    message: "",
                }
            })
            .collect();
        Self {
            connections,
            provider: 0,
            endpoint,
        }
    }

    fn model_picker(&self, index: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let target = cx.entity();
        let selected = self.connections[index].model;
        let label = selected.map_or("Choose a model", |model| MODELS[index][model]);
        Button::new("ai-model")
            .outline()
            .label(label)
            .icon(IconName::ChevronDown)
            .dropdown_menu(move |mut menu, _, _| {
                for (model, name) in MODELS[index].iter().enumerate() {
                    let target = target.clone();
                    menu = menu.item(
                        PopupMenuItem::new(*name)
                            .checked(selected == Some(model))
                            .on_click(move |_, _, cx| {
                                target.update(cx, |this, cx| {
                                    this.connections[index].model = Some(model);
                                    cx.notify();
                                });
                            }),
                    );
                }
                menu
            })
    }
}

impl Render for AiSettings {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let index = self.provider;
        let local = index == OLLAMA;
        let connection = &self.connections[index];
        let ready = if local {
            !self.endpoint.read(cx).value().trim().is_empty()
        } else {
            !connection.key.read(cx).value().trim().is_empty() || connection.added
        };
        let models_ready = connection.models_ready();
        v_flex().gap_4()
            .child(v_flex().gap_2()
                .child(div().font_semibold().child("API providers"))
                .child(h_flex().flex_wrap().gap_2().children(PROVIDERS.iter().enumerate().map(|(index, name)| {
                    Button::new(*name).label(*name).selected(self.provider == index)
                        .on_click(cx.listener(move |this, _, _, cx| { this.provider = index; cx.notify(); }))
                }))))
            .child(v_flex().p_5().gap_4().rounded(cx.theme().radius_lg).bg(cx.theme().secondary)
                .child(div().font_semibold().child(PROVIDERS[index]))
                .when(local, |group| group.child("Server address").child(Input::new(&self.endpoint)))
                .when(!local, |group| group.child(if connection.added { "API key added · preview" } else { "API key" })
                    .child(Input::new(&connection.key)))
                .child(h_flex().gap_2()
                    .child(Button::new("test-ai").label(if local { "Test connection" } else { "Test key" })
                        .disabled(!ready).on_click(cx.listener(move |this, _, _, cx| {
                            let connection = &mut this.connections[index];
                            connection.tested = true;
                            connection.message = "Simulated test passed. No connection was made.";
                            cx.notify();
                        })))
                    .when(!local && models_ready, |row| row
                        .child(Button::new("add-ai-key").primary().label(if connection.added { "Replace key" } else { "Add key" })
                            .disabled(!connection.tested || connection.key.read(cx).value().trim().is_empty())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                // Replace the input entity so the mock does not retain the entered secret.
                                let key = key_input("Add a new API key", window, cx);
                                reset_on_change(&key, index, cx);
                                let connection = &mut this.connections[index];
                                connection.key = key;
                                connection.added = true;
                                connection.tested = false;
                                connection.message = "Key added in this preview only. Nothing was saved.";
                                cx.notify();
                            })))
                        .child(div().flex_1())
                        .when(connection.added, |row| row.child(Button::new("remove-ai-key").label("Remove key")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                let connection = &mut this.connections[index];
                                connection.key.update(cx, |input, cx| input.set_value("", window, cx));
                                connection.added = false;
                                connection.tested = false;
                                connection.model = None;
                                connection.message = "Preview key removed.";
                                cx.notify();
                            }))))))
                .when(!connection.message.is_empty(), |group| group.child(div().text_sm().child(connection.message)))
                .when(models_ready, |group| group.child("Model").child(h_flex().child(self.model_picker(index, cx)))))
    }
}
