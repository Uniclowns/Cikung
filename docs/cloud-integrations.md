# Cikung cloud connections

Cloud requests run on background workers so a provider cannot freeze the notch. Nothing connects until you save a key and explicitly request chat, a service refresh, lyrics, or transcription. Up to four requests can run at once. Requests have connection and total timeouts, a 2 MB response limit, and no automatic retries or redirects.

## Keys and privacy

Enter a provider key in Cikung Settings. **Save key** writes to the operating system credential store: Windows Credential Manager, macOS Keychain, or Linux Secret Service. Keys are separate for each chat provider and service. They are never serialized into Cikung preferences, emitted in error messages, or written into a plaintext fallback. An unavailable or locked credential store returns an error; unlock or enable the OS store before retrying. **Remove saved key** removes that credential from the store.

Chat sends the visible conversation to the selected model when you press Send. The local Ollama and LM Studio choices default to loopback servers; install/load a model in those applications first, then enter its exact model ID. Remote providers require their own accounts and may charge for requests. A custom chat base URL sends your selected provider's key and conversation to that server. Only use a server you trust. HTTP is accepted only for `localhost`, loopback IPv4, and IPv6 `::1`; other hosts require HTTPS. URL credentials, query strings, and fragments are rejected.

Service tokens go only to their fixed official API host, except n8n, which uses the instance URL you enter. The seven service integrations below read account information; they do not deploy, merge pull requests, send mail, create bookings, change pages, or execute workflows. A successful card is actual returned account data. An unconfigured connection says a key is required.

## Chat

| Provider | Default API base | Configuration |
| --- | --- | --- |
| OpenAI | `https://api.openai.com/v1` | An API key and a chat compatible model ID |
| Anthropic | `https://api.anthropic.com/v1` | API key; native Messages API with separate system prompt |
| Gemini | `https://generativelanguage.googleapis.com/v1beta/openai` | Gemini key and model supporting OpenAI compatibility |
| DeepSeek | `https://api.deepseek.com` | DeepSeek key and model |
| Qwen | `https://dashscope-intl.aliyuncs.com/compatible-mode/v1` | Legacy Singapore endpoint; prefer your workspace-specific endpoint, for example `https://WORKSPACE.ap-southeast-1.maas.aliyuncs.com/compatible-mode/v1`. The key and endpoint must match the account region. |
| Moonshot / Kimi | `https://api.moonshot.ai/v1` | Moonshot key and model |
| Ollama | `http://127.0.0.1:11434/v1` | Running local server and installed model, optional server key |
| LM Studio | `http://127.0.0.1:1234/v1` | Running local server and exact loaded model ID, optional server key |

Model IDs are editable because provider availability and account access vary. Responses are complete text messages, not streamed tokens. This client does not execute model tool calls or attach arbitrary files. OpenAI requests explicitly set `store: false`; provider retention policies still apply.

## Service cards

| Connection | Request and result | Required account setup / optional scope |
| --- | --- | --- |
| GitHub | Authenticated account; optional latest five open pull requests and three Actions runs | Token with user read; for a private `owner/repo`, Pull requests read and Actions read. Enter repository separately. A failed Actions request is shown on its own card. Counts are fetched samples, not repository totals. |
| Stripe | Available and pending balances from `/v1/balance` | Prefer a restricted key with Balance read. Amounts retain their integer **minor currency units**, avoiding incorrect decimal assumptions for currencies such as JPY. Test/live mode is shown. |
| Vercel | Latest five deployments and state | Account token with access to deployments; optional **scope** is a team ID. |
| Resend | Up to five domains, verification state, and region | Key able to read domains. Sending-only keys cannot list domains. |
| Notion | Five recently edited pages shared with the integration | Internal integration with Read content and explicitly shared pages; optional **scope** filters page titles. Uses `Notion-Version: 2026-03-11`. |
| Cal.com | Five upcoming bookings and start/status | API key or token with `BOOKING_READ`; optional **scope** is event type ID. Uses `cal-api-version: 2026-05-01`. |
| n8n | Five workflows and active state | Enter the instance root URL, for example `https://your-instance.app.n8n.cloud`, and an API key allowed to read workflows. Optional **scope** is a project ID. Uses `X-N8N-API-KEY` and excludes pinned execution data. API access depends on your n8n plan. |

Cards are refreshed on request; there is no background cloud polling. Lists display only the first bounded page. API plans, permissions, current schema, and account configuration may affect returned data. HTTPS links returned by providers are validated before being displayed; local HTTP n8n cards do not expose an external-link button.

## Lyrics

