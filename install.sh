#!/bin/sh
#
# Instala o bdd na máquina (on-box). Baixa o binário estático do GitHub e o
# coloca em /usr/local/bin/bdd. Pode rodar via:
#   curl -L paulo-granthon.github.io/bdd | sh
#   wget -qO- paulo-granthon.github.io/bdd | sh
#
set -eu

REPO="paulo-granthon/bdd"
URL="https://github.com/${REPO}/releases/latest/download/bdd"
DEST="/usr/local/bin/bdd"

SUDO=""
if [ "$(id -u)" -ne 0 ]; then SUDO="sudo"; fi

echo "[bdd] baixando o binario..."
if command -v curl >/dev/null 2>&1; then
  curl -fSL "$URL" -o /tmp/bdd
elif command -v wget >/dev/null 2>&1; then
  wget -O /tmp/bdd "$URL" || wget --secure-protocol=TLSv1_2 -O /tmp/bdd "$URL"
else
  echo "[bdd] preciso de curl ou wget. Rode: sudo apt-get install -y curl" >&2
  exit 1
fi

$SUDO install -m 0755 /tmp/bdd "$DEST"
$SUDO mkdir -p /var/lib/bdd
$SUDO chmod 777 /var/lib/bdd
rm -f /tmp/bdd

echo "[bdd] instalado em ${DEST}"
echo
echo "Proximo:"
echo "  bdd id      (diga qual maquina e esta: MGM, N1 ou N2)"
echo "  bdd next    (veja o proximo passo a executar)"
