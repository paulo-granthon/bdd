#!/bin/sh
#
# Instala o bdd na máquina (on-box). Baixa o binário estático do GitHub e o
# coloca em /usr/local/bin/bdd. Pode rodar via:
#   curl -L paulo-granthon.github.io/bdd | sh
#   wget -qO- paulo-granthon.github.io/bdd | sh
#
set -eu

URL="https://paulo-granthon.github.io/bdd/bin"
DEST="/usr/local/bin/bdd"

SUDO=""
if [ "$(id -u)" -ne 0 ]; then SUDO="sudo"; fi

fetch() {
  if command -v curl >/dev/null 2>&1; then curl -fSL "$1" -o "$2"
  elif command -v wget >/dev/null 2>&1; then wget -O "$2" "$1" || wget --secure-protocol=TLSv1_2 -O "$2" "$1"
  else echo "[bdd] preciso de curl ou wget. Rode: sudo apt-get install -y curl" >&2; return 2; fi
}

echo "[bdd] baixando o binario..."
# logo apos um push, o asset 'latest' pode dar 404 por ~1 min enquanto sobe.
ok=0
for try in 1 2 3 4 5 6; do
  if fetch "$URL" /tmp/bdd && [ -s /tmp/bdd ]; then ok=1; break; fi
  rc=$?; [ "$rc" = 2 ] && exit 1
  echo "[bdd] ainda nao disponivel (tentativa ${try}), aguardando 10s..."
  sleep 10
done
[ "$ok" = 1 ] || { echo "[bdd] nao consegui baixar o binario." >&2; exit 1; }

$SUDO install -m 0755 /tmp/bdd "$DEST"
$SUDO mkdir -p /var/lib/bdd
$SUDO chmod 777 /var/lib/bdd
rm -f /tmp/bdd

echo "[bdd] instalado em ${DEST}"
echo
echo "Proximo:"
echo "  bdd id      (diga qual maquina e esta: MGM, N1 ou N2)"
echo "  bdd next    (veja o proximo passo a executar)"
