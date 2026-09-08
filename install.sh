#!/usr/bin/env bash
# Instala el applet dentro de $HOME (sin sudo).
set -euo pipefail

BIN=cosmic-applet-sysmon
APPID=io.github.martin_rizzi.CosmicSysMon
PREFIX="${PREFIX:-$HOME/.local}"

cd "$(dirname "$0")"

if [[ ! -x target/release/$BIN ]]; then
    echo "Falta target/release/$BIN — compilá primero (ver README)." >&2
    exit 1
fi

install -Dm755 "target/release/$BIN" "$PREFIX/bin/$BIN"
install -Dm644 "res/$APPID.desktop" "$PREFIX/share/applications/$APPID.desktop"
# El panel lanza el Exec tal cual: ruta absoluta para no depender del PATH de la sesión.
sed -i "s|^Exec=.*|Exec=$PREFIX/bin/$BIN|" "$PREFIX/share/applications/$APPID.desktop"

echo "Instalado:"
echo "  $PREFIX/bin/$BIN"
echo "  $PREFIX/share/applications/$APPID.desktop"
echo
echo "Agregalo al panel con el id: $APPID"
