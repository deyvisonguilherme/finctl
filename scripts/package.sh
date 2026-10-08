#!/usr/bin/env bash
set -euo pipefail

TARGET="${1:-x86_64-unknown-linux-gnu}"
TAG="${2:-$(git describe --tags --always 2>/dev/null || echo "v0.2.0")}"
PKG_NAME="finctl-${TAG}-${TARGET}"
OUT_DIR="target/package/${PKG_NAME}"

echo "==> Empacotando finctl para ${TARGET} (${TAG})..."
rm -rf "target/package/${PKG_NAME}"*
mkdir -p "${OUT_DIR}/completions" "${OUT_DIR}/man"

# 1. Copiar binário
BIN_FOUND=false
if [ -f "target/${TARGET}/release/finctl" ]; then
    cp "target/${TARGET}/release/finctl" "${OUT_DIR}/"
    BIN_FOUND=true
elif [ -f "target/${TARGET}/release/finctl.exe" ]; then
    cp "target/${TARGET}/release/finctl.exe" "${OUT_DIR}/"
    BIN_FOUND=true
elif [ -f "target/release/finctl" ]; then
    cp "target/release/finctl" "${OUT_DIR}/"
    BIN_FOUND=true
elif [ -f "target/release/finctl.exe" ]; then
    cp "target/release/finctl.exe" "${OUT_DIR}/"
    BIN_FOUND=true
fi

if [ "$BIN_FOUND" = false ]; then
    echo "Erro: binário finctl não encontrado em target/release ou target/${TARGET}/release."
    exit 1
fi

# 2. Copiar documentos
cp README.md CHANGELOG.md LICENSE "${OUT_DIR}/"

# 3. Copiar scripts de autocompletar
if [ -d "target/completions" ] && [ "$(ls -A target/completions 2>/dev/null)" ]; then
    cp target/completions/* "${OUT_DIR}/completions/"
else
    cargo run -p finctl -- completions bash > "${OUT_DIR}/completions/finctl.bash"
    cargo run -p finctl -- completions zsh > "${OUT_DIR}/completions/_finctl"
    cargo run -p finctl -- completions fish > "${OUT_DIR}/completions/finctl.fish"
    cargo run -p finctl -- completions powershell > "${OUT_DIR}/completions/_finctl.ps1"
    cargo run -p finctl -- completions elvish > "${OUT_DIR}/completions/finctl.elv"
fi

# 4. Copiar páginas de manual (man pages)
if [ -d "target/man" ] && [ "$(ls -A target/man 2>/dev/null)" ]; then
    cp target/man/* "${OUT_DIR}/man/"
else
    cargo run -p finctl -- man --dir "${OUT_DIR}/man"
fi

# 5. Compactar e gerar checksum SHA-256
cd target/package
if [[ "${TARGET}" == *"windows"* ]]; then
    if command -v zip >/dev/null 2>&1; then
        zip -r "${PKG_NAME}.zip" "${PKG_NAME}"
    else
        7z a "${PKG_NAME}.zip" "${PKG_NAME}"
    fi
    sha256sum "${PKG_NAME}.zip" > "${PKG_NAME}.zip.sha256"
    echo "==> Pacote criado com sucesso: target/package/${PKG_NAME}.zip"
    cat "${PKG_NAME}.zip.sha256"
else
    tar -czvf "${PKG_NAME}.tar.gz" "${PKG_NAME}"
    sha256sum "${PKG_NAME}.tar.gz" > "${PKG_NAME}.tar.gz.sha256"
    echo "==> Pacote criado com sucesso: target/package/${PKG_NAME}.tar.gz"
    cat "${PKG_NAME}.tar.gz.sha256"
fi
