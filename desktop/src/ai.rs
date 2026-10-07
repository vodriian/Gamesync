//! AI provider requests. Two wire formats cover every supported provider:
//! Anthropic Messages for Claude, and OpenAI-compatible chat completions for
//! OpenAI, Grok, Gemini, and Ollama. This module returns model text only.
//! Callers own prompts, validate every result, and control all record writes.
use crate::credentials::{CredentialStore as _, OsCredential};
use anyhow::{bail, ensure, Context as _, Result};
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wire {
    Anthropic,
    OpenAi,
}

pub struct ProviderInfo {
    /// Stable id for settings and the credential entry. Never rename.
    pub id: &'static str,
    pub name: &'static str,
    pub wire: Wire,
    /// Fixed API base. None for a local server at a user-entered address.
    pub base: Option<&'static str>,
    /// Where to get a key or the server, as (label, URL).
    pub help: (&'static str, &'static str),
}
impl ProviderInfo {
    /// Local servers use an address instead of a key. Their requests stay local.
    pub fn local(&self) -> bool {
        self.base.is_none()
    }
}

pub const PROVIDERS: [ProviderInfo; 5] = [
    ProviderInfo {
        id: "openai",
        name: "OpenAI",
        wire: Wire::OpenAi,
        base: Some("https://api.openai.com/v1"),
        help: (
            "platform.openai.com",
            "https://platform.openai.com/api-keys",
        ),
    },
    ProviderInfo {
        id: "claude",
        name: "Claude",
        wire: Wire::Anthropic,
        base: Some("https://api.anthropic.com/v1"),
        help: (
            "console.anthropic.com",
            "https://console.anthropic.com/settings/keys",
        ),
    },
    ProviderInfo {
        id: "grok",
        name: "Grok",
        wire: Wire::OpenAi,
        base: Some("https://api.x.ai/v1"),
        help: ("console.x.ai", "https://console.x.ai"),
    },
    ProviderInfo {
        id: "gemini",
        name: "Gemini",
        wire: Wire::OpenAi,
        base: Some("https://generativelanguage.googleapis.com/v1beta/openai"),
        help: ("aistudio.google.com", "https://aistudio.google.com/apikey"),
    },
    ProviderInfo {
        id: "ollama",
        name: "Ollama",
        wire: Wire::OpenAi,
        base: None,
        help: ("ollama.com", "https://ollama.com/download"),
    },
];
pub const OLLAMA_ADDRESS: &str = "http://localhost:11434";

pub fn provider(id: &str) -> Option<&'static ProviderInfo> {
    PROVIDERS.iter().find(|p| p.id == id)
}

/// One provider with its key or local address. Secrets never implement Debug.
pub struct Connection {
    pub info: &'static ProviderInfo,
    base: String,
    key: Option<String>,
}

impl Connection {
    /// `endpoint` is the local server address; `key` is required for cloud providers.
    pub fn new(id: &str, endpoint: Option<&str>, key: Option<String>) -> Result<Self> {
        let info = provider(id).context("Unknown AI provider")?;
        let base = match info.base {
            Some(base) => base.to_owned(),
            None => local_base(endpoint.unwrap_or(OLLAMA_ADDRESS))?,
        };
        let key = key.map(|k| k.trim().to_owned()).filter(|k| !k.is_empty());
        ensure!(info.local() || key.is_some(), "Add an API key first");
        Ok(Self { info, base, key })
    }

    /// Read the saved key from secure storage. This can show an OS prompt.
    pub fn saved(id: &str, entry: &crate::settings::AiProvider) -> Result<Self> {
        let info = provider(id).context("Unknown AI provider")?;
        let key = if info.local() {
            None
        } else {
            Some(
                OsCredential::ai(id)?
                    .get()?
                    .context("The saved key is missing. Add it again in Settings → AI.")?,
            )
        };
        Self::new(id, entry.endpoint.as_deref(), key)
    }

    fn client(timeout: Duration) -> Result<reqwest::blocking::Client> {
        Ok(reqwest::blocking::Client::builder()
            .user_agent("GameSync (personal game library)")
            .connect_timeout(Duration::from_secs(10))
            .timeout(timeout)
            .build()?)
    }

    fn authorize(
        &self,
        request: reqwest::blocking::RequestBuilder,
    ) -> reqwest::blocking::RequestBuilder {
        match (self.info.wire, &self.key) {
            (Wire::Anthropic, Some(key)) => request
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01"),
            (Wire::OpenAi, Some(key)) => request.bearer_auth(key),
            (_, None) => request,
        }
    }

