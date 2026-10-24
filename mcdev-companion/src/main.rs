fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let input = std::io::BufReader::new(std::io::stdin());
    let output = std::io::stdout();
    let result = match args.as_slice() {
        [] => mcdev_companion::run(input, output),
        [mode] if mode == "--stdio-completion" => {
            mcdev_companion::completion_lines::run_completion_lines_with(
                input,
                output,
                &mcdev_companion::completion_lines::complete_no_index,
            )
        }
        _ => {
            eprintln!("usage: mcdev-companion [--stdio-completion]");
            std::process::exit(2);
        }
    };
    if let Err(error) = result {
        eprintln!("mcdev-companion: {error}");
        std::process::exit(1);
    }
}
