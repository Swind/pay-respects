fn main() {
	// recompile when rules are updated
	println!("cargo::rerun-if-changed=./rules/");

	let mut entries: Vec<String> = std::fs::read_dir("./rules/")
		.expect("Failed to read rules directory")
		.filter_map(|e| e.ok())
		.filter_map(|e| {
			let path = e.path();
			if path.extension().map(|x| x == "toml").unwrap_or(false) {
				path.file_stem()
					.and_then(|s| s.to_str())
					.map(|s| s.to_string())
			} else {
				None
			}
		})
		.collect();
	entries.sort_unstable();

	let special: Vec<&String> = entries.iter().filter(|n| n.starts_with("_PR_")).collect();
	let command: Vec<&String> = entries.iter().filter(|n| !n.starts_with("_PR_")).collect();

	println!("cargo::rustc-env=_BUILTIN_RULES_TOTAL={}", entries.len());
	println!(
		"cargo::rustc-env=_BUILTIN_RULES_COMMAND={}",
		command.len()
	);
	println!(
		"cargo::rustc-env=_BUILTIN_RULES_COMMAND_NAMES={}",
		command.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(",")
	);
	println!(
		"cargo::rustc-env=_BUILTIN_RULES_SPECIAL_NAMES={}",
		special.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(",")
	);
}