    fn send(&self, request: reqwest::blocking::RequestBuilder) -> Result<Value> {
        let response = self.authorize(request).send().map_err(|error| {
            if error.is_timeout() {
                anyhow::anyhow!("{} did not answer in time", self.info.name)
            } else if self.info.local() {
                anyhow::anyhow!(
                    "Could not reach {}. Check that it is running.",
                    self.info.name
                )
            } else {
                anyhow::anyhow!("Could not reach {}. Check the connection.", self.info.name)
            }
        })?;
        let status = response.status();
        let body = response
            .text()
            .with_context(|| format!("{} sent an unreadable response", self.info.name))?;
        if !status.is_success() {
            bail!("{}", error_message(self.info.name, status.as_u16(), &body));
        }
        serde_json::from_str(&body)
            .with_context(|| format!("{} sent a response that is not JSON", self.info.name))
    }

    /// List chat models. A successful list is also the key or connection test.
    pub fn models(&self) -> Result<Vec<String>> {
        let url = match self.info.wire {
            Wire::Anthropic => format!("{}/models?limit=1000", self.base),
            Wire::OpenAi => format!("{}/models", self.base),
        };
        let body = self.send(Self::client(Duration::from_secs(20))?.get(url))?;
        let models = parse_models(self.info.wire, &body);
        ensure!(
            !models.is_empty(),
            "{} has no usable models for this key",
            self.info.name
        );
        Ok(models)
    }

    /// One request constrained to `schema`. Returns the text for the caller to
    /// validate; a schema-valid answer is still only a suggestion.
    pub fn complete_json(
        &self,
        model: &str,
        system: &str,
        user: &str,
        schema_name: &str,
        schema: &Value,
    ) -> Result<String> {
        // Local models on modest hardware can take minutes for one batch.
        let timeout = Duration::from_secs(if self.info.local() { 600 } else { 180 });
        let client = Self::client(timeout)?;
        match self.info.wire {
            Wire::Anthropic => {
                let mut body = json!({
                    "model": model,
                    "max_tokens": 16000,
                    "system": system,
                    "messages": [{"role": "user", "content": user}],
                    "output_config": {"format": {"type": "json_schema", "schema": schema}},
                });
                let mut request = client.post(format!("{}/messages", self.base));
                // These models can decline on a safety classifier; the server then
                // retries on Anthropic's recommended model in the same request.
                if FALLBACK_MODELS.contains(&model) {
                    body["fallbacks"] = json!("default");
                    request = request.header("anthropic-beta", "server-side-fallback-2026-07-01");
                }
                anthropic_text(self.info.name, &self.send(request.json(&body))?)
            }
            Wire::OpenAi => {
                let body = json!({
                    "model": model,
                    "messages": [
                        {"role": "system", "content": system},
                        {"role": "user", "content": user},
                    ],
                    "response_format": {
                        "type": "json_schema",
                        "json_schema": {"name": schema_name, "strict": true, "schema": schema},
                    },
                });
                let request = client
                    .post(format!("{}/chat/completions", self.base))
                    .json(&body);
                openai_text(self.info.name, &self.send(request)?)
            }
        }
    }
}

const FALLBACK_MODELS: [&str; 4] = [
    "claude-fable-5-1",
    "claude-opus-5-5",
    "claude-opus-5",
    "claude-sonnet-5-5",
];

/// Accept `host:port`, a full URL, or a URL that already ends in `/v1`.
fn local_base(address: &str) -> Result<String> {
    let address = address.trim().trim_end_matches('/');
    ensure!(!address.is_empty(), "Enter the server address");
    let address = if address.contains("://") {
        address.to_owned()
    } else {
        format!("http://{address}")
    };
    let url = reqwest::Url::parse(&address).context("Enter a valid server address")?;
    ensure!(
        matches!(url.scheme(), "http" | "https") && url.host().is_some(),
        "Enter an http or https server address"
    );
    let address = address.trim_end_matches('/');
    Ok(if address.ends_with("/v1") {
        address.to_owned()
    } else {
        format!("{address}/v1")
    })
}

