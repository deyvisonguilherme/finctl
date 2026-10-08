pub use crate::cli::CompletionsArgs;

pub fn handle_completions_command(args: CompletionsArgs, mut cmd: clap::Command) {
    clap_complete::generate(args.shell, &mut cmd, "finctl", &mut std::io::stdout());
}
