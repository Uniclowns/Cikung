//! Explicit, user-triggered cloud requests. Secrets live only in the OS credential store.
use reqwest::blocking::{Client, RequestBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::Read,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    time::Duration,
};

const MAX_RESPONSE: u64 = 2 * 1024 * 1024;
const MAX_AUDIO: u64 = 25_000_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatProvider {
    #[default]
    OpenAi,
    Anthropic,
    Gemini,
    DeepSeek,
    Qwen,
    Moonshot,
    Ollama,
    LmStudio,
}
impl ChatProvider {
    pub const ALL: [Self; 8] = [
        Self::OpenAi,
        Self::Anthropic,
        Self::Gemini,
        Self::DeepSeek,
        Self::Qwen,
        Self::Moonshot,
        Self::Ollama,
        Self::LmStudio,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::Gemini => "Gemini",
            Self::DeepSeek => "DeepSeek",
            Self::Qwen => "Qwen",
            Self::Moonshot => "Moonshot / Kimi",
            Self::Ollama => "Ollama",
            Self::LmStudio => "LM Studio",
        }
    }
    pub fn secret_name(self) -> &'static str {
        match self {
            Self::OpenAi => "chat-openai",
            Self::Anthropic => "chat-anthropic",
            Self::Gemini => "chat-gemini",
            Self::DeepSeek => "chat-deepseek",
            Self::Qwen => "chat-qwen",
            Self::Moonshot => "chat-moonshot",
            Self::Ollama => "chat-ollama",
            Self::LmStudio => "chat-lmstudio",
        }
    }
    pub fn default_base_url(self) -> &'static str {
        match self {
            Self::OpenAi => "https://api.openai.com/v1",
            Self::Anthropic => "https://api.anthropic.com/v1",
            Self::Gemini => "https://generativelanguage.googleapis.com/v1beta/openai",
            Self::DeepSeek => "https://api.deepseek.com",
            Self::Qwen => "https://dashscope-intl.aliyuncs.com/compatible-mode/v1",
            Self::Moonshot => "https://api.moonshot.ai/v1",
            Self::Ollama => "http://127.0.0.1:11434/v1",
            Self::LmStudio => "http://127.0.0.1:1234/v1",
        }
    }
    pub fn default_model(self) -> &'static str {
        match self {
            Self::OpenAi => "gpt-4.1-mini",
            Self::Anthropic => "claude-sonnet-4-5",
            Self::Gemini => "gemini-2.5-flash",
            Self::DeepSeek => "deepseek-flash",
            Self::Qwen => "qwen-plus",
            Self::Moonshot => "kimi-k3",
            Self::Ollama => "llama3.2",
            Self::LmStudio => "local-model",
        }
    }
    pub fn is_local(self) -> bool {
        matches!(self, Self::Ollama | Self::LmStudio)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ChatConfig {
    pub provider: ChatProvider,
    pub model: String,
    pub base_url: String,
}
impl ChatConfig {
    pub fn for_provider(provider: ChatProvider) -> Self {
        Self {
            provider,
            model: provider.default_model().into(),
            base_url: provider.default_base_url().into(),
        }
    }
}
impl Default for ChatConfig {
    fn default() -> Self {
        Self::for_provider(ChatProvider::OpenAi)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Service {
    #[default]
    GitHub,
    Stripe,
    Vercel,
    Resend,
    Notion,
    CalCom,
    N8n,
}
impl Service {
    pub const ALL: [Self; 7] = [
        Self::GitHub,
        Self::Stripe,
        Self::Vercel,
        Self::Resend,
        Self::Notion,
        Self::CalCom,
        Self::N8n,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::GitHub => "GitHub",
            Self::Stripe => "Stripe",
            Self::Vercel => "Vercel",
            Self::Resend => "Resend",
            Self::Notion => "Notion",
            Self::CalCom => "Cal.com",
            Self::N8n => "n8n",
        }
    }
    pub fn secret_name(self) -> &'static str {
        match self {
            Self::GitHub => "service-github",
            Self::Stripe => "service-stripe",
            Self::Vercel => "service-vercel",
            Self::Resend => "service-resend",
            Self::Notion => "service-notion",
            Self::CalCom => "service-calcom",
            Self::N8n => "service-n8n",
        }
    }
    pub fn scope_hint(self) -> &'static str {
        match self {
            Self::GitHub => "Repository: owner/repo (optional)",
            Self::Vercel => "Team ID (optional)",
            Self::Notion => "Page title search (optional)",
            Self::CalCom => "Event type ID (optional)",
            Self::N8n => "Project ID (optional)",
            _ => "No extra scope required",
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ServiceConfig {
    pub service: Service,
    pub base_url: String,
    pub repository: String,
    pub scope: String,
}
#[derive(Clone, Debug)]
pub struct ServiceCard {
    pub title: String,
    pub detail: String,
    pub url: String,
}

pub enum CloudEvent {
    Chat {
        request_id: u64,
        result: Result<String, String>,
    },
    Service {
        request_id: u64,
        service: Service,
        result: Result<Vec<ServiceCard>, String>,
    },
    Lyrics {
        request_id: u64,
        result: Result<String, String>,
    },
    Transcription {
        request_id: u64,
        result: Result<String, String>,
    },
}

pub struct CloudClient {
    tx: Sender<CloudEvent>,
    rx: Receiver<CloudEvent>,
    active: Arc<AtomicUsize>,
}
impl Default for CloudClient {
    fn default() -> Self {
        Self::new()
    }
}
impl CloudClient {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx,
            active: Arc::new(AtomicUsize::new(0)),
        }
    }
    pub fn poll(&self) -> Vec<CloudEvent> {
        self.rx.try_iter().collect()
    }
    #[cfg(test)]
    pub(crate) fn inject_event(&self, event: CloudEvent) {
        self.tx.send(event).unwrap();
    }
    #[allow(deprecated)] // fetch_update preserves the supported Rust 1.88 baseline.
    fn launch(&self, job: impl FnOnce() -> CloudEvent + Send + 'static) -> Result<(), String> {
        if self
            .active
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                if n < 4 { Some(n + 1) } else { None }
            })
            .is_err()
        {
            return Err("Four requests are already running. Wait for one to finish.".into());
        }
        let tx = self.tx.clone();
        let active = self.active.clone();
        if std::thread::Builder::new()
            .name("cikung-cloud".into())
            .spawn(move || {
                struct Guard(Arc<AtomicUsize>);
                impl Drop for Guard {
                    fn drop(&mut self) {
                        self.0.fetch_sub(1, Ordering::Release);
                    }
                }
                let _guard = Guard(active);
                let _ = tx.send(job());
            })
            .is_err()
        {
            self.active.fetch_sub(1, Ordering::Release);
            return Err("Could not start the background request.".into());
        }
        Ok(())
    }
    pub fn send_chat(
        &self,
        request_id: u64,
        config: ChatConfig,
        messages: Vec<ChatMessage>,
    ) -> Result<(), String> {
        self.launch(move || CloudEvent::Chat {
            request_id,
            result: chat(&config, &messages),
        })
    }
    pub fn refresh_service(&self, request_id: u64, config: ServiceConfig) -> Result<(), String> {
        self.launch(move || CloudEvent::Service {
            request_id,
            service: config.service,
            result: service_summary(&config),
        })
    }
    pub fn lookup_lyrics(
        &self,
        request_id: u64,
        title: String,
        artist: String,
        duration: f64,
    ) -> Result<(), String> {
        self.launch(move || CloudEvent::Lyrics {
            request_id,
            result: lookup_lyrics(&title, &artist, duration),
        })
    }
    /// Uploads only when explicitly invoked by the UI. Never called automatically.
    pub fn transcribe_audio(&self, request_id: u64, path: PathBuf) -> Result<(), String> {
        self.launch(move || CloudEvent::Transcription {
            request_id,
            result: transcribe(&path),
        })
    }
}

