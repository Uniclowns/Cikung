//! Authenticated, opt-in bridges. No endpoint executes tools or shell commands.
use eframe::egui;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    io::Read,
    net::{IpAddr, Ipv4Addr, UdpSocket},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

const MAX_BODY: usize = 48 * 1024;
const APPROVAL_TTL: u64 = 120;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentStep {
    pub state: String,
    pub detail: String,
    pub timestamp: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentSession {
    pub session_id: String,
    pub agent: String,
    pub project: String,
    pub state: String,
    pub detail: String,
    pub file_diff: Option<String>,
    pub request_id: Option<String>,
    pub updated_at: u64,
    pub history: Vec<AgentStep>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PhoneMediaCommand {
    pub command: String,
    pub session_id: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct BridgeSnapshot {
    pub sessions: Vec<AgentSession>,
    pub phone_enabled: bool,
    pub phone_port: Option<u16>,
    pub paired_phones: usize,
    pub last_error: Option<String>,
}
#[derive(Deserialize)]
struct AgentEvent {
    session_id: String,
    agent: String,
    #[serde(default)]
    project: String,
    state: String,
    #[serde(default)]
    detail: String,
    #[serde(default)]
    file_diff: Option<String>,
    #[serde(default)]
    request_id: Option<String>,
}
struct Decision {
    session_id: String,
    result: Option<bool>,
    created_at: u64,
}
#[derive(Default)]
struct State {
    sessions: HashMap<String, AgentSession>,
    completions: VecDeque<String>,
    completion_seen: HashSet<String>,
    decisions: HashMap<String, Decision>,
    media: Value,
    phone_port: Option<u16>,
    phone_token: Option<String>,
    phone_host: Option<String>,
    cookies: HashMap<String, u64>,
    commands: VecDeque<PhoneMediaCommand>,
    last_error: Option<String>,
    agents_peer: Option<AgentsPeer>,
    agents_last_seen: Option<Instant>,
    // Separate from the adapter key: tools must never be able to approve themselves.
    agents_link_key: String,
}

/// Memory-only handoff from the Agents companion to its Phone child process.
#[derive(Clone)]
struct AgentsPeer {
    endpoint: String,
    token: String,
    #[cfg(test)]
    generation: String,
    client: reqwest::blocking::Client,
}
impl AgentsPeer {
    fn request(&self, path: &str, body: Option<Value>) -> Result<Value, String> {
        let request = if let Some(body) = body {
            self.client
                .post(format!("{}{path}", self.endpoint))
                .json(&body)
        } else {
            self.client.get(format!("{}{path}", self.endpoint))
        };
        let response = request
            .bearer_auth(&self.token)
            .send()
            .map_err(|_| "Agents companion is unavailable. Open Phone again from Agents.")?;
        if !response.status().is_success() {
            return Err("Agents connection was rejected or this permission has expired.".into());
        }
        let mut bytes = Vec::new();
        response
            .take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Could not read the Agents connection response.")?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err("Agents connection response exceeded its limit.".into());
        }
        serde_json::from_slice(&bytes).map_err(|_| "Invalid Agents connection response.".into())
    }
    fn decide(&self, request_id: &str, allow: bool) -> Result<(), String> {
        if !identifier(request_id) {
            return Err("Invalid permission request.".into());
        }
        let result = self.request(
            "/v1/agents/decisions",
            Some(json!({"request_id":request_id,"allow":allow})),
        )?;
        if result.get("accepted").and_then(Value::as_bool) != Some(true) {
            return Err("Agents did not accept this permission decision.".into());
        }
        Ok(())
    }
}
type Shared = Arc<Mutex<State>>;
pub struct Bridge {
    shared: Shared,
    ctx: egui::Context,
    agent_token: String,
    local_port: u16,
    stop: Arc<AtomicBool>,
    phone_stop: Option<Arc<AtomicBool>>,
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn secret() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| format!("Could not create secure pairing key: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
fn lock(shared: &Shared) -> std::sync::MutexGuard<'_, State> {
    shared.lock().unwrap_or_else(|e| e.into_inner())
}
fn constant_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes().zip(b.bytes()).fold(0u8, |v, (x, y)| v | (x ^ y)) == 0
}
impl Bridge {
    pub fn start(ctx: egui::Context) -> Result<Self, String> {
        let server = Server::http((Ipv4Addr::LOCALHOST, 0)).map_err(|e| e.to_string())?;
        let local_port = server
            .server_addr()
            .to_ip()
            .ok_or("No local listening address")?
            .port();
        let agent_token = secret()?;
        let shared = Arc::new(Mutex::new(State {
            agents_link_key: secret()?,
            ..State::default()
        }));
        let stop = Arc::new(AtomicBool::new(false));
        run_server(
            server,
            shared.clone(),
            ctx.clone(),
            stop.clone(),
            false,
            agent_token.clone(),
        );
        Ok(Self {
            shared,
            ctx,
            agent_token,
            local_port,
            stop,
            phone_stop: None,
        })
    }
    pub fn local_endpoint(&self) -> String {
        format!("http://127.0.0.1:{}", self.local_port)
    }
    pub fn agent_token(&self) -> String {
        self.agent_token.clone()
    }
    /// Control capability for the trusted Phone companion only, never agent adapters.
    #[cfg(test)]
    pub fn agents_link_token(&self) -> String {
        lock(&self.shared).agents_link_key.clone()
    }
    /// The parent supplies a live loopback endpoint and key through process environment.
    /// No discovery file or persistent credential is created.
    #[cfg(test)]
    pub fn connect_agents(&self, endpoint: &str, token: &str) -> Result<(), String> {
        let url = reqwest::Url::parse(endpoint).map_err(|_| "Invalid Agents address.")?;
        if url.scheme() != "http"
            || url.host_str() != Some("127.0.0.1")
            || url.port().is_none_or(|p| p == self.local_port)
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
            || token.len() != 64
            || !token.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(
                "Agents connection needs a valid loopback address and temporary key.".into(),
            );
        }
        let peer = AgentsPeer {
            endpoint: url.as_str().trim_end_matches('/').to_string(),
            token: token.to_string(),
            generation: secret()?,
            client: reqwest::blocking::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_millis(250))
                .timeout(Duration::from_millis(750))
                .build()
                .map_err(|_| "Could not create the Agents connection.")?,
        };
        {
            let mut state = lock(&self.shared);
            state.agents_peer = Some(peer.clone());
            state.agents_last_seen = None;
            state.sessions.clear();
            state.completion_seen.clear();
            state.completions.clear();
            state.last_error = None;
        }
        let shared = self.shared.clone();
        let stop = self.stop.clone();
        let ctx = self.ctx.clone();
        thread::spawn(move || {
            while !stop.load(Ordering::Acquire) {
                let result = peer.request("/v1/agents/snapshot", None).and_then(|value| {
                    serde_json::from_value::<Vec<AgentSession>>(
                        value
                            .get("sessions")
                            .cloned()
                            .ok_or("Missing Agents sessions.")?,
                    )
                    .map_err(|_| "Invalid Agents sessions.".to_string())
                    .and_then(|sessions| {
                        if sessions.len() > 32
                            || sessions.iter().any(|s| {
                                !identifier(&s.session_id)
                                    || s.agent.len() > 64
                                    || s.project.len() > 256
                                    || s.state.len() > 32
                                    || s.detail.len() > 4000
                                    || s.history.len() > 10
                                    || s.history
                                        .iter()
                                        .any(|h| h.detail.len() > 4000 || h.state.len() > 32)
                                    || s.file_diff.as_ref().is_some_and(|d| d.len() > 24000)
                                    || s.request_id.as_deref().is_some_and(|id| !identifier(id))
                            })
                        {
                            return Err("Agents sessions exceeded their limits.".into());
                        }
                        Ok(sessions)
                    })
                });
                {
                    let mut state = lock(&shared);
                    if state
                        .agents_peer
                        .as_ref()
                        .is_none_or(|p| p.generation != peer.generation)
                    {
                        break;
                    }
                    match result {
                        Ok(sessions) => {
                            state.sessions = sessions
                                .into_iter()
                                .map(|s| (s.session_id.clone(), s))
                                .collect();
                            state.agents_last_seen = Some(Instant::now());
                            state.last_error = None;
                        }
                        Err(error) => {
                            // Never leave actionable stale approvals after loss of the parent.
                            state.sessions.clear();
                            state.completion_seen.clear();
                            state.completions.clear();
                            state.agents_last_seen = None;
                            state.last_error = Some(error);
                        }
                    }
                }
                ctx.request_repaint();
                for _ in 0..5 {
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            }
        });
        Ok(())
    }
    #[cfg(test)]
    pub fn is_agents_linked(&self) -> bool {
        lock(&self.shared)
            .agents_last_seen
            .is_some_and(|seen| seen.elapsed() < Duration::from_secs(3))
    }
    pub fn take_completions(&self) -> Vec<String> {
        lock(&self.shared).completions.drain(..).collect()
    }
    pub fn snapshot(&self) -> BridgeSnapshot {
        snapshot(&mut lock(&self.shared))
    }
    pub fn publish_media(&self, media: Value) {
        lock(&self.shared).media = media;
    }
    pub fn pairing_url(&self) -> Option<String> {
        let s = lock(&self.shared);
        Some(format!(
            "http://{}/pair?token={}",
            s.phone_host.as_ref()?,
            s.phone_token.as_ref()?
        ))
    }
    pub fn enable_phone(&mut self, enabled: bool) -> Result<(), String> {
        if let Some(stop) = self.phone_stop.take() {
            stop.store(true, Ordering::Release);
        }
        {
            let mut s = lock(&self.shared);
            s.phone_port = None;
            s.phone_host = None;
            s.phone_token = None;
            s.cookies.clear();
            s.commands.clear();
        }
        if !enabled {
            self.ctx.request_repaint();
            return Ok(());
        }
        let server = Server::http((Ipv4Addr::UNSPECIFIED, 0)).map_err(|e| e.to_string())?;
        let port = server
            .server_addr()
            .to_ip()
            .ok_or("No phone listening address")?
            .port();
        // Asking the OS for its route does not send a packet or contact a cloud service.
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).map_err(|e| e.to_string())?;
        socket
            .connect((Ipv4Addr::new(192, 0, 2, 1), 80))
            .map_err(|e| e.to_string())?;
        let ip = socket.local_addr().map_err(|e| e.to_string())?.ip();
        if ip.is_loopback() || ip.is_unspecified() {
            return Err("No LAN address found. Connect both devices to the same network.".into());
        }
        let token = secret()?;
        {
            let mut s = lock(&self.shared);
            s.phone_port = Some(port);
            s.phone_host = Some(format!("{ip}:{port}"));
            s.phone_token = Some(token);
        }
        let stop = Arc::new(AtomicBool::new(false));
        run_server(
            server,
            self.shared.clone(),
            self.ctx.clone(),
            stop.clone(),
            true,
            String::new(),
        );
        self.phone_stop = Some(stop);
        self.ctx.request_repaint();
        Ok(())
    }
    pub fn decide(&self, request_id: &str, allow: bool) -> Result<(), String> {
        let peer = { lock(&self.shared).agents_peer.clone() };
        if let Some(peer) = peer {
            return peer.decide(request_id, allow);
        }
        decide(&mut lock(&self.shared), request_id, allow)?;
        self.ctx.request_repaint();
        Ok(())
    }
    pub fn take_media_commands(&self) -> Vec<PhoneMediaCommand> {
        lock(&self.shared).commands.drain(..).collect()
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(stop) = &self.phone_stop {
            stop.store(true, Ordering::Release);
        }
        let mut s = lock(&self.shared);
        s.cookies.clear();
        s.phone_token = None;
        s.agents_link_key.clear();
    }
}
fn snapshot(s: &mut State) -> BridgeSnapshot {
    expire(s);
    let mut sessions: Vec<_> = s.sessions.values().cloned().collect();
    sessions.sort_by_key(|a| std::cmp::Reverse(a.updated_at));
    BridgeSnapshot {
        sessions,
        phone_enabled: s.phone_port.is_some(),
        phone_port: s.phone_port,
        paired_phones: s.cookies.len(),
        last_error: s.last_error.clone(),
    }
}
fn expire(s: &mut State) {
    let time = now();
    s.cookies.retain(|_, expiry| *expiry > time);
    for (id, d) in &mut s.decisions {
        if d.result.is_none() && time.saturating_sub(d.created_at) >= APPROVAL_TTL {
            d.result = Some(false);
            if let Some(session) = s.sessions.get_mut(&d.session_id)
                && session.request_id.as_deref() == Some(id)
            {
                session.state = "denied".into();
                session.detail = "Permission request timed out; denied.".into();
                session.updated_at = time;
                session.request_id = None;
            }
        }
    }
    s.decisions
        .retain(|_, d| time.saturating_sub(d.created_at) < 900);
}
fn decide(s: &mut State, id: &str, allow: bool) -> Result<(), String> {
    expire(s);
    let d = s
        .decisions
        .get_mut(id)
        .ok_or("Unknown or expired permission request")?;
    if d.result.is_some() {
        return Err("This request is already resolved".into());
    }
    d.result = Some(allow);
    if let Some(session) = s.sessions.get_mut(&d.session_id) {
        session.state = if allow { "working" } else { "denied" }.into();
        session.detail = if allow {
            "Approved by you"
        } else {
            "Denied by you"
        }
        .into();
        session.request_id = None;
        session.updated_at = now();
        session.history.push(AgentStep {
            state: session.state.clone(),
            detail: session.detail.clone(),
            timestamp: now(),
        });
        if session.history.len() > 40 {
            session.history.remove(0);
        }
    }
    Ok(())
}
fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
}
fn event(s: &mut State, e: AgentEvent) -> Result<(), String> {
    if !identifier(&e.session_id)
        || e.agent.is_empty()
        || e.agent.len() > 64
        || e.project.len() > 256
        || e.detail.len() > 4000
        || e.file_diff.as_ref().is_some_and(|d| d.len() > 24000)
    {
        return Err("Invalid or oversized event".into());
    }
    if ![
        "idle", "working", "thinking", "waiting", "approval", "complete", "error",
    ]
    .contains(&e.state.as_str())
    {
        return Err("Unsupported agent state".into());
    }
    if e.state == "approval" && !e.request_id.as_deref().is_some_and(identifier) {
        return Err("Approval needs a valid unique request_id".into());
    }
    if e.state != "approval" && e.request_id.is_some() {
        return Err("Only approval events can include request_id".into());
    }
    expire(s);
    if let Some(id) = &e.request_id
        && s.decisions.contains_key(id)
    {
        return Err("request_id has already been used".into());
    }
    // Only one pending request per session; superseding it always denies the earlier one.
    for d in s
        .decisions
        .values_mut()
        .filter(|d| d.session_id == e.session_id && d.result.is_none())
    {
        d.result = Some(false);
    }
    if s.sessions.len() >= 32 && !s.sessions.contains_key(&e.session_id) {
        let oldest = s
            .sessions
            .iter()
            .filter(|(_, v)| v.request_id.is_none())
            .min_by_key(|(_, v)| v.updated_at)
            .map(|(k, _)| k.clone());
        if let Some(id) = oldest {
            s.sessions.remove(&id);
            s.completion_seen.remove(&id);
        } else {
            return Err("Too many active sessions".into());
        }
    }
    if let Some(id) = &e.request_id {
        s.decisions.insert(
            id.clone(),
            Decision {
                session_id: e.session_id.clone(),
                result: None,
                created_at: now(),
            },
        );
    }
    let session = s
        .sessions
        .entry(e.session_id.clone())
        .or_insert_with(|| AgentSession {
            session_id: e.session_id.clone(),
            agent: e.agent.clone(),
            project: e.project.clone(),
            state: String::new(),
            detail: String::new(),
            file_diff: None,
            request_id: None,
            updated_at: now(),
            history: vec![],
        });
    if matches!(e.state.as_str(), "working" | "thinking") {
        s.completion_seen.remove(&e.session_id);
    }
    if e.state == "complete" && s.completion_seen.insert(e.session_id.clone()) {
        s.completions.push_back(e.agent.clone());
        while s.completions.len() > 32 {
            s.completions.pop_front();
        }
    }
    session.agent = e.agent;
    session.project = e.project;
    session.state = e.state;
    session.detail = e.detail;
    session.file_diff = e.file_diff;
    session.request_id = e.request_id;
    session.updated_at = now();
    session.history.push(AgentStep {
        state: session.state.clone(),
        detail: session.detail.clone(),
        timestamp: session.updated_at,
    });
    if session.history.len() > 40 {
        session.history.remove(0);
    }
    Ok(())
}
fn header<'a>(r: &'a Request, name: &str) -> Option<&'a str> {
    r.headers()
        .iter()
        .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name))
        .map(|h| h.value.as_str())
}
fn reply(r: Request, status: u16, body: String, content_type: &str, extra: &[(&str, String)]) {
    let mut response = Response::from_string(body).with_status_code(StatusCode(status));
    for (k, v) in [
        ("Content-Type", content_type),
        ("Cache-Control", "no-store"),
        ("X-Content-Type-Options", "nosniff"),
        ("Referrer-Policy", "no-referrer"),
        ("X-Frame-Options", "DENY"),
        (
            "Content-Security-Policy",
            "default-src 'self'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'",
        ),
    ] {
        if let Ok(h) = Header::from_bytes(k, v) {
            response = response.with_header(h);
        }
    }
    for (k, v) in extra {
        if let Ok(h) = Header::from_bytes(*k, v.as_str()) {
            response = response.with_header(h);
        }
    }
    let _ = r.respond(response);
}
fn json_reply(r: Request, status: u16, value: Value) {
    reply(
        r,
        status,
        value.to_string(),
        "application/json; charset=utf-8",
        &[],
    );
}
fn read_json(r: &mut Request) -> Result<Value, String> {
    if header(r, "Transfer-Encoding").is_some() {
        return Err("Chunked requests are not supported".into());
    }
    if !header(r, "Content-Type").is_some_and(|v| v.split(';').next() == Some("application/json")) {
        return Err("Expected application/json".into());
    }
    let len = r.body_length().ok_or("Content-Length required")?;
    if len > MAX_BODY {
        return Err("Request body too large".into());
    }
    let mut body = Vec::with_capacity(len);
    r.as_reader()
        .take((MAX_BODY + 1) as u64)
        .read_to_end(&mut body)
        .map_err(|e| e.to_string())?;
    if body.len() > MAX_BODY {
        return Err("Request body too large".into());
    }
    serde_json::from_slice(&body).map_err(|_| "Invalid JSON".into())
}
fn run_server(
    server: Server,
    shared: Shared,
    ctx: egui::Context,
    stop: Arc<AtomicBool>,
    phone: bool,
    token: String,
) {
    thread::spawn(move || {
        // Bound requests per source each minute; stale source entries are pruned.
        let mut rates: HashMap<IpAddr, (u64, u32)> = HashMap::new();
        while !stop.load(Ordering::Acquire) {
            let mut r = match server.recv_timeout(Duration::from_millis(150)) {
                Ok(Some(r)) => r,
                Ok(None) => continue,
                Err(_) => break,
            };
            if stop.load(Ordering::Acquire) {
                break;
            }
            let minute = now() / 60;
            rates.retain(|_, (m, _)| *m == minute);
            let ip = r
                .remote_addr()
                .map(|a| a.ip())
                .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
            if rates.len() >= 128 && !rates.contains_key(&ip) {
                json_reply(r, 429, json!({"error":"Too many clients"}));
                continue;
            }
            let rate = rates.entry(ip).or_insert((minute, 0));
            rate.1 += 1;
            if rate.1 > if phone { 300 } else { 2000 } {
                json_reply(r, 429, json!({"error":"Rate limit exceeded"}));
                continue;
            }
            let host = header(&r, "Host").unwrap_or("").to_string();
            let expected = if phone {
                lock(&shared).phone_host.clone().unwrap_or_default()
            } else {
                server
                    .server_addr()
                    .to_ip()
                    .map(|a| format!("127.0.0.1:{}", a.port()))
                    .unwrap_or_default()
            };
            if host != expected {
                json_reply(r, 403, json!({"error":"Invalid host"}));
                continue;
            }
            if header(&r, "Origin").is_some_and(|origin| origin != format!("http://{host}"))
                || header(&r, "Sec-Fetch-Site").is_some_and(|v| v == "cross-site")
            {
                json_reply(r, 403, json!({"error":"Cross-site request blocked"}));
                continue;
            }
            let path = r.url().split('?').next().unwrap_or("").to_string();
            if phone && path == "/pair" && r.method() == &Method::Get {
                let supplied = r.url().strip_prefix("/pair?token=").unwrap_or("");
                let mut s = lock(&shared);
                let valid = s
                    .phone_token
                    .as_deref()
                    .is_some_and(|t| constant_eq(t, supplied));
                if !valid || s.cookies.len() >= 8 {
                    drop(s);
                    json_reply(
                        r,
                        403,
                        json!({"error":"Invalid pairing link or too many phones"}),
                    );
                    continue;
                }
                match secret() {
                    Ok(cookie) => {
                        s.cookies.insert(cookie.clone(), now() + 12 * 60 * 60);
                        drop(s);
                        reply(
                            r,
                            303,
                            String::new(),
                            "text/plain",
                            &[
                                ("Location", "/".into()),
                                (
                                    "Set-Cookie",
                                    format!(
                                        "cikung={cookie}; HttpOnly; SameSite=Strict; Path=/; Max-Age=43200"
                                    ),
                                ),
                            ],
                        );
                        ctx.request_repaint();
                    }
                    Err(_) => {
                        drop(s);
                        json_reply(r, 500, json!({"error":"Pairing unavailable"}));
                    }
                }
                continue;
            }
            let authenticated = if phone {
                let cookie = header(&r, "Cookie").and_then(|v| {
                    v.split(';')
                        .map(str::trim)
                        .find_map(|v| v.strip_prefix("cikung="))
                });
                let mut s = lock(&shared);
                expire(&mut s);
                cookie.is_some_and(|c| s.cookies.get(c).is_some_and(|expiry| *expiry > now()))
            } else {
                header(&r, "Authorization")
                    .and_then(|v| v.strip_prefix("Bearer "))
                    .is_some_and(|v| {
                        if path == "/v1/agents/snapshot" || path == "/v1/agents/decisions" {
                            let state = lock(&shared);
                            !state.agents_link_key.is_empty()
                                && constant_eq(v, &state.agents_link_key)
                        } else {
                            constant_eq(v, &token)
                        }
                    })
            };
            if !authenticated {
                reply(
                    r,
                    401,
                    "Open the pairing link from Cikung on your computer.".into(),
                    "text/plain; charset=utf-8",
                    &[],
                );
                continue;
            }
            if phone && path == "/" && r.method() == &Method::Get {
                reply(
                    r,
                    200,
                    include_str!("../assets/phone.html").into(),
                    "text/html; charset=utf-8",
                    &[],
                );
                continue;
            }
            if phone && path == "/cat.png" && r.method() == &Method::Get {
                let mut response = Response::from_data(
                    include_bytes!("../assets/embed/cikung-face.png").as_slice(),
                );
                for (key, value) in [
                    ("Content-Type", "image/png"),
                    ("Cache-Control", "no-store"),
                    ("X-Content-Type-Options", "nosniff"),
                    ("Referrer-Policy", "no-referrer"),
                ] {
                    if let Ok(h) = Header::from_bytes(key, value) {
                        response = response.with_header(h);
                    }
                }
                let _ = r.respond(response);
                continue;
            }
            if phone && path == "/manifest.webmanifest" && r.method() == &Method::Get {
                reply(r,200,json!({"name":"Cikung Companion","short_name":"Cikung","start_url":"/","display":"standalone","background_color":"#080808","theme_color":"#080808","icons":[{"src":"/cat.png","sizes":"1254x1254","type":"image/png","purpose":"any"}]}).to_string(),"application/manifest+json",&[]);
                continue;
            }
            if path == "/v1/state" && r.method() == &Method::Get {
                let mut s = lock(&shared);
                let snap = snapshot(&mut s);
                let media = s.media.clone();
                let agents_linked = s
                    .agents_last_seen
                    .is_some_and(|seen| seen.elapsed() < Duration::from_secs(3));
                let agents_error = s.agents_peer.as_ref().and(s.last_error.clone());
                drop(s);
                json_reply(
                    r,
                    200,
                    json!({"sessions":snap.sessions,"media":media,"paired_phones":snap.paired_phones,"agents_linked":agents_linked,"agents_error":agents_error}),
                );
                continue;
            }
            if !phone && path == "/v1/agents/snapshot" && r.method() == &Method::Get {
                let mut snap = snapshot(&mut lock(&shared));
                // Phone needs recent context, not an unbounded history transfer.
                for session in &mut snap.sessions {
                    let keep_from = session.history.len().saturating_sub(10);
                    session.history.drain(..keep_from);
                }
                json_reply(r, 200, json!({"sessions":snap.sessions}));
                continue;
            }
            if !phone && path == "/v1/agents/events" && r.method() == &Method::Post {
                if lock(&shared).agents_peer.is_some() {
                    json_reply(
                        r,
                        409,
                        json!({"error":"Send agent events to the Agents companion."}),
                    );
                    continue;
                }
                let result = read_json(&mut r)
                    .and_then(|v| {
                        serde_json::from_value::<AgentEvent>(v).map_err(|e| e.to_string())
                    })
                    .and_then(|e| event(&mut lock(&shared), e));
                match result {
                    Ok(()) => {
                        ctx.request_repaint();
                        json_reply(r, 202, json!({"accepted":true}));
                    }
                    Err(e) => json_reply(r, 400, json!({"error":e})),
                }
                continue;
            }
            if !phone && path.starts_with("/v1/agents/decisions/") && r.method() == &Method::Get {
                let id = &path["/v1/agents/decisions/".len()..];
                let mut s = lock(&shared);
                expire(&mut s);
                let result=s.decisions.get(id).map(|d| json!({"request_id":id,"session_id":d.session_id,"decision":match d.result {Some(true)=>"allow",Some(false)=>"deny",None=>"pending"}}));
                drop(s);
                match result {
                    Some(v) => json_reply(r, 200, v),
                    None => json_reply(r, 404, json!({"error":"Unknown or expired request"})),
                }
                continue;
            }
            if ((phone && path == "/v1/decisions") || (!phone && path == "/v1/agents/decisions"))
                && r.method() == &Method::Post
            {
                let result = read_json(&mut r).and_then(|v| {
                    let id = v
                        .get("request_id")
                        .and_then(Value::as_str)
                        .ok_or("request_id required")?;
                    let allow = v
                        .get("allow")
                        .and_then(Value::as_bool)
                        .ok_or("allow must be boolean")?;
                    let peer = { lock(&shared).agents_peer.clone() };
                    if let Some(peer) = peer {
                        peer.decide(id, allow)
                    } else {
                        decide(&mut lock(&shared), id, allow)
                    }
                });
                match result {
                    Ok(()) => {
                        ctx.request_repaint();
                        json_reply(r, 200, json!({"accepted":true}));
                    }
                    Err(e) => json_reply(r, 400, json!({"error":e})),
                }
                continue;
            }
            if phone && path == "/v1/media/control" && r.method() == &Method::Post {
                let result = read_json(&mut r)
                    .and_then(|v| {
                        serde_json::from_value::<PhoneMediaCommand>(v).map_err(|e| e.to_string())
                    })
                    .and_then(|cmd| {
                        if !["play", "pause", "toggle", "next", "previous"]
                            .contains(&cmd.command.as_str())
                            || cmd.session_id.as_ref().is_some_and(|id| id.len() > 512)
                        {
                            return Err("Unsupported media control".into());
                        }
                        let mut s = lock(&shared);
                        if s.commands.len() >= 16 {
                            return Err("Command queue full".into());
                        }
                        s.commands.push_back(cmd);
                        Ok(())
                    });
                match result {
                    Ok(()) => {
                        ctx.request_repaint();
                        json_reply(r, 202, json!({"queued":true}));
                    }
                    Err(e) => json_reply(r, 400, json!({"error":e})),
                }
                continue;
            }
            json_reply(r, 404, json!({"error":"Unknown endpoint"}));
        }
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn rapid_turns_and_repeated_stop_events_produce_one_completion_per_turn() {
        let bridge = super::Bridge::start(eframe::egui::Context::default()).unwrap();
        for state in ["thinking", "working", "complete", "complete", "idle"] {
            super::event(
                &mut super::lock(&bridge.shared),
                super::AgentEvent {
                    session_id: "sound-test".into(),
                    agent: "Claude Code".into(),
                    project: String::new(),
                    state: state.into(),
                    detail: String::new(),
                    file_diff: None,
                    request_id: None,
                },
            )
            .unwrap();
        }
        assert_eq!(bridge.snapshot().sessions[0].state, "idle");
        super::event(
            &mut super::lock(&bridge.shared),
            super::AgentEvent {
                session_id: "sound-test".into(),
                agent: "Claude Code".into(),
                project: String::new(),
                state: "complete".into(),
                detail: String::new(),
                file_diff: None,
                request_id: None,
            },
        )
        .unwrap();
        assert_eq!(bridge.take_completions(), vec!["Claude Code"]);
        assert!(bridge.take_completions().is_empty());
        for state in ["working", "error", "waiting"] {
            super::event(
                &mut super::lock(&bridge.shared),
                super::AgentEvent {
                    session_id: "sound-test".into(),
                    agent: "Claude Code".into(),
                    project: String::new(),
                    state: state.into(),
                    detail: String::new(),
                    file_diff: None,
                    request_id: None,
                },
            )
            .unwrap();
        }
        assert!(bridge.take_completions().is_empty());
        for _ in 0..2 {
            for state in ["thinking", "complete"] {
                super::event(
                    &mut super::lock(&bridge.shared),
                    super::AgentEvent {
                        session_id: "sound-test".into(),
                        agent: "Claude Code".into(),
                        project: String::new(),
                        state: state.into(),
                        detail: String::new(),
                        file_diff: None,
                        request_id: None,
                    },
                )
                .unwrap();
            }
        }
        assert_eq!(bridge.take_completions().len(), 2);
    }
    #[test]
    fn completion_queue_is_bounded_without_changing_authoritative_sessions() {
        let mut state = super::State::default();
        for index in 0..64 {
            super::event(
                &mut state,
                super::AgentEvent {
                    session_id: format!("sound-{index}"),
                    agent: "Codex".into(),
                    project: String::new(),
                    state: "complete".into(),
                    detail: String::new(),
                    file_diff: None,
                    request_id: None,
                },
            )
            .unwrap();
        }
        assert_eq!(state.completions.len(), 32);
        assert_eq!(state.completion_seen.len(), 32);
        assert_eq!(state.sessions.len(), 32);
    }
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpStream,
    };
    fn request(
        bridge: &Bridge,
        path: &str,
        token: &str,
        body: Option<&str>,
        origin: Option<&str>,
    ) -> String {
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, bridge.local_port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let method = if body.is_some() { "POST" } else { "GET" };
        let data = body.unwrap_or("");
        let origin = origin
            .map(|o| format!("Origin: {o}\r\n"))
            .unwrap_or_default();
        write!(stream,"{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {token}\r\n{origin}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{data}",bridge.local_port,data.len()).unwrap();
        let mut output = String::new();
        stream.read_to_string(&mut output).unwrap();
        output
    }
    fn phone_request(port: u16, path: &str, cookie: Option<&str>, body: Option<&str>) -> String {
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let method = if body.is_some() { "POST" } else { "GET" };
        let data = body.unwrap_or("");
        let cookie = cookie
            .map(|c| format!("Cookie: {c}\r\n"))
            .unwrap_or_default();
        write!(stream,"{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{cookie}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{data}",data.len()).unwrap();
        let mut output = String::new();
        stream.read_to_string(&mut output).unwrap();
        output
    }
    #[test]
    fn local_authentication_and_decisions() {
        let bridge = Bridge::start(egui::Context::default()).unwrap();
        assert!(request(&bridge, "/v1/state", "bad", None, None).starts_with("HTTP/1.1 401"));
        let token = bridge.agent_token();
        assert!(
            request(
                &bridge,
                "/v1/state",
                &token,
                None,
                Some("https://attacker.example")
            )
            .starts_with("HTTP/1.1 403")
        );
        let body = r#"{"session_id":"test","agent":"Claude Code","state":"approval","detail":"Read source","request_id":"req-1"}"#;
        assert!(
            request(&bridge, "/v1/agents/events", &token, Some(body), None)
                .starts_with("HTTP/1.1 202")
        );
        assert!(
            request(&bridge, "/v1/agents/decisions/req-1", &token, None, None).contains("pending")
        );
        bridge.decide("req-1", false).unwrap();
        assert!(
            request(&bridge, "/v1/agents/decisions/req-1", &token, None, None).contains("deny")
        );
        assert!(bridge.decide("req-1", true).is_err());
        assert!(
            request(
                &bridge,
                "/v1/media/control",
                &token,
                Some(r#"{"command":"toggle"}"#),
                None
            )
            .starts_with("HTTP/1.1 404")
        );
    }
    #[cfg(target_os = "windows")]
    #[test]
    fn included_hook_publishes_a_permission_and_returns_the_decision() {
        use std::{
            os::windows::process::CommandExt,
            process::{Command, Stdio},
            time::Instant,
        };
        let bridge = Bridge::start(egui::Context::default()).unwrap();
        let shell = std::env::var_os("SystemRoot")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| "C:\\Windows".into())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let mut child = Command::new(shell)
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/cikung-hook.ps1"))
            .args(["-ClaudeCode", "-TimeoutSeconds", "5"])
            .env("CIKUNG_ENDPOINT", bridge.local_endpoint())
            .env("CIKUNG_TOKEN", bridge.agent_token())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(0x08000000)
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(br#"{"hook_event_name":"PermissionRequest","session_id":"hook-test","cwd":"Local test fixture","tool_name":"Read","tool_input":{"file_path":"fixture.txt"}}"#).unwrap();
        // A cold PowerShell start on CI runners can take most of 12 s on its own;
        // the hook itself still answers within its 5 s timeout once running.
        let deadline = Instant::now() + Duration::from_secs(45);
        let mut decided = false;
        loop {
            if !decided {
                for session in bridge.snapshot().sessions {
                    if let Some(id) = session.request_id {
                        assert_eq!(session.agent, "Claude Code");
                        assert!(session.detail.contains("fixture.txt"));
                        bridge.decide(&id, false).unwrap();
                        decided = true;
                    }
                }
            }
            if child.try_wait().unwrap().is_some() {
                break;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("The included hook did not finish in time");
            }
            thread::sleep(Duration::from_millis(40));
        }
        let output = child.wait_with_output().unwrap();
        assert!(
            decided,
            "The hook never published its request: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let response: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            response["hookSpecificOutput"]["hookEventName"],
            "PermissionRequest"
        );
        assert_eq!(
            response["hookSpecificOutput"]["decision"]["behavior"],
            "deny"
        );
    }
    #[test]
    fn expired_requests_default_deny() {
        let mut s = State::default();
        event(
            &mut s,
            AgentEvent {
                session_id: "s".into(),
                agent: "test".into(),
                project: String::new(),
                state: "approval".into(),
                detail: String::new(),
                file_diff: None,
                request_id: Some("r".into()),
            },
        )
        .unwrap();
        s.decisions.get_mut("r").unwrap().created_at = now() - APPROVAL_TTL;
        assert!(decide(&mut s, "r", true).is_err());
        assert_eq!(s.decisions["r"].result, Some(false));
        assert_eq!(s.sessions["s"].state, "denied");
    }
    #[test]
    fn superseded_requests_are_denied() {
        let mut s = State::default();
        for id in ["r1", "r2"] {
            event(
                &mut s,
                AgentEvent {
                    session_id: "s".into(),
                    agent: "test".into(),
                    project: String::new(),
                    state: "approval".into(),
                    detail: String::new(),
                    file_diff: None,
                    request_id: Some(id.into()),
                },
            )
            .unwrap();
        }
        assert_eq!(s.decisions["r1"].result, Some(false));
        assert!(s.decisions["r2"].result.is_none());
        assert!(!constant_eq("abc", "abd"));
        assert!(!identifier("../oops"));
    }
    #[test]
    fn phone_pairing_is_scoped_and_revocable() {
        // Use loopback as the test LAN, with the same phone router and authentication.
        let bridge = Bridge::start(egui::Context::default()).unwrap();
        let server = Server::http((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        {
            let mut s = lock(&bridge.shared);
            s.phone_port = Some(port);
            s.phone_host = Some(format!("127.0.0.1:{port}"));
            s.phone_token = Some("test-pair-key".into());
        }
        let stop = Arc::new(AtomicBool::new(false));
        run_server(
            server,
            bridge.shared.clone(),
            egui::Context::default(),
            stop.clone(),
            true,
            String::new(),
        );
        assert!(phone_request(port, "/v1/state", None, None).starts_with("HTTP/1.1 401"));
        assert!(phone_request(port, "/pair?token=wrong", None, None).starts_with("HTTP/1.1 403"));
        let paired = phone_request(port, "/pair?token=test-pair-key", None, None);
        assert!(paired.starts_with("HTTP/1.1 303"));
        let cookie = paired
            .lines()
            .find(|line| line.to_ascii_lowercase().starts_with("set-cookie:"))
            .unwrap()
            .split_once(':')
            .unwrap()
            .1
            .trim()
            .split(';')
            .next()
            .unwrap();
        assert!(paired.contains("HttpOnly; SameSite=Strict"));
        assert!(phone_request(port, "/v1/state", Some(cookie), None).starts_with("HTTP/1.1 200"));
        assert!(
            phone_request(
                port,
                "/v1/agents/events",
                Some(cookie),
                Some(r#"{"session_id":"evil","agent":"evil","state":"working"}"#)
            )
            .starts_with("HTTP/1.1 404")
        );
        assert!(
            phone_request(
                port,
                "/v1/media/control",
                Some(cookie),
                Some(r#"{"command":"shell"}"#)
            )
            .starts_with("HTTP/1.1 400")
        );
        assert!(
            phone_request(
                port,
                "/v1/media/control",
                Some(cookie),
                Some(r#"{"command":"toggle"}"#)
            )
            .starts_with("HTTP/1.1 202")
        );
        assert_eq!(bridge.take_media_commands()[0].command, "toggle");
        lock(&bridge.shared).cookies.clear();
        assert!(phone_request(port, "/v1/state", Some(cookie), None).starts_with("HTTP/1.1 401"));
        stop.store(true, Ordering::Release);
    }

    fn wait_for(mut condition: impl FnMut() -> bool) {
        let start = Instant::now();
        while !condition() {
            assert!(
                start.elapsed() < Duration::from_secs(4),
                "bridge fixture timed out"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn phone_companion_mirrors_authoritative_agents_and_proxies_explicit_decisions() {
        let agents = Bridge::start(egui::Context::default()).unwrap();
        let phone = Bridge::start(egui::Context::default()).unwrap();
        assert!(!phone.is_agents_linked());
        assert!(
            request(&agents, "/v1/agents/snapshot", "wrong", None, None)
                .starts_with("HTTP/1.1 401")
        );
        assert!(
            request(
                &agents,
                "/v1/agents/snapshot",
                &agents.agent_token(),
                None,
                None
            )
            .starts_with("HTTP/1.1 401")
        );
        assert!(
            request(
                &agents,
                "/v1/agents/decisions",
                &agents.agent_token(),
                Some(r#"{"request_id":"self-approval","allow":true}"#),
                None
            )
            .starts_with("HTTP/1.1 401")
        );
        phone
            .connect_agents(&agents.local_endpoint(), &agents.agents_link_token())
            .unwrap();
        wait_for(|| phone.is_agents_linked());

        let body = r#"{"session_id":"real-omp","agent":"Oh My Pi","project":"fixture","state":"approval","detail":"Read source","request_id":"phone-allow"}"#;
        assert!(
            request(
                &agents,
                "/v1/agents/events",
                &agents.agent_token(),
                Some(body),
                None
            )
            .starts_with("HTTP/1.1 202")
        );
        wait_for(|| {
            phone
                .snapshot()
                .sessions
                .iter()
                .any(|s| s.request_id.as_deref() == Some("phone-allow"))
        });
        assert_eq!(phone.snapshot().sessions[0].agent, "Oh My Pi");
        assert!(
            request(
                &phone,
                "/v1/agents/events",
                &phone.agent_token(),
                Some(body),
                None
            )
            .starts_with("HTTP/1.1 409")
        );

        // Exercise the phone router with a fixture cookie, without opening a LAN listener.
        let server = Server::http((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        {
            let mut state = lock(&phone.shared);
            state.phone_host = Some(format!("127.0.0.1:{port}"));
            state.phone_port = Some(port);
            state.cookies.insert("fixture-cookie".into(), now() + 60);
        }
        let stop = Arc::new(AtomicBool::new(false));
        run_server(
            server,
            phone.shared.clone(),
            egui::Context::default(),
            stop.clone(),
            true,
            String::new(),
        );
        assert!(
            phone_request(
                port,
                "/v1/decisions",
                None,
                Some(r#"{"request_id":"phone-allow","allow":true}"#)
            )
            .starts_with("HTTP/1.1 401")
        );
        assert!(
            phone_request(
                port,
                "/v1/decisions",
                Some("cikung=fixture-cookie"),
                Some(r#"{"request_id":"phone-allow","allow":true}"#)
            )
            .starts_with("HTTP/1.1 200")
        );
        assert!(
            request(
                &agents,
                "/v1/agents/decisions/phone-allow",
                &agents.agent_token(),
                None,
                None
            )
            .contains("\"decision\":\"allow\"")
        );
        assert!(phone.decide("phone-allow", true).is_err());

        let deny = body.replace("phone-allow", "phone-deny");
        request(
            &agents,
            "/v1/agents/events",
            &agents.agent_token(),
            Some(&deny),
            None,
        );
        wait_for(|| {
            phone
                .snapshot()
                .sessions
                .iter()
                .any(|s| s.request_id.as_deref() == Some("phone-deny"))
        });
        assert!(
            phone_request(
                port,
                "/v1/decisions",
                Some("cikung=fixture-cookie"),
                Some(r#"{"request_id":"phone-deny","allow":false}"#)
            )
            .starts_with("HTTP/1.1 200")
        );
        assert!(
            request(
                &agents,
                "/v1/agents/decisions/phone-deny",
                &agents.agent_token(),
                None,
                None
            )
            .contains("\"decision\":\"deny\"")
        );
        assert!(
            phone_request(port, "/v1/state", Some("cikung=fixture-cookie"), None)
                .contains("Oh My Pi")
        );
        stop.store(true, Ordering::Release);
    }

    #[test]
    fn phone_agent_link_rejects_remote_addresses_and_clears_stale_permissions_on_disconnect() {
        let agents = Bridge::start(egui::Context::default()).unwrap();
        let phone = Bridge::start(egui::Context::default()).unwrap();
        let key = agents.agent_token();
        for endpoint in [
            "http://example.com:1234",
            "http://localhost:1234",
            "http://127.0.0.1:1234/path",
            "http://user@127.0.0.1:1234",
            "http://127.0.0.1:1234?x=1",
        ] {
            assert!(phone.connect_agents(endpoint, &key).is_err());
        }
        assert!(phone.connect_agents(&phone.local_endpoint(), &key).is_err());
        assert!(
            phone
                .connect_agents(&agents.local_endpoint(), "short")
                .is_err()
        );
        phone
            .connect_agents(&agents.local_endpoint(), &"0".repeat(64))
            .unwrap();
        wait_for(|| phone.snapshot().last_error.is_some());
        assert!(!phone.is_agents_linked());

        let pending = r#"{"session_id":"claude","agent":"Claude","state":"approval","request_id":"stale-request"}"#;
        request(&agents, "/v1/agents/events", &key, Some(pending), None);
        phone
            .connect_agents(&agents.local_endpoint(), &agents.agents_link_token())
            .unwrap();
        wait_for(|| phone.is_agents_linked() && !phone.snapshot().sessions.is_empty());
        drop(agents);
        wait_for(|| !phone.is_agents_linked() && phone.snapshot().sessions.is_empty());
        assert!(phone.decide("stale-request", true).is_err());
        assert!(phone.snapshot().last_error.is_some());

        let restarted = Bridge::start(egui::Context::default()).unwrap();
        phone
            .connect_agents(&restarted.local_endpoint(), &restarted.agents_link_token())
            .unwrap();
        wait_for(|| phone.is_agents_linked());
        assert!(phone.snapshot().sessions.is_empty());
        assert!(phone.snapshot().last_error.is_none());
    }
}
