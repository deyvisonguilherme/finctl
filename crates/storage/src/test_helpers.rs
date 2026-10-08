use crate::db::run_migrations;
use domain::UserId;
use sqlx::PgPool;
use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, ImageExt};
use testcontainers_modules::postgres::Postgres;

pub struct TestDb {
    pub pool: PgPool,
    pub user_id: UserId,
    pub database_url: String,
    _container: ContainerAsync<Postgres>,
}

impl TestDb {
    pub async fn setup() -> Self {
        if std::env::var("DOCKER_HOST").is_err()
            && std::path::Path::new("/run/user/1000/podman/podman.sock").exists()
        {
            std::env::set_var("DOCKER_HOST", "unix:///run/user/1000/podman/podman.sock");
        }

        setup_test_postgres_tools();

        let container = Postgres::default()
            .with_tag("16-alpine")
            .start()
            .await
            .expect("Falha ao iniciar container Postgres para testes");

        let host_port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("Falha ao obter porta do Postgres");

        let database_url = format!("postgres://postgres:postgres@127.0.0.1:{host_port}/postgres");

        tokio::time::sleep(Duration::from_millis(300)).await;

        let pool = sqlx::PgPool::connect(&database_url)
            .await
            .expect("Falha ao conectar no banco de teste");

        run_migrations(&pool)
            .await
            .expect("Falha ao aplicar migrations no banco de teste");

        Self {
            pool,
            user_id: UserId::generate(),
            database_url,
            _container: container,
        }
    }
}

pub fn setup_test_postgres_tools() {
    if std::process::Command::new("pg_dump")
        .arg("--version")
        .output()
        .is_ok()
    {
        return;
    }

    let bin_dir = std::path::PathBuf::from("/tmp/finctl_test_tools");
    let _ = std::fs::create_dir_all(&bin_dir);

    let pg_dump_script = bin_dir.join("pg_dump");
    let pg_restore_script = bin_dir.join("pg_restore");

    let script_content_dump = r#"#!/bin/sh
exec podman run --rm --net=host -v /tmp:/tmp -e PGHOST -e PGPORT -e PGUSER -e PGPASSWORD docker.io/library/postgres:16-alpine pg_dump "$@"
"#;
    let script_content_restore = r#"#!/bin/sh
exec podman run --rm --net=host -v /tmp:/tmp -e PGHOST -e PGPORT -e PGUSER -e PGPASSWORD docker.io/library/postgres:16-alpine pg_restore "$@"
"#;

    let _ = std::fs::write(&pg_dump_script, script_content_dump);
    let _ = std::fs::write(&pg_restore_script, script_content_restore);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(&pg_dump_script) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(&pg_dump_script, perms.clone());
            let _ = std::fs::set_permissions(&pg_restore_script, perms);
        }
    }

    std::env::set_var("FINCTL_PG_DUMP_PATH", pg_dump_script.to_str().unwrap());
    std::env::set_var(
        "FINCTL_PG_RESTORE_PATH",
        pg_restore_script.to_str().unwrap(),
    );
}
