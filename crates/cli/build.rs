#[path = "src/cli.rs"]
mod cli;
#[path = "src/format.rs"]
mod format;

use clap::CommandFactory;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=src/cli.rs");
    println!("cargo:rerun-if-changed=src/format.rs");

    let profile = std::env::var("PROFILE").unwrap_or_default();
    if profile == "release" {
        let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
        let man_dir = out_dir.join("man");
        let _ = fs::create_dir_all(&man_dir);
        let cmd = cli::Cli::command();
        let _ = clap_mangen::generate_to(cmd, &man_dir);

        let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let target_man = manifest_dir.join("../../target/man");
        let _ = fs::create_dir_all(&target_man);
        let _ = clap_mangen::generate_to(cli::Cli::command(), &target_man);
    }
}
