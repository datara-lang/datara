fn main() {
    let mut args: Vec<String> = std::env::args().collect();
    if args.len() == 1
        || (args.len() > 1
            && args[1] != "lsp"
            && args[1] != "--version"
            && args[1] != "-v"
            && args[1] != "--help"
            && args[1] != "-h")
    {
        args.insert(1, "lsp".to_string());
    }
    forgen::cli::run_cli_with_args(&args);
}
