// Interactive (and flag-driven) setup wizard for the AI module:
// `pay-respects login` execs this module with a `login` argument, which is
// caught in `main.rs` before the normal AI-suggestion flow runs.

use std::io::Write;

use colored::Colorize;
use pay_respects_select::select_simple;
use rig_core::prelude::{ModelListingClient, VerifyClient};

use crate::rig_client::{
	PROVIDERS, ZAI_CODING_URL, is_known_provider, is_oauth_provider, requires_no_auth,
	supports_listing,
};
use crate::{build_client, build_client_no_auth};

struct LoginArgs {
	provider: Option<String>,
	api_key: Option<String>,
	model: Option<String>,
	url: Option<String>,
	no_verify: bool,
	help: bool,
}

pub async fn run(args: &[String]) -> Result<(), String> {
	let opts = parse_login_args(args)?;
	if opts.help {
		print_help();
		return Ok(());
	}

	let oauth = opts
		.provider
		.as_deref()
		.map(is_oauth_provider)
		.unwrap_or(false);

	// When every core field is given via flags, run fully non-interactively
	// (e.g. for scripting/CI): don't prompt for anything not explicitly set
	// beyond what's actually required. Model is intentionally NOT part of
	// this: it's only prompted for when the provider supports fetching a
	// real model list (see below); otherwise it's left for a later
	// `pay-respects model` run instead of forcing manual entry here.
	let fully_specified = if oauth {
		opts.provider.is_some()
	} else {
		opts.provider.is_some() && opts.api_key.is_some()
	};

	let provider = match &opts.provider {
		Some(p) => {
			if !is_known_provider(p) {
				return Err(format!(
					"unknown provider '{p}'. Run `pay-respects login --help` for the list of supported providers."
				));
			}
			p.clone()
		}
		None => prompt_select_provider()?,
	};

	let oauth = is_oauth_provider(&provider);
	let no_auth = requires_no_auth(&provider);

	// --- Z.AI coding plan ---
	let url = if provider == "zai" && opts.url.is_none() && !fully_specified {
		if prompt_yes_no("Use Z.AI coding plan endpoint?", false)? {
			Some(ZAI_CODING_URL.to_string())
		} else {
			None
		}
	} else {
		opts.url.clone()
	};

	// --- API key / OAuth ---
	let (stored_key, oauth_verified) = if oauth {
		let placeholder = "chatgpt-oauth".to_string();

		if !opts.no_verify {
			println!("Starting ChatGPT OAuth login...");
			trigger_chatgpt_oauth().await?;
			println!("{}", "OAuth login successful!".green());
		}
		(placeholder, true)
	} else {
		let api_key = match &opts.api_key {
			Some(k) => k.clone(),
			None if no_auth => String::new(),
			None => prompt_api_key(&provider)?,
		};
		let stored_key = if api_key.is_empty() {
			provider.clone()
		} else {
			api_key.clone()
		};
		(stored_key, false)
	};

	// --- URL override ---
	let url = if oauth {
		None
	} else {
		match &url {
			Some(u) => Some(u.clone()),
			None if fully_specified => None,
			None => prompt_optional_url(&provider)?,
		}
	};

	// --- Verify credentials (non-OAuth only) ---
	if !opts.no_verify && !oauth_verified {
		print!("Verifying credentials... ");
		std::io::stdout().flush().ok();
		match verify(&provider, &stored_key, url.as_deref()).await {
			Ok(()) => println!("{}", "ok".green()),
			Err(e) => {
				println!("{}", "failed".red());
				eprintln!("  {e}");
				if !prompt_yes_no("Continue anyway and save this configuration?", false)? {
					return Err("aborted".to_string());
				}
			}
		}
	}

	// --- Select model ---
	// Only prompt when the provider supports fetching a real model list
	// from its API; otherwise, don't force manual entry here — leave the
	// model field as-is (or unset) and let the user run
	// `pay-respects model` once they know what they want to use.
	let model = match &opts.model {
		Some(m) => Some(m.clone()),
		None if oauth => Some(prompt_chatgpt_model()?),
		None if supports_listing(&provider) => {
			match list_models(&provider, &stored_key, url.as_deref()).await {
				Ok(models) if !models.is_empty() => Some(prompt_select_model(&models)?),
				Ok(_) => {
					eprintln!("(no models returned by the provider)");
					None
				}
				Err(e) => {
					eprintln!("Failed to fetch model list: {e}");
					None
				}
			}
		}
		None => None,
	};

	write_config(&provider, &stored_key, model.as_deref(), url.as_deref())?;

	let path = pay_respects_utils::files::user_config_path();
	println!("Configuration saved to {}", path.bold());
	if model.is_none() {
		println!(
			"No model set. Run {} to pick one.",
			"pay-respects model".bold()
		);
	}
	Ok(())
}

