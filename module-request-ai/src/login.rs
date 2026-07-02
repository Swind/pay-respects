// Interactive (and flag-driven) setup wizard for the AI module:
// `pay-respects login` execs this module with a `login` argument, which is
// caught in `main.rs` before the normal AI-suggestion flow runs.

use std::io::Write;

use colored::Colorize;
use pay_respects_select::select_simple;
use rig_core::prelude::{ModelListingClient, VerifyClient};

use crate::rig_client::{PROVIDERS, is_known_provider, requires_no_auth, supports_listing};
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

	// When every core field is given via flags, run fully non-interactively
	// (e.g. for scripting/CI): don't prompt for anything not explicitly set,
	// just fall back to sensible defaults (provider's default URL, etc.).
	let fully_specified =
		opts.provider.is_some() && opts.api_key.is_some() && opts.model.is_some();

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

	let no_auth = requires_no_auth(&provider);

	let api_key = match &opts.api_key {
		Some(k) => k.clone(),
		None if no_auth => String::new(),
		None => prompt_api_key(&provider)?,
	};
	// A non-empty placeholder is still stored for no-auth providers, since
	// the module treats an empty api_key as "AI disabled" (see requests.rs).
	let stored_key = if api_key.is_empty() {
		provider.clone()
	} else {
		api_key.clone()
	};

	let url = match &opts.url {
		Some(u) => Some(u.clone()),
		None if fully_specified => None,
		None => prompt_optional_url(&provider)?,
	};

	if !opts.no_verify {
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

	let model = match &opts.model {
		Some(m) => m.clone(),
		None if supports_listing(&provider) => {
			match list_models(&provider, &stored_key, url.as_deref()).await {
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
		}
		None => prompt_line("Enter model name/ID: ")?,
	};

	write_config(&provider, &stored_key, &model, url.as_deref())?;

	let path = pay_respects_utils::files::user_config_path();
	println!("Configuration saved to {}", path.bold());
	Ok(())
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

Options:
	    --provider <PROVIDER>  AI provider to use
	    --api-key <KEY>        API key
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

fn prompt_api_key(provider: &str) -> Result<String, String> {
	rpassword::prompt_password(format!("Enter API key for {provider}: "))
		.map_err(|e| format!("failed to read API key: {e}"))
}

fn prompt_optional_url(provider: &str) -> Result<Option<String>, String> {
	if requires_no_auth(provider) {
		// Local runners are commonly used with a non-default port/host.
		if prompt_yes_no(
			&format!("Use a custom URL for {provider} instead of its default?"),
			false,
		)? {
			return Ok(Some(prompt_line("Enter URL: ")?));
		}
		return Ok(None);
	}

	if prompt_yes_no(
		&format!("Use {provider}'s default endpoint?"),
		true,
	)? {
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
		"gemini" => v!(rig_core::providers::gemini::Client),
		"mistral" => v!(rig_core::providers::mistral::Client),
		"cohere" => v!(rig_core::providers::cohere::Client),
		"xai" => v!(rig_core::providers::xai::Client),
		"deepseek" => v!(rig_core::providers::deepseek::Client),
		"perplexity" => v!(rig_core::providers::perplexity::Client),
		// `TogetherExt` doesn't implement `DebugExt` in rig-core 0.39, so
		// `VerifyClient` isn't available for it. Treat as unverifiable
		// rather than failing to compile.
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

fn write_config(provider: &str, key: &str, model: &str, url: Option<&str>) -> Result<(), String> {
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
	ai["model"] = toml_edit::value(model);
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
