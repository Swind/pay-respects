// Shared metadata and dispatch helpers for talking to rig-core provider
// clients. Used by both `provider.rs` (streaming completions) and
// `login.rs` (credential verification + model listing).

/// All supported providers as `(id, display_name)`, in the order shown to
/// the user during `pay-respects login`.
pub const PROVIDERS: &[(&str, &str)] = &[
	("openai", "OpenAI"),
	("anthropic", "Anthropic"),
	("chatgpt", "ChatGPT (subscription, OAuth login)"),
	("gemini", "Google Gemini"),
	("mistral", "Mistral"),
	("cohere", "Cohere"),
	("xai", "xAI"),
	("deepseek", "DeepSeek"),
	("perplexity", "Perplexity"),
	("together", "Together AI"),
	("groq", "Groq"),
	("openrouter", "OpenRouter"),
	("huggingface", "Hugging Face"),
	("hyperbolic", "Hyperbolic"),
	("ollama", "Ollama (local, usually no API key required)"),
	("llamafile", "Llamafile (local, no API key)"),
	("moonshot", "Moonshot (Kimi)"),
	("minimax", "MiniMax"),
	("xiaomimimo", "Xiaomi MiMo"),
	("zai", "Z.AI (GLM)"),
	("galadriel", "Galadriel"),
	("mira", "Mira"),
];

/// Providers that support fetching a list of available models via the API.
/// All other supported providers require the model name/ID to be entered
/// manually.
pub const LISTING_PROVIDERS: &[&str] = &[
	"openai",
	"anthropic",
	"deepseek",
	"mistral",
	"gemini",
	"ollama",
	"openrouter",
	"xiaomimimo",
];

/// Providers that don't support API key authentication at all (purely
/// local runners).
pub const NO_AUTH_PROVIDERS: &[&str] = &["llamafile"];

/// Providers that authenticate via OAuth device code flow instead of a
/// static API key. rig-core handles the full flow (token storage, refresh).
pub const OAUTH_PROVIDERS: &[&str] = &["chatgpt"];

/// Z.AI coding plan endpoint (alternative to the default general endpoint).
pub const ZAI_CODING_URL: &str = "https://api.z.ai/api/coding/paas/v4";

pub fn is_known_provider(provider: &str) -> bool {
	provider.is_empty() || PROVIDERS.iter().any(|(id, _)| *id == provider)
}

pub fn supports_listing(provider: &str) -> bool {
	LISTING_PROVIDERS.contains(&provider)
}

pub fn requires_no_auth(provider: &str) -> bool {
	NO_AUTH_PROVIDERS.contains(&provider)
}

pub fn is_oauth_provider(provider: &str) -> bool {
	OAUTH_PROVIDERS.contains(&provider)
}

/// Builds a client for `$client` (a `rig_core::providers::*::Client` type
/// path) authenticated with `$key`, optionally overriding its base URL.
#[macro_export]
macro_rules! build_client {
	($client:ty, $key:expr, $url:expr) => {{
		let mut builder = <$client>::builder().api_key($key.to_string());
		if let Some(url) = $url {
			builder = builder.base_url(url);
		}
		builder
			.build()
			.map_err(|e| format!("failed to build client: {e}"))
	}};
}

/// Like [`build_client!`], but for providers that don't support API key
/// authentication (e.g. Llamafile).
#[macro_export]
macro_rules! build_client_no_auth {
	($client:ty, $url:expr) => {{
		let mut builder = <$client>::builder().api_key(rig_core::client::Nothing);
		if let Some(url) = $url {
			builder = builder.base_url(url);
		}
		builder
			.build()
			.map_err(|e| format!("failed to build client: {e}"))
	}};
}
