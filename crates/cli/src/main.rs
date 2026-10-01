use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "finctl",
    version,
    about = "Sistema de controle financeiro pessoal"
)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
    println!("finctl {}", env!("CARGO_PKG_VERSION"));
}