fn credential(name: &str) -> Result<keyring::Entry, String> {
    if name.is_empty()
        || name.len() > 100
        || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
    {
        return Err("Invalid credential name.".into());
    }
    keyring::Entry::new("Cikung", name)
        .map_err(|_| "The system credential store is unavailable.".into())
}
pub fn save_secret(name: &str, secret: &str) -> Result<(), String> {
    let secret = secret.trim();
    if secret.is_empty() {
        return Err("Enter a key before saving.".into());
    }
    if secret.len() > 16000 {
        return Err("The key is too long.".into());
    }
    credential(name)?
        .set_password(secret)
        .map_err(|_| "Could not save the key in the system credential store.".into())
}
pub fn read_secret(name: &str) -> Result<Option<String>, String> {
    match credential(name)?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => {
            Err("Could not read the system credential store. Unlock it and try again.".into())
        }
    }
}
pub fn remove_secret(name: &str) -> Result<(), String> {
    match credential(name)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err("Could not remove the saved key from the system credential store.".into()),
    }
}
fn required_secret(name: &str) -> Result<String, String> {
    read_secret(name)?
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Not connected. Save this provider's API key in Settings first.".into())
}

fn http() -> Result<Client, String> {
    Client::builder()
        .user_agent(concat!("Cikung/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(90))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Could not initialize the network client.".into())
}

/// Plain HTTP is restricted to loopback, including IPv6 loopback; no URL credentials.
pub fn validate_base_url(input: &str) -> Result<reqwest::Url, String> {
    let mut url = reqwest::Url::parse(input.trim())
        .map_err(|_| "Enter a valid absolute server URL.".to_string())?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Server URL must not contain credentials, a query, or a fragment.".into());
    }
    let loopback = url
        .host_str()
        .map(|host| {
            host.eq_ignore_ascii_case("localhost")
                || host
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        })
        .unwrap_or(false);
    if url.scheme() != "https" && !(url.scheme() == "http" && loopback) {
        return Err("Use HTTPS, or HTTP on localhost / a loopback IP only.".into());
    }
    if url.host().is_none() {
        return Err("Server URL needs a hostname.".into());
    }
    let path = url.path().trim_end_matches('/').to_owned();
    url.set_path(&path);
    Ok(url)
}
fn endpoint(base: &str, path: &str) -> Result<String, String> {
    let url = validate_base_url(base)?;
    Ok(format!(
        "{}/{}",
        url.as_str().trim_end_matches('/'),
        path.trim_start_matches('/')
    ))
}
fn json_response(request: RequestBuilder) -> Result<Value, String> {
    let response = request.send().map_err(|e| {
        if e.is_timeout() {
            "Request timed out. Try again.".to_string()
        } else {
            "Could not reach the provider. Check the connection and server address.".to_string()
        }
    })?;
    let status = response.status();
    if !status.is_success() {
        return Err(match status.as_u16() {
            401 => "Authentication failed. Check the saved API key.".into(),
            403 => "Access denied. Check the key's read permissions and account scope.".into(),
            404 => "Resource or model not found. Check the configuration.".into(),
            429 => {
                let retry = response
                    .headers()
                    .get("retry-after")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok());
                match retry {
                    Some(n) => {
                        format!("Rate limit reached. Wait at least {n} seconds before retrying.")
                    }
                    None => "Rate limit reached. Wait before retrying.".into(),
                }
            }
            _ => format!(
                "Provider returned HTTP {}. Try again or check configuration.",
                status.as_u16()
            ),
        });
    }
    let mut bytes = Vec::new();
    response
        .take(MAX_RESPONSE + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not read the provider's response.".to_string())?;
    if bytes.len() as u64 > MAX_RESPONSE {
        return Err("Provider response exceeded the size limit.".into());
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| "Provider returned an unexpected response format.".into())
}