/// `pay-respects model [MODEL]` — switch model without re-running full login.
/// Reads the current `[ai]` config to reuse provider/key/url, then either
/// sets the model directly from the argument or lets the user pick.
pub async fn run_model(args: &[String]) -> Result<(), String> {
	let model_arg = args.iter().find(|a| !a.starts_with('-')).cloned();

	// Load current config to reuse provider/key/url
	let file_conf = crate::config::load_config();
	let ai = file_conf.ai.unwrap_or_default();

	let provider = ai.provider.unwrap_or_default();
	let key = ai.api_key.unwrap_or_default();
	let url = ai.url;

	if provider.is_empty() || key.is_empty() {
		return Err(
			"No AI provider configured. Run `pay-respects login` first.".to_string(),
		);
	}

	let oauth = is_oauth_provider(&provider);
	let model = if let Some(m) = model_arg {
		m
	} else if oauth {
		// ChatGPT: show known models
		prompt_chatgpt_model()?
	} else if supports_listing(&provider) {
		match list_models(&provider, &key, url.as_deref()).await {
			Ok(models) if !models.is_empty() => prompt_select_model(&models)?,
			Ok(_) => {
				eprintln!("(no models returned by the provider)");
				prompt_line("Enter model name/ID: ")?
			}
			Err(e) => {
				eprintln!("Failed to fetch model list: {e}");
				prompt_line("Enter model name/ID: ")?
			}
		}
	} else {
		prompt_line(&format!(
			"Enter model name/ID{}: ",
			ai.model.as_deref().map(|m| format!(" (current: {m})")).unwrap_or_default()
		))?
	};

	if model.trim().is_empty() {
		println!("No model given, keeping current configuration unchanged.");
		return Ok(());
	}

	// Only update the model field, preserve everything else
	write_config_model(&model)?;

	let path = pay_respects_utils::files::user_config_path();
	println!("Model set to {}", model.bold());
	println!("Config: {}", path);
	Ok(())
}

/// Updates only the `model` field in the `[ai]` section, preserving all
/// other fields and comments.
fn write_config_model(model: &str) -> Result<(), String> {
	let path = pay_respects_utils::files::user_config_path();

	let existing = std::fs::read_to_string(&path).unwrap_or_default();
	let mut doc = existing
		.parse::<toml_edit::DocumentMut>()
		.map_err(|e| format!("failed to parse config file: {e}"))?;

	if !doc.contains_table("ai") {
		doc["ai"] = toml_edit::table();
	}
	let ai = doc["ai"].as_table_mut().ok_or("`ai` is not a table")?;
	ai["model"] = toml_edit::value(model);

	if let Some(parent) = std::path::Path::new(&path).parent() {
		std::fs::create_dir_all(parent).map_err(|e| format!("failed to create directory: {e}"))?;
	}
	std::fs::write(&path, doc.to_string()).map_err(|e| format!("failed to write config: {e}"))
}

fn parse_login_args(args: &[String]) -> Result<LoginArgs, String> {
	let mut opts = LoginArgs {
		provider: None,
		api_key: None,
		model: None,
		url: None,
		no_verify: false,
		help: false,
	};

	let mut iter = args.iter().peekable();
	while let Some(arg) = iter.next() {
		match arg.as_str() {
			"-h" | "--help" => opts.help = true,
			"--no-verify" => opts.no_verify = true,
			"--provider" => {
				opts.provider = Some(
					iter.next()
						.ok_or("--provider requires an argument")?
						.clone(),
				)
			}
			"--api-key" => {
				opts.api_key = Some(iter.next().ok_or("--api-key requires an argument")?.clone())
			}
			"--model" => {
				opts.model = Some(iter.next().ok_or("--model requires an argument")?.clone())
			}
			"--url" => opts.url = Some(iter.next().ok_or("--url requires an argument")?.clone()),
			other => return Err(format!("unknown option: {other}")),
		}
	}
	Ok(opts)
}

