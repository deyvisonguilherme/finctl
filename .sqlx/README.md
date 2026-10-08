# .sqlx

Este diretório versionado armazena metadados de compilação offline do `sqlx` (`SQLX_OFFLINE=true`).

O projeto `finctl` utiliza queries dinâmicas validadas em tempo de execução e embute suas migrações em tempo de compilação via `sqlx::migrate!("./migrations")`, dispensando banco de dados acessível durante o build de release.
