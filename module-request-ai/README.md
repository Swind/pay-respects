# Request AI Module

Module for [pay-respects](https://github.com/Swind/pay-respects) to request AI
suggestions when no built-in rule matches. Powered by
[`rig-core`](https://github.com/0xplaygrounds/rig), which lets this module
talk to a provider's native API directly (not just OpenAI-compatible
endpoints).

This module does nothing unless it is explicitly configured: there is no
built-in default API key, URL, or model. See [Configuration](#configuration)
below.

> **Migrating from older versions:** `url` used to be the full chat
> completions endpoint (e.g. `https://api.openai.com/v1/chat/completions`).
> It is now the provider's API root instead (e.g. `https://api.openai.com/v1`),
> since the specific path is now built internally. Drop the trailing
> `/chat/completions` from your existing `url` if you have one configured.
> `_PR_AI_EXTRA_BODY` was also removed; merge its content into `_PR_AI_EXTRA`
> (or the config file's `extra` field) instead.

## Quick setup: `pay-respects login`

The easiest way to configure this module is the interactive setup wizard:
```sh
pay-respects login
```
This walks you through selecting a provider, entering an API key, and
optionally a custom URL, then verifies the credentials and writes the
`[ai]` section of `config.toml` for you (preserving the rest of the file).

If the provider supports fetching a model list from its API (see
[Supported providers](#supported-providers)), you'll also be asked to pick
one. Otherwise, `login` leaves the model unset — run `pay-respects model`
afterward to finish setup:
```sh
pay-respects model              # interactive: pick from a list, or type one
pay-respects model gpt-4o-mini  # set directly
```
This reuses the provider/API key/URL already in your config and only
updates the `model` field (leaving everything else untouched, including an
existing model if you don't provide a new one and listing fails).

It also accepts flags for non-interactive/scripted use:
```sh
pay-respects login --provider openai --api-key sk-... --model gpt-4o
pay-respects login --help   # full list of flags
```

## Configuration

Configuration can be set via the `[ai]` section of
[`config.toml`](../config.md), or overridden with environment variables.
Environment variables always take priority over the config file.

```toml
# ~/.config/pay-respects/config.toml
[ai]
provider = "openai"                                   # optional, defaults to "openai"
api_key = "sk-..."
model = "gpt-4o"
url = "https://api.openai.com/v1"                     # optional, overrides the provider's default URL
```

| Config field        | Env var                      | Description |
|---|---|---|
| `provider`          | `_PR_AI_PROVIDER`             | Which LLM provider to use. See [Supported providers](#supported-providers). Defaults to `openai`. |
| `api_key`           | `_PR_AI_API_KEY`               | Your API key. |
| `model`             | `_PR_AI_MODEL`                 | Model name/ID. Reasoning models are supported. |
| `url`               | `_PR_AI_URL`                    | Overrides the provider's default base URL. Required for self-hosted/custom OpenAI-compatible endpoints (see examples below); optional for hosted providers with a well-known endpoint. |
| `additional_prompt` | `_PR_AI_ADDITIONAL_PROMPT`      | Additional prompt/context to include. (Yes, you can include role-playing prompts you pervert) |
| `locale`            | `_PR_AI_LOCALE`                 | Locale in which the AI explains the suggestion. Defaults to your system locale. |
| `extra`             | `_PR_AI_EXTRA`                  | Raw JSON object merged into the request, e.g. `{"temperature":0.5}`. For servers expecting a nested `extra_body` field, nest it yourself: `{"extra_body":{"chat_template_kwargs":{"enable_thinking":false}}}`. |
| —                   | `_PR_AI_DISABLE`                | Setting to any value disables AI integration. |

Compile-time variables set the default value for their respective variable
above, when neither the env var nor the config file set it:
`_DEF_PR_AI_PROVIDER`, `_DEF_PR_AI_API_KEY`, `_DEF_PR_AI_URL`, `_DEF_PR_AI_MODEL`.

### Examples

Custom OpenAI-compatible endpoint (Groq, self-hosted proxy, etc.), the
default `provider`:
```toml
[ai]
api_key = "gsk_..."
model = "llama-3.3-70b-versatile"
url = "https://api.groq.com/openai/v1"
```

Anthropic, using its native API directly (not OpenAI-compatible, only
possible thanks to `rig-core`):
```toml
[ai]
provider = "anthropic"
api_key = "sk-ant-..."
model = "claude-sonnet-4-6"
```

Local Ollama:
```toml
[ai]
provider = "ollama"
api_key = "ollama"     # any non-empty placeholder; Ollama itself needs no key
model = "llama3"
# url = "http://localhost:11434"   # default, only needed for remote Ollama
```

## Supported providers

Set `provider` to one of the following (default: `openai`):

`openai`, `anthropic`, `chatgpt`, `gemini`, `mistral`, `cohere`, `xai`,
`deepseek`, `perplexity`, `together`, `groq`, `openrouter`, `huggingface`,
`hyperbolic`, `ollama`, `llamafile`, `moonshot`, `minimax`, `xiaomimimo`,
`zai`, `galadriel`, `mira`.

Any of these can also be pointed at a self-hosted/proxied endpoint using
`url`, as long as it speaks that provider's native API.

Not currently supported: Azure OpenAI (needs extra deployment/API-version
config).

**ChatGPT (subscription, OAuth):** The `chatgpt` provider uses a ChatGPT
Plus/Pro subscription via OAuth device code flow — no API key needed.
`pay-respects login` will open a browser for you to authenticate. The
OAuth token is cached and auto-refreshed by rig-core in
`~/.config/chatgpt/auth.json`.

**Z.AI coding plan:** When you select `zai` during `pay-respects login`,
you'll be asked whether to use the coding plan endpoint. You can also
set it manually:
```toml
[ai]
provider = "zai"
api_key = "..."
model = "glm-4.6"
url = "https://api.z.ai/api/coding/paas/v4"
```

Model listing (used by `pay-respects login` to let you pick a model from a
list) is supported for: `openai`, `anthropic`, `deepseek`, `mistral`,
`gemini`, `ollama`, `openrouter`, `xiaomimimo`. For all other providers,
`login` will ask you to type the model name/ID directly.

Credential verification (used by `pay-respects login` before saving) works
for all providers above except `together` (rig-core limitation) and
`chatgpt` (OAuth completion itself serves as verification).

## Reasoning models

Reasoning/thinking output is supported two ways:

- Natively, when the provider exposes a structured reasoning channel (e.g.
  DeepSeek Reasoner, extended thinking, etc.) — handled automatically.
- Via a literal `<think>...</think>` block in the response text, for models
  that emit reasoning this way instead (common with some open-weight models
  served through Ollama/vLLM).

## Advanced Usages

For non-trivial suggestions, you can add more context as comments (for Bash and Zsh, interactive comments needs to be explicitly enabled):
```sh
rustup # how do I setup nightly toolchain?
```
Or just a comment:
```sh
# git cache credential for 3 hours
```