/// Model lists include embedding, image, and audio models; keep chat models.
fn parse_models(wire: Wire, body: &Value) -> Vec<String> {
    const NOT_CHAT: [&str; 12] = [
        "embed",
        "tts",
        "whisper",
        "dall-e",
        "image",
        "audio",
        "realtime",
        "moderation",
        "transcribe",
        "imagen",
        "veo",
        "aqa",
    ];
    let mut models: Vec<String> = body["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| {
            // Anthropic publishes capabilities; skip models that cannot return a schema.
            wire != Wire::Anthropic
                || m["capabilities"]["structured_outputs"]["supported"].as_bool() != Some(false)
        })
        .filter_map(|m| m["id"].as_str())
        .map(|id| id.strip_prefix("models/").unwrap_or(id).to_owned())
        .filter(|id| {
            let lower = id.to_lowercase();
            !id.is_empty() && !NOT_CHAT.iter().any(|word| lower.contains(word))
        })
        .collect();
    if wire == Wire::OpenAi {
        models.sort();
    }
    models.dedup();
    models
}

fn anthropic_text(name: &str, body: &Value) -> Result<String> {
    match body["stop_reason"].as_str() {
        Some("refusal") => bail!("{name} declined this request"),
        Some("max_tokens") => bail!("{name} stopped before the answer was complete"),
        _ => {}
    }
    let text: String = body["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|block| block["type"] == "text")
        .filter_map(|block| block["text"].as_str())
        .collect();
    ensure!(!text.trim().is_empty(), "{name} sent an empty answer");
    Ok(text)
}

fn openai_text(name: &str, body: &Value) -> Result<String> {
    let choice = &body["choices"][0];
    if choice["message"]["refusal"]
        .as_str()
        .is_some_and(|r| !r.is_empty())
    {
        bail!("{name} declined this request");
    }
    if choice["finish_reason"] == "length" {
        bail!("{name} stopped before the answer was complete");
    }
    let text = choice["message"]["content"].as_str().unwrap_or_default();
    ensure!(!text.trim().is_empty(), "{name} sent an empty answer");
    Ok(text.to_owned())
}

/// Short, actionable errors. Provider text is bounded and never contains our key.
fn error_message(name: &str, status: u16, body: &str) -> String {
    let detail = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            v["error"]["message"]
                .as_str()
                .or_else(|| v["error"].as_str())
                .or_else(|| v[0]["error"]["message"].as_str())
                .map(str::to_owned)
        })
        .map(|d| d.chars().take(200).collect::<String>());
    let base = match status {
        401 | 403 => format!("{name} rejected the key"),
        404 => format!("{name} does not have this model or address"),
        429 => format!("{name} rate limit reached. Wait, then retry"),
        500..=599 => format!("{name} had a server error. Retry later"),
        _ => format!("{name} rejected the request ({status})"),
    };
    match detail {
        Some(detail) if !detail.is_empty() => format!("{base}: {detail}"),
        _ => format!("{base}."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead as _, BufReader, Read as _, Write as _};

    /// One captured HTTP request: request line, lowercase headers, and body.
    struct Seen {
        line: String,
        headers: Vec<(String, String)>,
        body: String,
    }
    impl Seen {
        fn header(&self, name: &str) -> Option<&str> {
            self.headers
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.as_str())
        }
        fn json(&self) -> Value {
            serde_json::from_str(&self.body).unwrap()
        }
    }

    /// A local server that answers one request with `status` and `body`.
    fn serve_once(status: u16, body: &str) -> (String, std::thread::JoinHandle<Seen>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let body = body.to_owned();
        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let mut headers = Vec::new();
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                let header = header.trim_end();
                if header.is_empty() {
                    break;
                }
                let (name, value) = header.split_once(':').unwrap();
                headers.push((name.to_lowercase(), value.trim().to_owned()));
            }
            let length = headers
                .iter()
                .find(|(n, _)| n == "content-length")
                .map_or(0, |(_, v)| v.parse().unwrap());
            let mut request_body = vec![0; length];
            reader.read_exact(&mut request_body).unwrap();
            let mut stream = stream;
            write!(
                stream,
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            Seen {
                line: line.trim_end().to_owned(),
                headers,
                body: String::from_utf8(request_body).unwrap(),
            }
        });
        (address, handle)
    }

    fn with_base(id: &str, base: String, key: Option<&str>) -> Connection {
        let mut connection = Connection::new(id, None, key.map(str::to_owned)).unwrap();
        connection.base = base;
        connection
    }

    #[test]
    fn local_addresses_get_one_v1_suffix() {
        assert_eq!(
            local_base("http://localhost:11434/").unwrap(),
            "http://localhost:11434/v1"
        );
        assert_eq!(
            local_base("192.168.1.5:11434").unwrap(),
            "http://192.168.1.5:11434/v1"
        );
        assert_eq!(
            local_base("https://box.lan/ollama/v1").unwrap(),
            "https://box.lan/ollama/v1"
        );
        assert!(local_base("ftp://box").is_err());
        assert!(local_base(" ").is_err());
    }

    #[test]
    fn cloud_providers_need_a_key() {
        assert!(Connection::new("claude", None, None).is_err());
        assert!(Connection::new("claude", None, Some("  ".into())).is_err());
        assert!(Connection::new("ollama", None, None).is_ok());
        assert!(Connection::new("other", None, Some("k".into())).is_err());
    }

    #[test]
    fn model_lists_keep_chat_models() {
        let openai = json!({"data": [
            {"id": "gpt-5"}, {"id": "text-embedding-3-small"}, {"id": "gpt-4o-mini-tts"},
            {"id": "gpt-5-mini"}, {"id": "dall-e-3"}
        ]});
        assert_eq!(parse_models(Wire::OpenAi, &openai), ["gpt-5", "gpt-5-mini"]);
        let gemini = json!({"data": [{"id": "models/gemini-2.5-flash"}, {"id": "models/text-embedding-004"}]});
        assert_eq!(parse_models(Wire::OpenAi, &gemini), ["gemini-2.5-flash"]);
        // Anthropic order is newest first; keep it.
        let claude = json!({"data": [
            {"id": "claude-opus-5-5", "capabilities": {"structured_outputs": {"supported": true}}},
            {"id": "claude-old", "capabilities": {"structured_outputs": {"supported": false}}},
            {"id": "claude-haiku-4-5"}
        ]});
        assert_eq!(
            parse_models(Wire::Anthropic, &claude),
            ["claude-opus-5-5", "claude-haiku-4-5"]
        );
        assert!(parse_models(Wire::OpenAi, &json!({"error": "x"})).is_empty());
    }

    #[test]
    fn anthropic_answers_use_text_blocks_and_stop_reasons() {
        let ok = json!({"stop_reason": "end_turn", "content": [
            {"type": "thinking", "thinking": ""},
            {"type": "text", "text": "{\"games\":"}, {"type": "text", "text": "[]}"}
        ]});
        assert_eq!(anthropic_text("Claude", &ok).unwrap(), "{\"games\":[]}");
        for reason in ["refusal", "max_tokens"] {
            let body = json!({"stop_reason": reason, "content": [{"type": "text", "text": "{}"}]});
            assert!(anthropic_text("Claude", &body).is_err());
        }
        assert!(anthropic_text("Claude", &json!({"content": []})).is_err());
    }

    #[test]
    fn openai_answers_check_refusal_and_length() {
        let ok = json!({"choices": [{"finish_reason": "stop", "message": {"content": "{}"}}]});
        assert_eq!(openai_text("OpenAI", &ok).unwrap(), "{}");
        let cut = json!({"choices": [{"finish_reason": "length", "message": {"content": "{"}}]});
        assert!(openai_text("OpenAI", &cut).is_err());
        let refused = json!({"choices": [{"message": {"content": null, "refusal": "No"}}]});
        assert!(openai_text("OpenAI", &refused).is_err());
    }

    #[test]
    fn errors_are_short_and_specific() {
        let body = r#"{"error":{"type":"authentication_error","message":"invalid x-api-key"}}"#;
        assert_eq!(
            error_message("Claude", 401, body),
            "Claude rejected the key: invalid x-api-key"
        );
        assert_eq!(
            error_message("Gemini", 429, r#"[{"error":{"message":"Quota"}}]"#),
            "Gemini rate limit reached. Wait, then retry: Quota"
        );
        assert_eq!(
            error_message("Ollama", 502, "<html>"),
            "Ollama had a server error. Retry later."
        );
    }

    #[test]
    fn anthropic_requests_use_messages_schema_and_key_header() {
        let answer = json!({"stop_reason": "end_turn",
            "content": [{"type": "text", "text": "{\"games\":[]}"}]});
        let (address, server) = serve_once(200, &answer.to_string());
        let connection = with_base("claude", format!("{address}/v1"), Some("sk-test"));
        let schema = json!({"type": "object"});
        let text = connection
            .complete_json("claude-opus-5-5", "system text", "user text", "s", &schema)
            .unwrap();
        assert_eq!(text, "{\"games\":[]}");
        let seen = server.join().unwrap();
        assert_eq!(seen.line, "POST /v1/messages HTTP/1.1");
        assert_eq!(seen.header("x-api-key"), Some("sk-test"));
        assert_eq!(seen.header("anthropic-version"), Some("2023-06-01"));
        assert_eq!(
            seen.header("anthropic-beta"),
            Some("server-side-fallback-2026-07-01")
        );
        assert_eq!(seen.header("authorization"), None);
        let body = seen.json();
        assert_eq!(body["model"], "claude-opus-5-5");
        assert_eq!(body["system"], "system text");
        assert_eq!(
            body["messages"][0],
            json!({"role": "user", "content": "user text"})
        );
        assert_eq!(
            body["output_config"]["format"],
            json!({"type": "json_schema", "schema": schema})
        );
        assert_eq!(body["fallbacks"], "default");
        assert!(body.get("thinking").is_none() && body.get("temperature").is_none());
    }

    #[test]
    fn anthropic_fallbacks_are_only_sent_for_models_that_support_them() {
        let answer =
            json!({"stop_reason": "end_turn", "content": [{"type": "text", "text": "{}"}]});
        let (address, server) = serve_once(200, &answer.to_string());
        let connection = with_base("claude", format!("{address}/v1"), Some("k"));
        connection
            .complete_json("claude-haiku-4-5", "s", "u", "n", &json!({}))
            .unwrap();
        let seen = server.join().unwrap();
        assert_eq!(seen.header("anthropic-beta"), None);
        assert!(seen.json().get("fallbacks").is_none());
    }

    #[test]
    fn anthropic_model_list_is_the_key_test() {
        let list = json!({"data": [{"id": "claude-opus-5-5"}]});
        let (address, server) = serve_once(200, &list.to_string());
        let connection = with_base("claude", format!("{address}/v1"), Some("k"));
        assert_eq!(connection.models().unwrap(), ["claude-opus-5-5"]);
        let seen = server.join().unwrap();
        assert_eq!(seen.line, "GET /v1/models?limit=1000 HTTP/1.1");
        assert_eq!(seen.header("x-api-key"), Some("k"));
    }

    #[test]
    fn openai_compatible_requests_use_bearer_and_strict_schema() {
        let answer = json!({"choices": [{"finish_reason": "stop", "message": {"content": "{}"}}]});
        let (address, server) = serve_once(200, &answer.to_string());
        let connection = with_base("openai", format!("{address}/v1"), Some("sk-o"));
        let schema = json!({"type": "object"});
        connection
            .complete_json("gpt-5", "sys", "usr", "play_now_profiles", &schema)
            .unwrap();
        let seen = server.join().unwrap();
        assert_eq!(seen.line, "POST /v1/chat/completions HTTP/1.1");
        assert_eq!(seen.header("authorization"), Some("Bearer sk-o"));
        assert_eq!(seen.header("x-api-key"), None);
        let body = seen.json();
        assert_eq!(
            body["messages"][0],
            json!({"role": "system", "content": "sys"})
        );
        assert_eq!(
            body["messages"][1],
            json!({"role": "user", "content": "usr"})
        );
        assert_eq!(
            body["response_format"],
            json!({"type": "json_schema", "json_schema":
                {"name": "play_now_profiles", "strict": true, "schema": schema}})
        );
        // Newer OpenAI models reject max_tokens; no limit field is sent.
        assert!(body.get("max_tokens").is_none());
    }

    #[test]
    fn ollama_sends_no_auth_to_the_entered_address() {
        let list = json!({"data": [{"id": "gemma4"}, {"id": "nomic-embed-text"}]});
        let (address, server) = serve_once(200, &list.to_string());
        let connection = Connection::new("ollama", Some(&address), None).unwrap();
        assert_eq!(connection.models().unwrap(), ["gemma4"]);
        let seen = server.join().unwrap();
        assert_eq!(seen.line, "GET /v1/models HTTP/1.1");
        assert_eq!(seen.header("authorization"), None);
    }

    #[test]
    fn http_errors_reach_the_caller_without_the_key() {
        let error = json!({"error": {"message": "invalid key"}});
        let (address, server) = serve_once(401, &error.to_string());
        let connection = with_base("grok", format!("{address}/v1"), Some("secret-key"));
        let message = format!("{:#}", connection.models().unwrap_err());
        server.join().unwrap();
        assert_eq!(message, "Grok rejected the key: invalid key");
        assert!(!message.contains("secret-key"));
    }

    #[test]
    fn a_closed_local_server_gives_a_running_hint() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        let connection = Connection::new("ollama", Some(&address), None).unwrap();
        let message = format!("{:#}", connection.models().unwrap_err());
        assert_eq!(message, "Could not reach Ollama. Check that it is running.");
    }
}
