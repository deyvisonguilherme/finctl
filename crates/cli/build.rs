#[path = "src/cli.rs"]
mod cli;
#[path = "src/format.rs"]
mod format;

use clap::CommandFactory;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=src/cli.rs");
    println!("cargo:rerun-if-changed=src/format.rs");
    println!("cargo:rerun-if-changed=../../.git/HEAD");

    let pkg_version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.2.0".to_string());

    let git_commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
            } else {
                None
            }
        });

    let git_date = Command::new("git")
        .args(["log", "-1", "--format=%cs"])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
            } else {
                None
            }
        });

    let full_version = match (git_commit, git_date) {
        (Some(commit), Some(date)) if !commit.is_empty() && !date.is_empty() => {
            format!("{pkg_version} ({commit} {date})")
        }
        (Some(commit), _) if !commit.is_empty() => {
            format!("{pkg_version} ({commit})")
        }
        _ => pkg_version,
    };

    println!("cargo:rustc-env=FINCTL_VERSION={}", full_version);

    let profile = std::env::var("PROFILE").unwrap_or_default();
    if profile == "release" {
        let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let target_dir = manifest_dir.join("../../target");

        let target_man = target_dir.join("man");
        let _ = fs::create_dir_all(&target_man);
        let _ = clap_mangen::generate_to(cli::Cli::command(), &target_man);

        let target_completions = target_dir.join("completions");
        let _ = fs::create_dir_all(&target_completions);
        let mut cmd = cli::Cli::command();
        let _ = clap_complete::generate_to(
            clap_complete::shells::Bash,
            &mut cmd,
            "finctl",
            &target_completions,
        );
        let _ = clap_complete::generate_to(
            clap_complete::shells::Zsh,
            &mut cmd,
            "finctl",
            &target_completions,
        );
        let _ = clap_complete::generate_to(
            clap_complete::shells::Fish,
            &mut cmd,
            "finctl",
            &target_completions,
        );
        let _ = clap_complete::generate_to(
            clap_complete::shells::PowerShell,
            &mut cmd,
            "finctl",
            &target_completions,
        );
        let _ = clap_complete::generate_to(
            clap_complete::shells::Elvish,
            &mut cmd,
            "finctl",
            &target_completions,
        );
    }
}