fn chat(config: &ChatConfig, messages: &[ChatMessage]) -> Result<String, String> {
    let key = if config.provider.is_local() {
        read_secret(config.provider.secret_name())?
    } else {
        Some(required_secret(config.provider.secret_name())?)
    };
    chat_with_key(config, messages, key.as_deref())
}
fn chat_with_key(
    config: &ChatConfig,
    messages: &[ChatMessage],
    key: Option<&str>,
) -> Result<String, String> {
    if config.model.trim().is_empty() {
        return Err("Enter the provider's model ID in Settings.".into());
    }
    if messages.is_empty()
        || messages.len() > 100
        || messages.iter().map(|m| m.content.len()).sum::<usize>() > 250_000
    {
        return Err(
            "Chat is empty or exceeds the conversation size limit. Start a new chat.".into(),
        );
    }
    if messages
        .iter()
        .any(|m| !matches!(m.role.as_str(), "system" | "user" | "assistant"))
    {
        return Err("Unsupported chat message role.".into());
    }
    let base = if config.base_url.trim().is_empty() {
        config.provider.default_base_url()
    } else {
        config.base_url.trim()
    };
    let client = http()?;
    let value = if config.provider == ChatProvider::Anthropic {
        let system = messages
            .iter()
            .filter(|m| m.role == "system")
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let conversation = messages
            .iter()
            .filter(|m| m.role != "system")
            .collect::<Vec<_>>();
        if conversation.is_empty() {
            return Err("Add a user message before sending.".into());
        }
        json_response(client.post(endpoint(base,"messages")?).header("x-api-key",key.unwrap_or("")).header("anthropic-version","2023-06-01").json(&json!({"model":config.model,"max_tokens":4096,"system":system,"messages":conversation})))?
    } else {
        let mut request = client.post(endpoint(base, "chat/completions")?);
        if let Some(key) = key.filter(|s| !s.is_empty()) {
            request = request.bearer_auth(key)
        }
        let mut body = json!({"model":config.model,"messages":messages,"stream":false});
        if config.provider == ChatProvider::OpenAi {
            body["store"] = json!(false);
        }
        json_response(request.json(&body))?
    };
    let text = if config.provider == ChatProvider::Anthropic {
        value["content"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter(|b| b["type"] == "text")
                    .filter_map(|b| b["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default()
    } else {
        value["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    };
    if text.trim().is_empty() {
        Err("The model returned no text. Check the model's supported API format.".into())
    } else {
        Ok(text)
    }
}

fn card(
    title: impl Into<String>,
    detail: impl Into<String>,
    url: impl Into<String>,
) -> ServiceCard {
    ServiceCard {
        title: title.into(),
        detail: detail.into(),
        url: safe_link(&url.into()),
    }
}
fn safe_link(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .filter(|u| u.scheme() == "https" && u.username().is_empty() && u.password().is_none())
        .map(|u| u.into())
        .unwrap_or_default()
}
fn field<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or("")
}
fn rows<'a>(v: &'a Value, key: &str) -> Result<&'a [Value], String> {
    let value = if key.is_empty() { v } else { &v[key] };
    value
        .as_array()
        .map(|a| a.as_slice())
        .ok_or_else(|| "The provider returned an unexpected list format.".into())
}
fn service_summary(config: &ServiceConfig) -> Result<Vec<ServiceCard>, String> {
    let key = required_secret(config.service.secret_name())?;
    service_with_key(config, &key, None)
}
fn service_with_key(
    config: &ServiceConfig,
    key: &str,
    test_base: Option<&str>,
) -> Result<Vec<ServiceCard>, String> {
    let client = http()?;
    let mut cards = Vec::new();
    let scope = config.scope.trim();
    let base = match config.service {
        Service::GitHub => "https://api.github.com",
        Service::Stripe => "https://api.stripe.com",
        Service::Vercel => "https://api.vercel.com",
        Service::Resend => "https://api.resend.com",
        Service::Notion => "https://api.notion.com",
        Service::CalCom => "https://api.cal.com",
        Service::N8n => {
            if config.base_url.trim().is_empty() {
                return Err("Enter your n8n instance URL in Settings.".into());
            }
            config.base_url.trim()
        }
    };
    // Test override is private; service tokens always go to official service hosts (or configured n8n).
    let base = test_base.unwrap_or(base);
    let get = |path: &str| -> Result<Value, String> {
        json_response(client.get(endpoint(base, path)?).bearer_auth(key))
    };
    match config.service {
        Service::GitHub => {
            let user = json_response(
                client
                    .get(endpoint(base, "user")?)
                    .bearer_auth(key)
                    .header("Accept", "application/vnd.github+json")
                    .header("X-GitHub-Api-Version", "2026-03-10"),
            )?;
            if field(&user, "login").is_empty() {
                return Err("GitHub did not return an account.".into());
            }
            cards.push(card(
                field(&user, "login"),
                "Connected to GitHub",
                field(&user, "html_url"),
            ));
            if !config.repository.trim().is_empty() {
                let repo = config.repository.trim();
                let parts = repo.split('/').collect::<Vec<_>>();
                if parts.len() != 2
                    || parts.iter().any(|p| {
                        p.is_empty()
                            || !p
                                .bytes()
                                .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
                    })
                    || parts.iter().any(|p| *p == "." || *p == "..")
                {
                    return Err("GitHub repository must be owner/repo.".into());
                }
                let pulls = get(&format!("repos/{repo}/pulls?state=open&per_page=5"))?;
                let pulls = rows(&pulls, "")?;
                cards.push(card(
                    "Recent open pull requests",
                    format!("{} returned (up to 5; not a repository total)", pulls.len()),
                    format!("https://github.com/{repo}/pulls"),
                ));
                for p in pulls {
                    cards.push(card(
                        format!("#{} {}", p["number"], field(p, "title")),
                        format!("by {}", field(&p["user"], "login")),
                        field(p, "html_url"),
                    ))
                }
                match get(&format!("repos/{repo}/actions/runs?per_page=3")) {
                    Ok(runs) => {
                        for run in rows(&runs, "workflow_runs")? {
                            cards.push(card(
                                field(run, "name"),
                                format!("{} {}", field(run, "status"), field(run, "conclusion")),
                                field(run, "html_url"),
                            ))
                        }
                    }
                    Err(e) => cards.push(card(
                        "GitHub Actions",
                        e,
                        format!("https://github.com/{repo}/actions"),
                    )),
                }
            }
        }
        Service::Stripe => {
            let value = get("v1/balance")?;
            let mode = if value["livemode"].as_bool().unwrap_or(false) {
                "Live"
            } else {
                "Test"
            };
            for category in ["available", "pending"] {
                for b in rows(&value, category)? {
                    let amount = b["amount"]
                        .as_i64()
                        .ok_or("Stripe returned an invalid balance.")?;
                    cards.push(card(
                        format!(
                            "{} {} balance",
                            field(b, "currency").to_uppercase(),
                            category
                        ),
                        format!("{amount} minor currency units · {mode} mode"),
                        "https://dashboard.stripe.com/balance",
                    ))
                }
            }
        }
        Service::Vercel => {
            let mut request = client
                .get(endpoint(base, "v7/deployments")?)
                .bearer_auth(key)
                .query(&[("limit", "5")]);
            if !scope.is_empty() {
                request = request.query(&[("teamId", scope)])
            }
            let value = json_response(request)?;
            for d in rows(&value, "deployments")? {
                let url = field(d, "inspectorUrl");
                cards.push(card(
                    field(d, "name"),
                    format!("{} · {}", field(d, "state"), field(d, "target")),
                    if url.is_empty() {
                        "https://vercel.com/dashboard"
                    } else {
                        url
                    },
                ))
            }
        }
        Service::Resend => {
            let value = get("domains?limit=5")?;
            for domain in rows(&value, "data")? {
                cards.push(card(
                    field(domain, "name"),
                    format!("{} · {}", field(domain, "status"), field(domain, "region")),
                    "https://resend.com/domains",
                ))
            }
        }
        Service::Notion => {
            let mut body = json!({"page_size":5,"sort":{"direction":"descending","timestamp":"last_edited_time"},"filter":{"property":"object","value":"page"}});
            if !scope.is_empty() {
                body["query"] = json!(scope)
            }
            let value = json_response(
                client
                    .post(endpoint(base, "v1/search")?)
                    .bearer_auth(key)
                    .header("Notion-Version", "2026-03-11")
                    .json(&body),
            )?;
            for page in rows(&value, "results")? {
                let title = page["properties"]
                    .as_object()
                    .and_then(|properties| properties.values().find_map(|p| p["title"].as_array()))
                    .map(|a| {
                        a.iter()
                            .map(|part| field(part, "plain_text"))
                            .collect::<String>()
                    })
                    .filter(|t| !t.is_empty())
                    .unwrap_or_else(|| "Untitled page".into());
                cards.push(card(
                    title,
                    format!("Edited {}", field(page, "last_edited_time")),
                    field(page, "url"),
                ))
            }
        }
        Service::CalCom => {
            let mut request = client
                .get(endpoint(base, "v2/bookings")?)
                .bearer_auth(key)
                .header("cal-api-version", "2026-05-01")
                .query(&[("status", "upcoming"), ("limit", "5")]);
            if !scope.is_empty() {
                request = request.query(&[("eventTypeId", scope)])
            }
            let value = json_response(request)?;
            if value["status"] != "success" {
                return Err("Cal.com could not list bookings. Check permissions.".into());
            }
            for booking in rows(&value, "data")? {
                cards.push(card(
                    field(booking, "title"),
                    format!("{} · {}", field(booking, "start"), field(booking, "status")),
                    "https://app.cal.com/bookings/upcoming",
                ))
            }
        }
        Service::N8n => {
            let mut request = client
                .get(endpoint(base, "api/v1/workflows")?)
                .header("X-N8N-API-KEY", key)
                .query(&[("limit", "5"), ("excludePinnedData", "true")]);
            if !scope.is_empty() {
                request = request.query(&[("projectId", scope)])
            }
            let value = json_response(request)?;
            for workflow in rows(&value, "data")? {
                let active = workflow["active"].as_bool().unwrap_or_else(|| {
                    workflow["activeVersionId"]
                        .as_str()
                        .is_some_and(|s| !s.is_empty())
                });
                cards.push(card(
                    field(workflow, "name"),
                    if active {
                        "Active workflow"
                    } else {
                        "Inactive workflow"
                    },
                    base,
                ))
            }
        }
    }
    if cards.is_empty() {
        cards.push(card(
            "Connected",
            "No items returned for this account / scope.",
            base,
        ))
    }
    Ok(cards)
}

pub fn lookup_lyrics(title: &str, artist: &str, duration: f64) -> Result<String, String> {
    if title.trim().is_empty() || artist.trim().is_empty() {
        return Err("The player must provide both a title and artist to find lyrics.".into());
    }
    let mut request = http()?
        .get("https://lrclib.net/api/get")
        .query(&[("track_name", title.trim()), ("artist_name", artist.trim())]);
    if duration.is_finite() && (1.0..=3600.0).contains(&duration) {
        request = request.query(&[("duration", duration)])
    }
    let value = json_response(request.timeout(Duration::from_secs(15)))?;
    if value["instrumental"].as_bool() == Some(true) {
        return Ok("Instrumental track (no lyrics).".into());
    }
    value["syncedLyrics"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            value["plainLyrics"]
                .as_str()
                .filter(|s| !s.trim().is_empty())
        })
        .map(str::to_string)
        .ok_or_else(|| "No lyrics were returned for this track.".into())
}

fn transcribe(path: &std::path::Path) -> Result<String, String> {
    let metadata = std::fs::metadata(path)
        .map_err(|_| "The recording is unavailable. Record audio first.".to_string())?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_AUDIO {
        return Err(
            "Choose an audio recording between 1 byte and 25 MB. Split longer recordings first."
                .into(),
        );
    }
    let key = required_secret(ChatProvider::OpenAi.secret_name())?;
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mime = match extension.as_str() {
        "wav" => "audio/wav",
        "mp3" | "mpga" | "mpeg" => "audio/mpeg",
        "m4a" | "mp4" => "audio/mp4",
        "webm" => "audio/webm",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        _ => {
            return Err(
                "Unsupported recording format. Choose WAV, MP3, M4A, MP4, WebM, Ogg or FLAC."
                    .into(),
            );
        }
    };
    let file =
        std::fs::File::open(path).map_err(|_| "Could not open the recording.".to_string())?;
    let mut bytes = Vec::new();
    file.take(MAX_AUDIO + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not read the recording.".to_string())?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_AUDIO {
        return Err("The recording changed size. Choose an audio file under 25 MB.".into());
    }
    let part = reqwest::blocking::multipart::Part::bytes(bytes)
        .file_name(format!("recording.{extension}"))
        .mime_str(mime)
        .map_err(|_| "Unsupported audio encoding.".to_string())?;
    let form = reqwest::blocking::multipart::Form::new()
        .text("model", "gpt-4o-mini-transcribe")
        .text("response_format", "json")
        .part("file", part);
    let result = json_response(
        http()?
            .post("https://api.openai.com/v1/audio/transcriptions")
            .bearer_auth(key)
            .timeout(Duration::from_secs(180))
            .multipart(form),
    )?;
    result["text"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| "No speech was returned for this recording.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    fn server(status: u16, body: &str) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let body = body.to_string();
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut block = [0; 4096];
            loop {
                let n = stream.read(&mut block).unwrap();
                if n == 0 {
                    break;
                }
                bytes.extend_from_slice(&block[..n]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]);
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|v| v.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            write!(stream,"HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            String::from_utf8(bytes).unwrap()
        });
        (format!("http://{address}"), handle)
    }
    #[test]
    fn endpoints_require_tls_except_loopback() {
        for good in [
            "https://example.com/api",
            "http://localhost:5678",
            "http://127.9.8.7",
            "http://[::1]:1234",
        ] {
            assert!(validate_base_url(good).is_ok(), "{good}")
        }
        for bad in [
            "http://example.com",
            "file:///tmp/key",
            "http://127.0.0.1.evil.com",
            "https://user:secret@example.com",
            "https://example.com?api_key=secret",
            "https://example.com#secret",
        ] {
            assert!(validate_base_url(bad).is_err(), "{bad}")
        }
    }
    #[test]
    fn openai_compatible_posts_and_parses() {
        let (base, server) = server(200, r#"{"choices":[{"message":{"content":"hello cat"}}]}"#);
        let config = ChatConfig {
            provider: ChatProvider::Ollama,
            model: "test-model".into(),
            base_url: base,
        };
        let result = chat_with_key(
            &config,
            &[ChatMessage {
                role: "user".into(),
                content: "hello".into(),
            }],
            Some("test-credential"),
        )
        .unwrap();
        assert_eq!(result, "hello cat");
        let request = server.join().unwrap();
        assert!(request.starts_with("POST /chat/completions "));
        assert!(request.contains("Bearer test-credential"));
        let body: Value = serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(body["messages"][0]["content"], "hello");
        assert_eq!(body["stream"], false)
    }
    #[test]
    fn anthropic_separates_system_and_collects_text() {
        let (base, server) = server(
            200,
            r#"{"content":[{"type":"thinking","thinking":"private"},{"type":"text","text":"hello"},{"type":"text","text":"cat"}]}"#,
        );
        let config = ChatConfig {
            provider: ChatProvider::Anthropic,
            model: "test-model".into(),
            base_url: base,
        };
        assert_eq!(
            chat_with_key(
                &config,
                &[
                    ChatMessage {
                        role: "system".into(),
                        content: "rules".into()
                    },
                    ChatMessage {
                        role: "user".into(),
                        content: "hi".into()
                    }
                ],
                Some("test-key")
            )
            .unwrap(),
            "hello\ncat"
        );
        let request = server.join().unwrap();
        assert!(request.to_lowercase().contains("x-api-key: test-key"));
        let body: Value = serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(body["system"], "rules");
        assert_eq!(body["messages"].as_array().unwrap().len(), 1)
    }
    #[test]
    fn errors_do_not_echo_provider_body_or_secret() {
        let (base, server) = server(401, r#"{"error":"secret-test-key private response"}"#);
        let result =
            json_response(http().unwrap().get(base).bearer_auth("secret-test-key")).unwrap_err();
        assert!(result.contains("Authentication"));
        assert!(!result.contains("secret-test-key"));
        assert!(!result.contains("private response"));
        server.join().unwrap();
    }
    #[test]
    fn service_scope_is_encoded_and_status_parsed() {
        let (base, server) = server(
            200,
            r#"{"deployments":[{"name":"my-app","state":"READY","target":"production","inspectorUrl":"https://vercel.com/project/deploy"}]}"#,
        );
        let config = ServiceConfig {
            service: Service::Vercel,
            scope: "team/a&secret".into(),
            ..Default::default()
        };
        let cards = service_with_key(&config, "test-key", Some(&base)).unwrap();
        assert_eq!(cards[0].title, "my-app");
        assert!(cards[0].detail.contains("READY"));
        let request = server.join().unwrap();
        assert!(request.contains("teamId=team%2Fa%26secret"));
    }
    #[test]
    fn links_reject_active_and_credential_schemes() {
        assert_eq!(safe_link("javascript:alert(1)"), "");
        assert_eq!(safe_link("https://user:pass@example.com"), "");
        assert_eq!(safe_link("https://example.com"), "https://example.com/");
    }
    #[test]
    fn settings_serialize_without_credentials() {
        let config = ChatConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("api_key"));
        assert!(!json.contains("secret"));
    }
    #[test]
    fn services_parse_real_response_shapes() {
        let fixtures = [
            (
                Service::GitHub,
                r#"{"login":"cat-owner","html_url":"https://github.com/cat-owner"}"#,
                "GET /user ",
                "cat-owner",
            ),
            (
                Service::Stripe,
                r#"{"available":[{"amount":1234,"currency":"jpy"}],"pending":[],"livemode":false}"#,
                "GET /v1/balance ",
                "JPY available balance",
            ),
            (
                Service::Resend,
                r#"{"data":[{"name":"example.com","status":"verified","region":"us-east-1"}]}"#,
                "GET /domains?limit=5 ",
                "example.com",
            ),
            (
                Service::Notion,
                r#"{"results":[{"properties":{"Name":{"title":[{"plain_text":"Cat notes"}]}},"last_edited_time":"2026-10-18","url":"https://www.notion.so/page"}]}"#,
                "POST /v1/search ",
                "Cat notes",
            ),
            (
                Service::CalCom,
                r#"{"status":"success","data":[{"title":"Cat meeting","start":"2026-10-19T08:00:00Z","status":"accepted"}]}"#,
                "GET /v2/bookings?status=upcoming&limit=5 ",
                "Cat meeting",
            ),
            (
                Service::N8n,
                r#"{"data":[{"name":"Cat workflow","activeVersionId":"v1"}]}"#,
                "GET /api/v1/workflows?limit=5&excludePinnedData=true ",
                "Cat workflow",
            ),
        ];
        for (service, body, request_line, title) in fixtures {
            let (base, server) = server(200, body);
            let config = ServiceConfig {
                service,
                base_url: base.clone(),
                ..Default::default()
            };
            let cards = service_with_key(&config, "test-key", Some(&base)).unwrap();
            assert_eq!(cards[0].title, title);
            let request = server.join().unwrap();
            assert!(request.starts_with(request_line), "{request}");
            if service == Service::N8n {
                assert!(request.to_lowercase().contains("x-n8n-api-key: test-key"));
                assert!(cards[0].detail.contains("Active"));
            }
        }
    }
}