fn print_help() {
	println!(
		r#"pay-respects login [OPTIONS]

Interactively configure the AI module (provider, API key, and model),
writing the result to the [ai] section of config.toml. Any option given
below is used as-is instead of being prompted for.

For OAuth providers (chatgpt), --api-key is not needed; the login flow
will open a browser for authentication instead.

Options:
	    --provider <PROVIDER>  AI provider to use
	    --api-key <KEY>        API key (not needed for chatgpt/llamafile)
	    --model <MODEL>        Model name/ID
	    --url <URL>            Override the provider's default base URL
	    --no-verify            Skip verifying the API key before saving
	-h, --help                 Print help

Supported providers:
"#
	);
	for (id, name) in PROVIDERS {
		println!("  {id:<12} {name}");
	}
}

fn prompt_select_provider() -> Result<String, String> {
	let items: Vec<String> = PROVIDERS
		.iter()
		.map(|(id, name)| format!("{name} ({id})"))
		.collect();
	let idx = select_simple("Select an AI provider:", &items)
		.map_err(|e| format!("selection failed: {e}"))?;
	Ok(PROVIDERS[idx].0.to_string())
}

fn prompt_select_model(models: &[rig_core::model::Model]) -> Result<String, String> {
	let mut items: Vec<String> = models
		.iter()
		.map(|m| {
			if m.display_name() == m.id {
				m.id.clone()
			} else {
				format!("{} ({})", m.display_name(), m.id)
			}
		})
		.collect();
	items.push("[Enter a model name/ID manually]".to_string());

	let idx =
		select_simple("Select a model:", &items).map_err(|e| format!("selection failed: {e}"))?;

	if idx == models.len() {
		prompt_line("Enter model name/ID: ")
	} else {
		Ok(models[idx].id.clone())
	}
}

fn prompt_chatgpt_model() -> Result<String, String> {
	let models = [
		"gpt-5.4",
		"gpt-5.4-pro",
		"gpt-5.3-codex",
		"gpt-5.3-codex-spark",
		"gpt-5.3-instant",
		"gpt-5.3-chat-latest",
	];
	let items: Vec<String> = models
		.iter()
		.map(|m| format!("{m} ({m})"))
		.chain(std::iter::once("[Enter a model name/ID manually]".to_string()))
		.collect();
	let idx =
		select_simple("Select a model:", &items).map_err(|e| format!("selection failed: {e}"))?;

	if idx == models.len() {
		prompt_line("Enter model name/ID: ")
	} else {
		Ok(models[idx].to_string())
	}
}

fn prompt_api_key(provider: &str) -> Result<String, String> {
	rpassword::prompt_password(format!("Enter API key for {provider}: "))
		.map_err(|e| format!("failed to read API key: {e}"))
}

fn prompt_optional_url(provider: &str) -> Result<Option<String>, String> {
	if requires_no_auth(provider) {
		if prompt_yes_no(
			&format!("Use a custom URL for {provider} instead of its default?"),
			false,
		)? {
			return Ok(Some(prompt_line("Enter URL: ")?));
		}
		return Ok(None);
	}

	if prompt_yes_no(&format!("Use {provider}'s default endpoint?"), true)? {
		Ok(None)
	} else {
		Ok(Some(prompt_line("Enter URL: ")?))
	}
}

fn prompt_line(prompt: &str) -> Result<String, String> {
	eprint!("{prompt}");
	std::io::stderr().flush().ok();
	let mut input = String::new();
	std::io::stdin()
		.read_line(&mut input)
		.map_err(|e| format!("failed to read input: {e}"))?;
	Ok(input.trim().to_string())
}

fn prompt_yes_no(prompt: &str, default_yes: bool) -> Result<bool, String> {
	let hint = if default_yes { "[Y/n]" } else { "[y/N]" };
	let input = prompt_line(&format!("{prompt} {hint} "))?;
	Ok(match input.to_lowercase().as_str() {
		"" => default_yes,
		"y" | "yes" => true,
		_ => false,
	})
}

/// Triggers the ChatGPT OAuth device code flow. rig-core handles the full
/// flow internally (including token caching and refresh).
///
/// Do not use a completion request for this. Model availability is account
/// dependent, so a successful OAuth handshake can still fail with "model is
/// not supported" if we verify by prompting a hardcoded model.
async fn trigger_chatgpt_oauth() -> Result<(), String> {
	let client = rig_core::providers::chatgpt::Client::builder()
		.oauth()
		.on_device_code(|prompt| {
			println!(
				"\n  Open {} in your browser and enter code: {}\n",
				prompt.verification_uri.bold(),
				prompt.user_code.bold()
			);
		})
		.build()
		.map_err(|e| format!("failed to build ChatGPT client: {e}"))?;

	client
		.authorize()
		.await
		.map_err(|e| format!("ChatGPT OAuth failed: {e}"))
}

