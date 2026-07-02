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
			"login" => {
				let rest: Vec<String> = iter.collect();
				return run_login(&rest);
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
	login                        Configure an AI provider (requires the AI module)
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
# See module-request-ai/README.md for the full list of supported providers.
# [ai]
# provider = "openai"
# url = "https://api.openai.com/v1"
# api_key = "your-api-key"
# model = "gpt-4o"
# additional_prompt = ""
# locale = ""
# extra = '{"temperature":0.5}'
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

/// Locates the `_pay-respects-fallback-*request-ai*` binary, searching
/// `$_PR_LIB`/the compile-time default lib dir first, falling back to a
/// `$PATH` scan. Returns an absolute path when found via the lib dir, or a
/// bare executable name (resolvable through `$PATH`) otherwise.
fn find_fallback_ai_binary() -> Option<String> {
	let is_match = |name: &str| {
		name.starts_with("_pay-respects-fallback-")
			&& name.contains("request-ai")
			// module binaries never contain a literal `.` in their name;
			// this also filters out stray build artifacts (e.g. `*.d`
			// cargo dep-info files) that can end up alongside dev builds.
			&& !name.contains('.')
	};

	let lib_dir = std::env::var("_PR_LIB")
		.ok()
		.or_else(|| option_env!("_DEF_PR_LIB").map(|s| s.to_string()));

	if let Some(lib_dir) = lib_dir {
		for p in lib_dir.split(pay_respects_utils::files::path_env_sep()) {
			#[cfg(windows)]
			let p = pay_respects_utils::files::path_convert(p);

			let Ok(files) = std::fs::read_dir(p) else {
				continue;
			};
			for file in files.flatten() {
				let name = file.file_name().to_string_lossy().to_string();
				if is_match(&name) {
					return Some(file.path().to_string_lossy().to_string());
				}
			}
		}
		None
	} else {
		get_path_files().into_iter().find(|f| is_match(f))
	}
}

/// Execs the AI module binary with a `login` argument, forwarding any
/// additional CLI args (e.g. `--provider`, `--api-key`) and inheriting
/// stdio for the fully-interactive setup wizard.
fn run_login(args: &[String]) -> Status {
	let bin = match find_fallback_ai_binary() {
		Some(bin) => bin,
		None => {
			print_error(
				"AI module (_pay-respects-fallback-100-request-ai) not found. Is it installed and in $PATH (or $_PR_LIB)?",
			);
			return Status::Error;
		}
	};

	let status = std::process::Command::new(&bin).arg("login").args(args).status();

	match status {
		Ok(status) if status.success() => Status::Exit,
		Ok(_) => Status::Error,
		Err(e) => {
			print_error(&format!("Failed to run AI module: {}", e));
			Status::Error
		}
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
	let command_names = option_env!("_BUILTIN_RULES_COMMAND_NAMES").unwrap_or("");
	let special_names = option_env!("_BUILTIN_RULES_SPECIAL_NAMES").unwrap_or("");
	if !command_names.is_empty() {
		println!("  Command-specific: {}", command_names.replace(',', ", "));
	}
	if !special_names.is_empty() {
		println!("  Special (_PR_*):  {}", special_names.replace(',', ", "));
	}

	// Config files
	section("Config Files");
	let loaded = config_files();
	if loaded.is_empty() {
		println!("  (none)");
		println!(
			"  Run {} to create one",
			"pay-respects init-config".bold()
		);
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
