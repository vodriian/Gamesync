//! UI-only connection preview. No credentials, network calls, or disk writes.
use gpui::{div, prelude::*, Entity, Window};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex, ActiveTheme as _, Disableable as _, Selectable as _, StyledExt as _,
};

const PROVIDERS: [&str; 5] = ["Ollama", "OpenAI", "Claude", "Grok", "Gemini"];

struct Connection {
    model: Entity<InputState>,
    key: Entity<InputState>,
    tested: bool,
    added: bool,
    message: &'static str,
}

pub struct AiSettings {
    connections: Vec<Connection>,
    provider: usize,
    endpoint: Entity<InputState>,
}

impl AiSettings {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let endpoint =
            cx.new(|cx| InputState::new(window, cx).default_value("http://localhost:11434"));
        cx.subscribe(&endpoint, |this, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                this.connections[0].tested = false;
                this.connections[0].message = "";
                cx.notify();
            }
        })
        .detach();
        let connections = (0..5)
            .map(|index| {
                let model =
                    cx.new(|cx| InputState::new(window, cx).placeholder("Enter model name"));
                let key = cx.new(|cx| {
                    InputState::new(window, cx)
                        .masked(true)
                        .placeholder("Enter a dummy API key")
                });
                for input in [&model, &key] {
                    cx.subscribe(input, move |this, _, event, cx| {
                        if matches!(event, InputEvent::Change) {
                            this.connections[index].tested = false;
                            this.connections[index].message = "";
                            cx.notify();
                        }
                    })
                    .detach();
                }
                Connection {
                    model,
                    key,
                    tested: false,
                    added: false,
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
}

impl Render for AiSettings {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let index = self.provider;
        let local = index == 0;
        let connection = &self.connections[index];
        let ready = !connection.model.read(cx).value().trim().is_empty()
            && if local {
                !self.endpoint.read(cx).value().trim().is_empty()
            } else {
                !connection.key.read(cx).value().trim().is_empty() || connection.added
            };
        v_flex().gap_4()
            .child(div().text_sm().text_color(cx.theme().muted_foreground)
                .child("Preview · tests are simulated. Use dummy keys; nothing is saved or sent."))
            .child(v_flex().gap_2()
                .child(div().font_semibold().child("Local models"))
                .child(Button::new("ollama-provider").label("Ollama").selected(local)
                    .on_click(cx.listener(|this, _, _, cx| { this.provider = 0; cx.notify(); }))))
            .child(v_flex().gap_2()
                .child(div().font_semibold().child("API providers"))
                .child(h_flex().flex_wrap().gap_2().children((1..5).map(|index| {
                    Button::new(PROVIDERS[index]).label(PROVIDERS[index]).selected(self.provider == index)
                        .on_click(cx.listener(move |this, _, _, cx| { this.provider = index; cx.notify(); }))
                }))))
            .child(v_flex().p_5().gap_4().rounded_lg().bg(cx.theme().secondary)
                .child(div().font_semibold().child(PROVIDERS[index]))
                .child("Model")
                .child(Input::new(&connection.model))
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
                    .when(!local, |row| row
                        .child(Button::new("add-ai-key").primary().label(if connection.added { "Replace key" } else { "Add key" })
                            .disabled(!connection.tested || connection.key.read(cx).value().trim().is_empty())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                // Replace the input entity so the mock does not retain the entered secret.
                                let key = cx.new(|cx| InputState::new(window, cx).masked(true).placeholder("Enter a dummy replacement key"));
                                cx.subscribe(&key, move |this, _, event, cx| {
                                    if matches!(event, InputEvent::Change) {
                                        this.connections[index].tested = false;
                                        this.connections[index].message = "";
                                        cx.notify();
                                    }
                                }).detach();
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
                                connection.message = "Preview key removed.";
                                cx.notify();
                            }))))))
                .when(!connection.message.is_empty(), |group| group.child(div().text_sm().child(connection.message))))
    }
}