async fn verify(provider: &str, key: &str, url: Option<&str>) -> Result<(), String> {
	macro_rules! v {
		($client:ty) => {{
			let client = build_client!($client, key, url)?;
			client.verify().await.map_err(|e| e.to_string())
		}};
	}
	macro_rules! v_no_auth {
		($client:ty) => {{
			let client = build_client_no_auth!($client, url)?;
			client.verify().await.map_err(|e| e.to_string())
		}};
	}

	match provider {
		"" | "openai" => v!(rig_core::providers::openai::Client),
		"anthropic" => v!(rig_core::providers::anthropic::Client),
		"chatgpt" => {
			// ChatGPT uses OAuth, verify() doesn't apply. OAuth was already
			// triggered in trigger_chatgpt_oauth() above.
			Ok(())
		}
		"gemini" => v!(rig_core::providers::gemini::Client),
		"mistral" => v!(rig_core::providers::mistral::Client),
		"cohere" => v!(rig_core::providers::cohere::Client),
		"xai" => v!(rig_core::providers::xai::Client),
		"deepseek" => v!(rig_core::providers::deepseek::Client),
		"perplexity" => v!(rig_core::providers::perplexity::Client),
		"together" => Err("verification is not supported for this provider".to_string()),
		"groq" => v!(rig_core::providers::groq::Client),
		"openrouter" => v!(rig_core::providers::openrouter::Client),
		"huggingface" => v!(rig_core::providers::huggingface::Client),
		"hyperbolic" => v!(rig_core::providers::hyperbolic::Client),
		"ollama" => v!(rig_core::providers::ollama::Client),
		"llamafile" => v_no_auth!(rig_core::providers::llamafile::Client),
		"moonshot" => v!(rig_core::providers::moonshot::Client),
		"minimax" => v!(rig_core::providers::minimax::Client),
		"xiaomimimo" => v!(rig_core::providers::xiaomimimo::Client),
		"zai" => v!(rig_core::providers::zai::Client),
		"galadriel" => v!(rig_core::providers::galadriel::Client),
		"mira" => v!(rig_core::providers::mira::Client),
		other => Err(format!("unknown AI provider: '{other}'")),
	}
}

async fn list_models(
	provider: &str,
	key: &str,
	url: Option<&str>,
) -> Result<Vec<rig_core::model::Model>, String> {
	macro_rules! l {
		($client:ty) => {{
			let client = build_client!($client, key, url)?;
			let models = client
				.list_models()
				.await
				.map_err(|e| format!("failed to list models: {e}"))?;
			Ok(models.iter().cloned().collect())
		}};
	}

	match provider {
		"" | "openai" => l!(rig_core::providers::openai::Client),
		"anthropic" => l!(rig_core::providers::anthropic::Client),
		"deepseek" => l!(rig_core::providers::deepseek::Client),
		"mistral" => l!(rig_core::providers::mistral::Client),
		"gemini" => l!(rig_core::providers::gemini::Client),
		"ollama" => l!(rig_core::providers::ollama::Client),
		"openrouter" => l!(rig_core::providers::openrouter::Client),
		"xiaomimimo" => l!(rig_core::providers::xiaomimimo::Client),
		other => Err(format!("provider '{other}' does not support model listing")),
	}
}

/// Writes provider/api_key/url unconditionally; `model` is only written
/// when `Some` — when `None`, whatever model value is already in the
/// config file (if any) is left untouched, rather than being blanked out.
fn write_config(
	provider: &str,
	key: &str,
	model: Option<&str>,
	url: Option<&str>,
) -> Result<(), String> {
	let path = pay_respects_utils::files::user_config_path();

	let existing = std::fs::read_to_string(&path).unwrap_or_default();
	let mut doc = existing
		.parse::<toml_edit::DocumentMut>()
		.map_err(|e| format!("failed to parse existing config file: {e}"))?;

	if !doc.contains_table("ai") {
		doc["ai"] = toml_edit::table();
	}
	let ai = doc["ai"].as_table_mut().ok_or("`ai` is not a table")?;

	ai["provider"] = toml_edit::value(provider);
	ai["api_key"] = toml_edit::value(key);
	if let Some(model) = model {
		ai["model"] = toml_edit::value(model);
	}
	if let Some(url) = url {
		ai["url"] = toml_edit::value(url);
	} else {
		ai.remove("url");
	}

	if let Some(parent) = std::path::Path::new(&path).parent() {
		std::fs::create_dir_all(parent).map_err(|e| format!("failed to create directory: {e}"))?;
	}
	std::fs::write(&path, doc.to_string()).map_err(|e| format!("failed to write config: {e}"))
}
