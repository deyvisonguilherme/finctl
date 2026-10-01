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
    _container: ContainerAsync<Postgres>,
}

impl TestDb {
    pub async fn setup() -> Self {
        if std::env::var("DOCKER_HOST").is_err()
            && std::path::Path::new("/run/user/1000/podman/podman.sock").exists()
        {
            std::env::set_var("DOCKER_HOST", "unix:///run/user/1000/podman/podman.sock");
        }

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
            _container: container,
        }
    }
}
