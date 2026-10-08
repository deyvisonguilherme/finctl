use finctl_cli::run;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    if let Err((msg, code)) = run().await {
        eprintln!("{msg}");
        ExitCode::from(code)
    } else {
        ExitCode::SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use finctl_cli::get_database_url;

    #[test]
    fn test_missing_database_url_returns_code_2() {
        std::env::remove_var("DATABASE_URL");
        let err = get_database_url().unwrap_err();
        assert_eq!(err.1, 2);
        assert!(err.0.contains("DATABASE_URL"));
    }
}
