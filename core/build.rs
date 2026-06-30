fn main() {
	// recompile when rules are updated
	println!("cargo::rerun-if-changed=./rules/");

	// count built-in rules (excluding special _PR_* rules)
	let total = std::fs::read_dir("./rules/")
		.expect("Failed to read rules directory")
		.filter_map(|e| e.ok())
		.filter(|e| {
			e.path()
				.extension()
				.map(|ext| ext == "toml")
				.unwrap_or(false)
		})
		.count();

	let special = std::fs::read_dir("./rules/")
		.expect("Failed to read rules directory")
		.filter_map(|e| e.ok())
		.filter(|e| {
			e.path()
				.file_name()
				.and_then(|n| n.to_str())
				.map(|n| n.starts_with("_PR_") && n.ends_with(".toml"))
				.unwrap_or(false)
		})
		.count();

	println!("cargo::rustc-env=_BUILTIN_RULES_TOTAL={}", total);
	println!(
		"cargo::rustc-env=_BUILTIN_RULES_COMMAND={}",
		total - special
	);
}
