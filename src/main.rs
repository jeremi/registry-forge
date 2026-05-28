use clap::Parser;

fn main() {
    let cli = registry_forge::Cli::parse();
    if let Err(err) = registry_forge::run(cli) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
