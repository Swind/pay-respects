use crate::{init::Init, shell::initialization};
use colored::Colorize;
use pay_respects_utils::files::{config_files, get_path_files, user_config_path};
use pay_respects_utils::strings::print_error;

pub enum Status {
	Continue,
	Exit, // version, help, etc.
	Error,
}

pub fn handle_args(args: impl IntoIterator<Item = String>) -> Status {
	let mut iter = args.into_iter().peekable();
	let mut init = Init::new();
	if let Some(binary_path) = iter.next() {
		init.binary_path = binary_path;
	}

	if iter.peek().is_none() {
		return Status::Continue;
	}

	while let Some(arg) = iter.next() {
		match arg.as_str() {
			"-h" | "--help" => {
				print_help();
				return Status::Exit;
			}
			"-v" | "--version" => {
				print_version();
				return Status::Exit;
			}
			"init-config" => {
				init_config();
				return Status::Exit;
			}
			"info" => {
				print_info();
				return Status::Exit;
			}
			"-a" | "--alias" => match iter.peek() {
				Some(next_arg) if !next_arg.starts_with('-') => {
					init.alias = next_arg.to_string();
					iter.next();
				}
				_ => init.alias = String::from("f"),
			},
			"--nocnf" => init.cnf = false,
			"-P" | "--prompt-prefix" => match iter.peek() {
				Some(next_arg) if !next_arg.starts_with('-') => {
					init.cmd_prefix = Some(next_arg.to_string());
					iter.next();
				}
				_ => {
					print_error("--prompt-prefix requires argument.");
					return Status::Error;
				}
			},
			_ => {
				if arg.starts_with('-') {
					print_error(&format!("Unknown option: {}", arg));
					return Status::Error;
				} else {
					init.shell = arg
				}
			}
		}
	}

	if init.shell.is_empty() {
		eprintln!("{}", t!("no-shell"));
		return Status::Error;
	}

	initialization(&mut init);
	Status::Exit
}

fn print_help() {
	let init = "pay-respects <shell> [OPTIONS]";
	let options = r#"Options:
	-h, --help                   Print help information
	-v, --version                Print version information
	-a, --alias [<alias>]        Set alias for the function (default: f)
	    --nocnf                  Do not load cnf file
	-P, --prompt-prefix <prefix> Force a prompt prefix (e.g. ">" or "❯")

Commands:
	init-config                  Create a default config file
	info                         Show current configuration and loaded modules
"#;
	println!(
		"{}",
		t!(
			"help",
			usage = format!("{}\n\n{}", init, options),
			eval = "Bash / Zsh / Fish".bold().to_string(),
			eval_examples = r#"
eval "$(pay-respects bash)"
eval "$(pay-respects zsh)"
pay-respects fish | source
"#,
			manual = "Nushell / PowerShell".bold().to_string(),
			manual_examples = r#"
pay-respects nu
pay-respects pwsh
"#
		)
	);
}

fn print_version() {
	println!(
		"version: {}",
		option_env!("CARGO_PKG_VERSION").unwrap_or("unknown")
	);
	let lib = option_env!("_DEF_PR_LIB").map(|dir| dir.to_string());
	if let Some(lib) = lib {
		println!("Default lib directory: {}", lib);
	}
	let package_manager = option_env!("_DEF_PR_PACKAGE_MANAGER").map(|dir| dir.to_string());
	if let Some(package_manager) = package_manager {
		println!("Default package manager: {}", package_manager);
	}
}

