use askama::Template;
use serde_json::Value;

use crate::buffer;
use crate::config;
use crate::provider;

struct Conf {
	provider: String,
	key: String,
	url: Option<String>,
	model: String,
	extra: Option<Value>,
}

#[derive(Template)]
#[template(path = "prompt.txt")]
struct AiPrompt<'a> {
	last_command: &'a str,
	error_msg: &'a str,
	additional_prompt: &'a str,
	set_locale: &'a str,
}

pub async fn ai_suggestion(last_command: &str, error_msg: &str, locale: &str) {
	let conf = match Conf::new() {
		Some(conf) => conf,
		None => {
			return;
		}
	};

	let file_conf = config::load_config();
	let ai = file_conf.ai.unwrap_or_default();

	let char_limit = 500;

	let error_msg = if error_msg.chars().count() > 500 + 3 {
		let half = char_limit / 2;
		format!(
			"{}...{}",
			error_msg.chars().take(half).collect::<String>(),
			error_msg
				.chars()
				.rev()
				.take(half)
				.collect::<String>()
				.chars()
				.rev()
				.collect::<String>()
		)
	} else {
		error_msg.to_string()
	};

	let user_locale = {
		let locale = std::env::var("_PR_AI_LOCALE")
			.ok()
			.or(ai.locale)
			.unwrap_or_else(|| locale.to_string());
		if locale.len() < 2 {
			"en-US".to_string()
		} else {
			locale
		}
	};

	let set_locale = if !user_locale.starts_with("en") {
		format!(". Use language for locale {}", user_locale)
	} else {
		"".to_string()
	};

	let addtional_prompt = std::env::var("_PR_AI_ADDITIONAL_PROMPT")
		.ok()
		.or(ai.additional_prompt)
		.unwrap_or_default();

	let ai_prompt = AiPrompt {
		last_command,
		error_msg: &error_msg,
		additional_prompt: &addtional_prompt,
		set_locale: &set_locale,
	}
	.render()
	.unwrap()
	.trim()
	.to_string();

	#[cfg(debug_assertions)]
	eprintln!("AI module: AI prompt: {}", ai_prompt);

	let mut buffer = buffer::Buffer::new();
	let result = provider::stream_completion(
		&conf.provider,
		&conf.key,
		conf.url.as_deref(),
		&conf.model,
		conf.extra.clone(),
		ai_prompt,
		&mut buffer,
	)
	.await;

	if let Err(e) = result {
		eprintln!("AI module: request failed: {}", e);
		return;
	}

	let suggestions = buffer
		.buf
		.trim()
		.trim_end_matches("```")
		.trim()
		.trim_start_matches("<suggest>")
		.trim_end_matches("</suggest>")
		.replace("<br>", "<_PR_BR>");

	println!("{}", suggestions);
}

impl Conf {
	pub fn new() -> Option<Self> {
		let file_conf = config::load_config();
		let ai = file_conf.ai.unwrap_or_default();

		let key = std::env::var("_PR_AI_API_KEY")
			.ok()
			.or_else(|| option_env!("_DEF_PR_AI_API_KEY").map(|s| s.to_string()))
			.or(ai.api_key)
			.unwrap_or_default();
		if key.is_empty() {
			return None;
		}

		// Selects which LLM provider to talk to (see `provider.rs` for the
		// full list). Defaults to "openai", preserving the historical
		// behavior of talking to any OpenAI-compatible endpoint via `url`.
		let provider = std::env::var("_PR_AI_PROVIDER")
			.ok()
			.or_else(|| option_env!("_DEF_PR_AI_PROVIDER").map(|s| s.to_string()))
			.or(ai.provider)
			.unwrap_or_default();

		// Optional: overrides the provider's default base URL. Required for
		// custom OpenAI-compatible endpoints (self-hosted proxies, Ollama,
		// Groq, etc.), optional for providers with a well-known default.
		let url = std::env::var("_PR_AI_URL")
			.ok()
			.or_else(|| option_env!("_DEF_PR_AI_URL").map(|s| s.to_string()))
			.or(ai.url)
			.filter(|s| !s.is_empty());

		let model = std::env::var("_PR_AI_MODEL")
			.ok()
			.or_else(|| option_env!("_DEF_PR_AI_MODEL").map(|s| s.to_string()))
			.or(ai.model)
			.unwrap_or_default();
		if model.is_empty() {
			return None;
		}

		// Raw JSON object merged into the request (e.g. temperature, or a
		// nested `extra_body` field for servers that expect one).
		let extra = std::env::var("_PR_AI_EXTRA")
			.ok()
			.or(ai.extra)
			.and_then(|s| serde_json::from_str::<Value>(&s).ok());

		Some(Conf {
			provider,
			key,
			url,
			model,
			extra,
		})
	}
}
