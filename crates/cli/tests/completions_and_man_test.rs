use clap_complete::Shell;
use clap_mangen::Man;
use finctl_cli::build_cli;
use std::fs;
use std::process::Command;

#[test]
fn test_all_commands_and_arguments_have_help_descriptions() {
    let cmd = build_cli();
    check_command_help_recursively(&cmd, "finctl");
}

fn check_command_help_recursively(cmd: &clap::Command, path: &str) {
    let current_name = if path.is_empty() {
        cmd.get_name().to_string()
    } else {
        format!("{path} {}", cmd.get_name())
    };

    // Todo comando deve possuir descrição (about)
    assert!(
        cmd.get_about().is_some(),
        "Comando '{}' está sem descrição (about). Todos os comandos devem ter descrição.",
        current_name
    );

    // Todo argumento/flag deve possuir texto de ajuda (help)
    for arg in cmd.get_arguments() {
        let arg_id = arg.get_id().as_str();
        // Ignora os argumentos automáticos padrão do Clap
        if arg_id == "help" || arg_id == "version" {
            continue;
        }

        assert!(
            arg.get_help().is_some(),
            "Argumento '{}' no comando '{}' está sem texto descritivo de ajuda (help).",
            arg_id,
            current_name
        );
    }

    // Verifica recursivamente todos os subcomandos
    for sub in cmd.get_subcommands() {
        if sub.get_name() == "help" {
            continue;
        }
        check_command_help_recursively(sub, &current_name);
    }
}

#[test]
fn test_generate_completions_for_all_supported_shells() {
    let shells = [
        Shell::Bash,
        Shell::Zsh,
        Shell::Fish,
        Shell::PowerShell,
        Shell::Elvish,
    ];

    for shell in shells {
        let mut cmd = build_cli();
        let mut buffer = Vec::new();
        clap_complete::generate(shell, &mut cmd, "finctl", &mut buffer);

        let script = String::from_utf8(buffer).expect("Script de completion deve ser UTF-8 válido");
        assert!(
            !script.is_empty(),
            "Script de autocompletar para {shell:?} não deve ser vazio"
        );
        assert!(
            script.contains("finctl"),
            "Script de autocompletar para {shell:?} deve conter referência a 'finctl'"
        );
    }
}

#[test]
fn test_bash_completion_syntax_smoke_test() {
    let mut cmd = build_cli();
    let mut buffer = Vec::new();
    clap_complete::generate(Shell::Bash, &mut cmd, "finctl", &mut buffer);

    let tmp_file =
        std::env::temp_dir().join(format!("finctl_completion_{}.bash", uuid::Uuid::new_v4()));
    fs::write(&tmp_file, buffer).unwrap();

    // Se bash estiver instalado no sistema, valida sintaxe com bash -n
    if let Ok(output) = Command::new("bash").arg("-n").arg(&tmp_file).output() {
        assert!(
            output.status.success(),
            "O script de autocompletar para bash possui erros de sintaxe: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let _ = fs::remove_file(tmp_file);
}

#[test]
fn test_zsh_completion_syntax_smoke_test() {
    let mut cmd = build_cli();
    let mut buffer = Vec::new();
    clap_complete::generate(Shell::Zsh, &mut cmd, "finctl", &mut buffer);

    let tmp_file = std::env::temp_dir().join(format!("_finctl_{}", uuid::Uuid::new_v4()));
    fs::write(&tmp_file, buffer).unwrap();

    // Se zsh estiver presente no sistema / CI, valida sintaxe com zsh -n
    if let Ok(output) = Command::new("zsh").arg("-n").arg(&tmp_file).output() {
        assert!(
            output.status.success(),
            "O script de autocompletar para zsh possui erros de sintaxe: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    } else {
        println!("zsh não encontrado no ambiente local; teste executado condicionalmente.");
    }

    let _ = fs::remove_file(tmp_file);
}

#[test]
fn test_man_page_rendering_and_directory_generation() {
    let cmd = build_cli();
    let man = Man::new(cmd.clone());
    let mut output = Vec::new();
    man.render(&mut output).unwrap();

    let roff = String::from_utf8(output).unwrap();
    assert!(roff.contains(".TH finctl 1"));
    assert!(roff.contains("Sistema de controle financeiro pessoal"));

    let tmp_man_dir = std::env::temp_dir().join(format!("finctl_man_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&tmp_man_dir).unwrap();
    let paths = clap_mangen::generate_to(cmd, &tmp_man_dir);
    assert!(paths.is_ok());

    let main_man = tmp_man_dir.join("finctl.1");
    assert!(main_man.exists());
    assert!(fs::metadata(main_man).unwrap().len() > 0);

    let _ = fs::remove_dir_all(tmp_man_dir);
}

#[test]
fn test_handle_completions_and_man_commands() {
    use finctl_cli::commands::completions::{handle_completions_command, CompletionsArgs};
    use finctl_cli::commands::man::{handle_man_command, ManArgs};

    // Test completions handler executes without error
    let cmd = build_cli();
    handle_completions_command(CompletionsArgs { shell: Shell::Bash }, cmd);

    // Test man handler with dir executes and creates files
    let cmd = build_cli();
    let tmp_dir = std::env::temp_dir().join(format!("finctl_man_handler_{}", uuid::Uuid::new_v4()));
    let res = handle_man_command(
        ManArgs {
            dir: Some(tmp_dir.clone()),
        },
        cmd,
    );
    assert!(res.is_ok());
    assert!(tmp_dir.join("finctl.1").exists());
    let _ = fs::remove_dir_all(tmp_dir);
}