fn init_config() {
	let path = user_config_path();
	let path_display = path.bold();

	if std::path::Path::new(&path).exists() {
		eprint!(
			"Config file already exists at {}\nOverwrite? [y/N] ",
			path_display
		);
		let mut input = String::new();
		std::io::stdin().read_line(&mut input).unwrap();
		let input = input.trim();
		if input != "y" && input != "Y" {
			println!("Aborted.");
			return;
		}
	}

	let default_config = r#"# pay-respects configuration file
# See https://github.com/Swind/pay-respects/blob/main/config.md for all options

# Preferred command for privileged access
# privilege = "sudo"

# Maximum time in milliseconds for getting previous output
# timeout = 3000

# Apply existing rules to a set of commands
# merge_commands = [
# 	["ls", "exa"],
# 	["grep", "rg"],
# ]

# Commands that won't return any message when run
# blocking_commands = ["vim", "nano"]

# How suggestions are evaluated after being confirmed ("Internal" or "Shell")
# eval_method = "Internal"

# Algorithm for fuzzy searching ("TrigramDamerauLevenshtein" or "DamerauLevenshtein")
# search_type = "TrigramDamerauLevenshtein"

# Minimum characters required to start searching
# search_threshold = 3

# [trigram]
# minimum_score = 0.5

# [dl_distance]
# percentage = 27.18
# max = 5
# min = 1

# [package_manager]
# package_manager = "pacman"
# install_method = "System"

# AI module settings (requires _pay-respects-fallback-100-request-ai)
# [ai]
# url = "https://api.openai.com/v1/chat/completions"
# api_key = "your-api-key"
# model = "gpt-4o"
# additional_prompt = ""
# locale = ""
"#;

	// Create parent directory if needed
	if let Some(parent) = std::path::Path::new(&path).parent() {
		if let Err(e) = std::fs::create_dir_all(parent) {
			print_error(&format!("Failed to create directory {}: {}", parent.display(), e));
			return;
		}
	}

	match std::fs::write(&path, default_config) {
		Ok(_) => println!("Config file created at {}", path_display),
		Err(e) => print_error(&format!("Failed to write config file: {}", e)),
	}
}

fn print_info() {
	let section = |title: &str| println!("\n{}", title.bold().underline());

	// Version
	section("Version");
	println!(
		"  {}",
		option_env!("CARGO_PKG_VERSION").unwrap_or("unknown")
	);

	// Built-in rules
	section("Built-in Rules");
	let total = option_env!("_BUILTIN_RULES_TOTAL").unwrap_or("?");
	let command = option_env!("_BUILTIN_RULES_COMMAND").unwrap_or("?");
	println!("  Total:            {}", total);
	println!("  Command-specific: {}", command);
	println!(
		"  Special (_PR_*):  {}",
		total
			.parse::<usize>()
			.ok()
			.zip(command.parse::<usize>().ok())
			.map(|(t, c)| (t - c).to_string())
			.unwrap_or_else(|| "?".to_string())
	);

	// Config files
	section("Config Files");
	let loaded = config_files();
	if loaded.is_empty() {
		println!("  (none loaded)");
		println!("  Default path: {}", user_config_path().dimmed());
	} else {
		for file in &loaded {
			println!("  {}", file.green());
		}
	}

	// Modules & fallbacks from PATH
	section("Modules");
	let path_files = get_path_files();
	let modules: Vec<_> = path_files
		.iter()
		.filter(|f| f.starts_with("_pay-respects-module-"))
		.collect();
	let fallbacks: Vec<_> = path_files
		.iter()
		.filter(|f| f.starts_with("_pay-respects-fallback-"))
		.collect();

	if modules.is_empty() {
		println!("  Runtime modules: (none)");
	} else {
		println!("  Runtime modules:");
		for m in &modules {
			println!("    {}", m);
		}
	}

	if fallbacks.is_empty() {
		println!("  Fallback modules: (none)");
	} else {
		println!("  Fallback modules:");
		for f in &fallbacks {
			println!("    {}", f);
		}
	}

	println!();
}

#[cfg(test)]
mod tests {
	use super::{Status, handle_args};

	#[test]
	fn test_handle_args() {
		assert!(matches!(
			handle_args([String::from("pay-respects")]),
			Status::Continue
		));

		for args in [
			[String::new(), String::from("-h")],
			[String::new(), String::from("--help")],
			[String::new(), String::from("-v")],
			[String::new(), String::from("--version")],
			[String::new(), String::from("zsh")],
		] {
			println!("Arguments {:?} should return Exit", args);
			assert!(matches!(handle_args(args), Status::Exit));
		}

		for args in [
			[String::new(), String::from("fish"), String::from("--alias")],
			[String::new(), String::from("bash"), String::from("--nocnf")],
		] {
			println!("Arguments {:?} should return Exit", args);
			assert!(matches!(handle_args(args), Status::Exit));
		}

		for args in [
			[String::new(), String::from("-a")],
			[String::new(), String::from("--alias")],
			[String::new(), String::from("--nocnf")],
		] {
			println!("Arguments {:?} should return Error", args);
			assert!(matches!(handle_args(args), Status::Error));
		}

		for args in [
			[String::new(), String::from("-a"), String::from("--nocnf")],
			[
				String::new(),
				String::from("--alias"),
				String::from("--nocnf"),
			],
		] {
			println!("Argument {:?} should return Error", args);
			assert!(matches!(handle_args(args), Status::Error));
		}
	}
}
