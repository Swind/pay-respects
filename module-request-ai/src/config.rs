use pay_respects_utils::files::config_files;
use pay_respects_utils::merge_option;
use pay_respects_utils::strings::print_error;
use serde::Deserialize;

#[derive(Deserialize, Default)]
pub struct ConfigReader {
	pub ai: Option<AiConfig>,
}

#[derive(Deserialize, Default, Clone)]
pub struct AiConfig {
	pub provider: Option<String>,
	pub url: Option<String>,
	pub api_key: Option<String>,
	pub model: Option<String>,
	pub additional_prompt: Option<String>,
	pub locale: Option<String>,
	/// Raw JSON object merged into the request (e.g. `{"temperature":0.5}`).
	/// For providers/proxies that expect a nested `extra_body` field, nest it
	/// yourself, e.g. `{"extra_body":{"chat_template_kwargs":{...}}}`.
	pub extra: Option<String>,
}

#[derive(Default)]
pub struct Config {
	pub ai: Option<AiConfig>,
}

impl Config {
	pub fn merge(&mut self, reader: ConfigReader) {
		merge_option!(self, reader, ai);
	}
}

pub fn load_config() -> Config {
	let mut config = Config::default();

	for file in config_files() {
		let content = std::fs::read_to_string(&file).expect("Failed to read config file");
		let reader: ConfigReader = toml::from_str(&content).unwrap_or_else(|_| {
			print_error(&format!(
				"Failed to parse config file at {}. Skipping.",
				file
			));
			ConfigReader::default()
		});
		config.merge(reader);
	}
	config
}
