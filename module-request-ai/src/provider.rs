// Dispatches AI completion requests to the appropriate LLM provider via the
// `rig-core` crate. This lets pay-respects talk to a provider's native API
// (Anthropic, Gemini, etc.) instead of being limited to OpenAI-compatible
// endpoints only.

use futures::StreamExt;
use rig_core::agent::{MultiTurnStreamItem, StreamingResult};
use rig_core::prelude::CompletionClient;
use rig_core::streaming::{StreamedAssistantContent, StreamingPrompt};
use serde_json::Value;

use crate::buffer::Buffer;
use crate::{build_client, build_client_no_auth};

/// Builds a client/agent for `$client` (a `rig_core::providers::*::Client`
/// type path), streams the prompt, and feeds the response into `buffer`.
macro_rules! run_provider {
	($client:ty, $key:expr, $url:expr, $model:expr, $extra:expr, $prompt:expr, $buffer:expr) => {{
		let client = build_client!($client, $key, $url)?;

		let mut agent_builder = client.agent($model.to_string());
		if let Some(extra) = $extra.clone() {
			agent_builder = agent_builder.additional_params(extra);
		}
		let agent = agent_builder.build();

		let mut stream = agent.stream_prompt($prompt.clone()).await;
		consume_stream(&mut stream, $buffer).await
	}};
}

/// Like [`run_provider`], but for providers that don't support API key
/// authentication at all (e.g. Llamafile, which is a purely local runner).
macro_rules! run_provider_no_auth {
	($client:ty, $url:expr, $model:expr, $extra:expr, $prompt:expr, $buffer:expr) => {{
		let client = build_client_no_auth!($client, $url)?;

		let mut agent_builder = client.agent($model.to_string());
		if let Some(extra) = $extra.clone() {
			agent_builder = agent_builder.additional_params(extra);
		}
		let agent = agent_builder.build();

		let mut stream = agent.stream_prompt($prompt.clone()).await;
		consume_stream(&mut stream, $buffer).await
	}};
}

/// Streams a completion from `provider` and writes the response into `buffer`.
///
/// `url` overrides the provider's default base URL, which is useful for
/// self-hosted proxies, regional endpoints, or local servers (Ollama,
/// Llamafile).
#[allow(clippy::too_many_arguments)]
pub async fn stream_completion(
	provider: &str,
	key: &str,
	url: Option<&str>,
	model: &str,
	extra: Option<Value>,
	prompt: String,
	buffer: &mut Buffer,
) -> Result<(), String> {
	match provider {
		"" | "openai" => {
			run_provider!(
				rig_core::providers::openai::Client,
				key,
				url,
				model,
				extra,
				prompt,
				buffer
			)
		}
		"anthropic" => {
			run_provider!(
				rig_core::providers::anthropic::Client,
				key,
				url,
				model,
				extra,
				prompt,
				buffer
			)
		}
		"gemini" => {
			run_provider!(
				rig_core::providers::gemini::Client,
				key,
				url,
				model,
				extra,
				prompt,
				buffer
			)
		}
		"mistral" => {
			run_provider!(
				rig_core::providers::mistral::Client,
				key,
				url,
				model,
				extra,
				prompt,
				buffer
			)
		}
		"cohere" => {
			run_provider!(
				rig_core::providers::cohere::Client,
				key,
				url,
				model,
				extra,
				prompt,
				buffer
			)
		}
		"xai" => {
			run_provider!(
				rig_core::providers::xai::Client,
				key,
				url,
				model,
				extra,
				prompt,
				buffer
			)
		}
		"deepseek" => {
			run_provider!(
				rig_core::providers::deepseek::Client,
				key,
				url,
				model,
				extra,
				prompt,
				buffer
			)
		}
		"perplexity" => {
			run_provider!(
				rig_core::providers::perplexity::Client,
				key,
				url,
				model,
				extra,
				prompt,
				buffer
			)
		}
		"together" => {
			run_provider!(
				rig_core::providers::together::Client,
				key,
				url,
				model,
				extra,
				prompt,
				buffer
			)
		}
		"groq" => {
			run_provider!(
				rig_core::providers::groq::Client,
				key,
				url,
				model,
				extra,
				prompt,
				buffer
			)
		}
		"openrouter" => {
			run_provider!(
				rig_core::providers::openrouter::Client,
				key,
				url,
				model,
				extra,
				prompt,
				buffer
			)
		}
		"huggingface" => {
			run_provider!(
				rig_core::providers::huggingface::Client,
				key,
				url,
				model,
				extra,
				prompt,
				buffer
			)
		}
		"hyperbolic" => {
			run_provider!(
				rig_core::providers::hyperbolic::Client,
				key,
				url,
				model,
				extra,
				prompt,
				buffer
			)
		}
		"ollama" => {
			run_provider!(
				rig_core::providers::ollama::Client,
				key,
				url,
				model,
				extra,
				prompt,
				buffer
			)
		}
		"llamafile" => {
			run_provider_no_auth!(
				rig_core::providers::llamafile::Client,
				url,
				model,
				extra,
				prompt,
				buffer
			)
		}
		"moonshot" => {
			run_provider!(
				rig_core::providers::moonshot::Client,
				key,
				url,
				model,
				extra,
				prompt,
				buffer
			)
		}
		"minimax" => {
			run_provider!(
				rig_core::providers::minimax::Client,
				key,
				url,
				model,
				extra,
				prompt,
				buffer
			)
		}
		"xiaomimimo" => {
			run_provider!(
				rig_core::providers::xiaomimimo::Client,
				key,
				url,
				model,
				extra,
				prompt,
				buffer
			)
		}
		"zai" => {
			run_provider!(
				rig_core::providers::zai::Client,
				key,
				url,
				model,
				extra,
				prompt,
				buffer
			)
		}
		"galadriel" => {
			run_provider!(
				rig_core::providers::galadriel::Client,
				key,
				url,
				model,
				extra,
				prompt,
				buffer
			)
		}
		"mira" => {
			run_provider!(
				rig_core::providers::mira::Client,
				key,
				url,
				model,
				extra,
				prompt,
				buffer
			)
		}
		other => Err(format!(
			"unknown AI provider: '{other}'. See `pay-respects-module-request-ai` README for the list of supported providers."
		)),
	}
}

/// Consumes a rig streaming response, feeding text/reasoning deltas into
/// `buffer` as they arrive.
async fn consume_stream<R>(stream: &mut StreamingResult<R>, buffer: &mut Buffer) -> Result<(), String>
where
	R: Clone,
{
	while let Some(item) = stream.next().await {
		match item {
			Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(
				text,
			))) => {
				if buffer.is_reasoning() {
					buffer.finish_reasoning();
				}
				buffer.proc(&text.text);
			}
			Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Reasoning(
				reasoning,
			))) => {
				buffer.proc_reasoning(&reasoning.display_text());
			}
			Ok(MultiTurnStreamItem::StreamAssistantItem(
				StreamedAssistantContent::ReasoningDelta { reasoning, .. },
			)) => {
				buffer.proc_reasoning(&reasoning);
			}
			Ok(MultiTurnStreamItem::FinalResponse(_)) => break,
			Ok(_) => {
				// tool calls, etc. -- not applicable, no tools are registered
			}
			Err(e) => return Err(e.to_string()),
		}
	}
	if buffer.is_reasoning() {
		buffer.finish_reasoning();
	}
	Ok(())
}
