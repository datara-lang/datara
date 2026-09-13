fn main() {
    let mut args: Vec<String> = std::env::args().collect();
    if args.len() == 1
        || (args.len() > 1 && args[1] != "lint" && args[1] != "clippy" && args[1] != "audit")
    {
        args.insert(1, "clippy".to_string());
    }
    forgen::cli::run_cli_with_args(&args);
}
