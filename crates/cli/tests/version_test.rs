use finctl_cli::FINCTL_VERSION;
use std::process::Command;

#[test]
fn test_version_string_format() {
    let pkg_version = env!("CARGO_PKG_VERSION");
    assert!(
        FINCTL_VERSION.starts_with(pkg_version),
        "FINCTL_VERSION deve iniciar com CARGO_PKG_VERSION ({pkg_version}), obtido: {FINCTL_VERSION}"
    );

    // Se estiver em repositório git (temos .git/), deve conter commit e data
    if std::path::Path::new("../../.git").exists() || std::path::Path::new(".git").exists() {
        assert!(
            FINCTL_VERSION.contains('(') && FINCTL_VERSION.contains(')'),
            "FINCTL_VERSION em repo git deve incluir hash e data entre parênteses: {FINCTL_VERSION}"
        );
    }
}

#[test]
fn test_cli_version_flag_output() {
    let binary_path = env!("CARGO_BIN_EXE_finctl");
    let output = Command::new(binary_path)
        .arg("--version")
        .output()
        .expect("Deve conseguir executar binário finctl --version");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.starts_with("finctl "),
        "Saída de --version deve começar com 'finctl ': {stdout}"
    );
    assert!(
        stdout.contains(env!("CARGO_PKG_VERSION")),
        "Saída de --version deve conter a versão do pacote: {stdout}"
    );
}
