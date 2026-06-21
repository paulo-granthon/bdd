#!/usr/bin/env bash
#
# inject.sh - roda no HOST (não nas VMs). Acha as VMs na rede, você marca cada
# uma como MGM / N1 / N2, e o bdd é instalado em cada uma via SSH (sem precisar
# digitar nada dentro da VM).
#
# Requisitos no host: ssh, scp, sshpass, e curl ou wget.
#
set -uo pipefail

REPO="paulo-granthon/bdd"
BIN_URL="https://github.com/${REPO}/releases/latest/download/bdd"
SSH_OPTS="-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ConnectTimeout=5"
TMP_BIN="$(mktemp)"
trap 'rm -f "$TMP_BIN"' EXIT

c_green=$'\e[32m'; c_cyan=$'\e[36m'; c_red=$'\e[31m'; c_dim=$'\e[2m'; c_off=$'\e[0m'
say()  { echo "${c_cyan}[inject]${c_off} $*"; }
err()  { echo "${c_red}[inject][erro]${c_off} $*" >&2; }

# --- dependências do host ---------------------------------------------------
need() { command -v "$1" >/dev/null 2>&1; }
for dep in ssh scp; do need "$dep" || { err "falta '$dep' no host."; exit 1; }; done
if ! need sshpass; then
  err "falta 'sshpass' no host (usado para autenticar por senha)."
  echo "  Instale: sudo pacman -S sshpass   (ou: sudo apt-get install -y sshpass)"
  exit 1
fi

# --- baixa o binário uma vez no host ---------------------------------------
say "baixando o binário do bdd..."
if need curl; then curl -fSL "$BIN_URL" -o "$TMP_BIN"
else wget -O "$TMP_BIN" "$BIN_URL"; fi
[ -s "$TMP_BIN" ] || { err "download do binário falhou."; exit 1; }

# --- descobre a sub-rede do host -------------------------------------------
detect_cidr() {
  ip -o -4 addr show scope global 2>/dev/null \
    | awk '{print $4}' | grep -vE '^127\.' | head -n1
}
CIDR="$(detect_cidr)"
if [ -z "$CIDR" ]; then err "não achei a rede do host."; exit 1; fi
BASE="$(echo "$CIDR" | cut -d/ -f1 | cut -d. -f1-3)"
say "procurando VMs (porta SSH aberta) em ${BASE}.0/24 ..."

# --- scan: hosts com a porta 22 aberta -------------------------------------
FOUND=()
scan_one() {
  local ip="$1"
  if timeout 1 bash -c "exec 3<>/dev/tcp/${ip}/22" 2>/dev/null; then echo "$ip"; fi
}
if need nmap; then
  mapfile -t FOUND < <(nmap -n -p22 --open -oG - "${BASE}.0/24" 2>/dev/null | awk '/22\/open/{print $2}')
else
  tmpf="$(mktemp)"
  for i in $(seq 1 254); do scan_one "${BASE}.${i}" >>"$tmpf" & done
  wait
  mapfile -t FOUND < <(sort -t. -k4 -n "$tmpf")
  rm -f "$tmpf"
fi

if [ "${#FOUND[@]}" -eq 0 ]; then
  err "nenhum host com SSH encontrado. Confira que as VMs estão ligadas com OpenSSH."
  exit 1
fi

# --- lista numerada ---------------------------------------------------------
echo
say "VMs encontradas:"
i=1
for ip in "${FOUND[@]}"; do
  host="$(timeout 1 getent hosts "$ip" 2>/dev/null | awk '{print $2}')"
  printf "  %d) %s %s\n" "$i" "$ip" "${c_dim}${host}${c_off}"
  i=$((i+1))
done

# --- atribuição de papéis ---------------------------------------------------
declare -A ROLE_IP
used=""
pick() {
  local role="$1" sel
  while :; do
    read -r -p "Qual host é o ${role}? [número, ou Enter para pular]: " sel
    [ -z "$sel" ] && return 0
    if ! [[ "$sel" =~ ^[0-9]+$ ]] || [ "$sel" -lt 1 ] || [ "$sel" -gt "${#FOUND[@]}" ]; then
      echo "  número inválido."; continue
    fi
    case " $used " in *" $sel "*) echo "  já usado, escolha outro."; continue;; esac
    ROLE_IP[$role]="${FOUND[$((sel-1))]}"
    used="$used $sel"
    return 0
  done
}
echo
pick MGM
pick N1
pick N2

if [ "${#ROLE_IP[@]}" -eq 0 ]; then err "nada selecionado."; exit 1; fi

echo
say "vai instalar em:"
for role in MGM N1 N2; do
  [ -n "${ROLE_IP[$role]:-}" ] && printf "  %s -> %s\n" "$role" "${ROLE_IP[$role]}"
done
read -r -p "Confirma? [s/N]: " ok
case "$ok" in s|S|sim|y|Y) ;; *) say "cancelado."; exit 0;; esac

# --- injeta máquina por máquina --------------------------------------------
inject_one() {
  local role="$1" ip="$2" rl_lc user pass
  rl_lc="$(echo "$role" | tr 'A-Z' 'a-z')"
  echo
  say "=== ${role} (${ip}) ==="
  while :; do
    read -r -p "  usuário SSH de ${ip}: " user
    read -r -s -p "  senha de ${user}@${ip}: " pass; echo
    if sshpass -p "$pass" ssh $SSH_OPTS "${user}@${ip}" true 2>/dev/null; then
      break
    fi
    err "autenticação falhou, tente de novo."
  done
  say "  copiando binário..."
  sshpass -p "$pass" scp $SSH_OPTS "$TMP_BIN" "${user}@${ip}:/tmp/bdd" || { err "scp falhou."; return 1; }
  say "  instalando e definindo papel ${role}..."
  sshpass -p "$pass" ssh $SSH_OPTS "${user}@${ip}" \
    "echo '$pass' | sudo -S sh -c 'install -m 0755 /tmp/bdd /usr/local/bin/bdd && mkdir -p /var/lib/bdd && chmod 777 /var/lib/bdd && rm -f /tmp/bdd' && /usr/local/bin/bdd id ${rl_lc} >/dev/null" \
    && say "  ${c_green}ok${c_off}: bdd instalado em ${ip} como ${role}" \
    || err "instalação remota falhou em ${ip}"
}

for role in MGM N1 N2; do
  [ -n "${ROLE_IP[$role]:-}" ] && inject_one "$role" "${ROLE_IP[$role]}"
done

echo
say "${c_green}pronto.${c_off}"
echo "Próximo: em cada VM (via SSH ou console) rode:"
echo "  bdd next     (mostra o próximo passo)"
echo "  bdd log      (mostra tudo)"