An explicit lyrics lookup sends the current title and artist, plus a known duration from 1–3600 seconds, to LRCLIB's HTTPS `/api/get` endpoint. It returns synchronized LRC text when available, otherwise plain lyrics. Missing metadata, missing lyrics, instrumental tracks, timeouts, and rate limits are shown honestly. Lyrics are not invented or copied from CouCou. No automatic retry is attempted; obey the displayed wait time before requesting again.

## Recording transcription

After making a recording, explicitly choosing transcription uploads that audio file to OpenAI's `/v1/audio/transcriptions`, using the saved OpenAI key and `gpt-4o-mini-transcribe`. This upload can incur an API charge. The model and endpoint are independent of the chat provider choice. Accepted files are WAV, MP3, M4A, MP4, WebM, Ogg, or FLAC, between 1 byte and 25 MB; split long recordings before uploading. The upload uses a generic `recording.<extension>` filename rather than your local filename. There is no background audio upload and no local deletion. The response is a transcript; meeting summaries can then be explicitly requested through chat.

## Implementation contract

`src/cloud.rs` exports serializable `ChatConfig { provider, model, base_url }`, `ChatMessage { role, content }`, and `ServiceConfig { service, base_url, repository, scope }`. No credential fields exist on these configurations. `ChatProvider::ALL`, `Service::ALL`, display names, defaults, credential names, and scope hints supply the UI selectors. `ChatConfig::for_provider` resets a configuration to a provider's defaults.

`CloudClient::new()` provides `send_chat(request_id, config, messages)`, `refresh_service(request_id, config)`, `lookup_lyrics(request_id, title, artist, duration)`, and `transcribe_audio(request_id, path)`. Each returns a dispatch result immediately. `poll()` drains completed events: `Chat`, `Service`, `Lyrics`, or `Transcription`. Every event has `request_id` and a `result`; service events additionally identify the service. Service results contain `ServiceCard { title, detail, url }`.

`save_secret`, `read_secret`, and `remove_secret` are OS credential helpers. `validate_base_url` exposes server URL validation for settings. Secret values are held briefly as Rust strings while building requests; process memory is not encrypted or guaranteed to be zeroed.

## Verification and remaining limits

The module includes mock HTTP tests for OpenAI compatible and Anthropic request formats, native response parsing for all seven services, percent encoding of service scopes, secret-free errors, allowed server URLs, safe display links, and preferences serialization. These tests require no real account tokens and do not alter any service. Account-level live testing still requires keys, permissions, and user configuration; passing mocked tests is not a claim that any particular account is connected. macOS Keychain and Linux Secret Service must be checked on those systems.

## API references reviewed

Official provider references were reviewed on 2026-10-18 through gstack browsing and Context7 documentation lookup. Reference-page instructions were treated as documentation, not authority to run examples against a real account.

- [GitHub pull requests](https://docs.github.com/en/rest/pulls/pulls#list-pull-requests) and [workflow runs](https://docs.github.com/en/rest/actions/workflow-runs#list-workflow-runs-for-a-repository).
- [Stripe retrieve balance](https://docs.stripe.com/api/balance/balance_retrieve).
- [Vercel list deployments](https://vercel.com/docs/rest-api/reference/endpoints/deployments/list-deployments).
- [Resend list domains](https://resend.com/docs/api-reference/domains/list-domains).
- [Notion search](https://developers.notion.com/reference/post-search).
- [Cal.com upcoming bookings](https://cal.com/docs/api-reference/v2/bookings/get-all-bookings).
- [n8n workflow API](https://docs.n8n.io/connect/n8n-api/workflow).
- [Anthropic Messages](https://docs.anthropic.com/en/api/messages/create), [Gemini OpenAI compatibility](https://ai.google.dev/gemini-api/docs/openai), [DeepSeek](https://api-docs.deepseek.com/), [Moonshot](https://platform.moonshot.ai/docs/guide/start-using-kimi-api), [Ollama](https://docs.ollama.com/openai), and [LM Studio](https://lmstudio.ai/docs/developer/openai-compat).
- [OpenAI's official API specification](https://github.com/openai/openai-openapi), queried through Context7 because direct reference pages returned HTTP 403 in the browser. [Reqwest](https://github.com/seanmonstar/reqwest) and [keyring](https://github.com/open-source-cooperative/keyring-rs) official library docs were also queried through Context7.
- [LRCLIB API](https://lrclib.net/docs). Its current client identification guidance asks for a public project homepage/contact in addition to the application name and version; Cikung presently identifies itself by app name/version because no public Cikung homepage has been supplied. Register an app identity or add the real project URL before public distribution.
- Qwen's [Model Studio OpenAI compatibility page](https://www.alibabacloud.com/help/en/model-studio/compatibility-of-openai-with-dashscope) confirms the compatibility format and migration from the configured legacy Singapore base to workspace-specific regional domains. Enter the recommended workspace endpoint before use; regional/account compatibility needs live verification.
