fn main() {
    let mut args: Vec<String> = std::env::args().collect();
    if args.len() == 1 || (args.len() > 1 && args[1] != "fmt" && args[1] != "format") {
        args.insert(1, "fmt".to_string());
    }
    forgen::cli::run_cli_with_args(&args);
}
