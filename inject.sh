#!/usr/bin/env bash
#
# inject.sh - roda no HOST (não nas VMs). Acha as VMs na rede, descobre qual é
# MGM / N1 / N2 (quando o EX02 já foi feito) e instala o bdd em cada uma via SSH,
# sem você digitar nada dentro da VM.
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
say() { echo "${c_cyan}[inject]${c_off} $*"; }
err() { echo "${c_red}[inject][erro]${c_off} $*" >&2; }

need() { command -v "$1" >/dev/null 2>&1; }
for dep in ssh scp; do need "$dep" || { err "falta '$dep' no host."; exit 1; }; done
if ! need sshpass; then
  err "falta 'sshpass' no host (autenticação por senha)."
  echo "  Instale: sudo pacman -S sshpass   (ou: sudo apt-get install -y sshpass)"
  exit 1
fi

say "baixando o binário do bdd..."
if need curl; then curl -fSL "$BIN_URL" -o "$TMP_BIN"; else wget -O "$TMP_BIN" "$BIN_URL"; fi
[ -s "$TMP_BIN" ] || { err "download do binário falhou."; exit 1; }

# --- sub-rede do host -------------------------------------------------------
CIDR="$(ip -o -4 addr show scope global 2>/dev/null | awk '{print $4}' | grep -vE '^127\.' | head -n1)"
[ -n "$CIDR" ] || { err "não achei a rede do host."; exit 1; }
BASE="$(echo "$CIDR" | cut -d/ -f1 | cut -d. -f1-3)"
say "procurando VMs (SSH aberto) em ${BASE}.0/24 ..."

FOUND=()
if need nmap; then
  mapfile -t FOUND < <(nmap -n -p22 --open -oG - "${BASE}.0/24" 2>/dev/null | awk '/22\/open/{print $2}')
else
  tmpf="$(mktemp)"
  for i in $(seq 1 254); do
    ( timeout 1 bash -c "exec 3<>/dev/tcp/${BASE}.${i}/22" 2>/dev/null && echo "${BASE}.${i}" >>"$tmpf" ) &
  done
  wait
  mapfile -t FOUND < <(sort -t. -k4 -n "$tmpf"); rm -f "$tmpf"
fi
[ "${#FOUND[@]}" -gt 0 ] || { err "nenhum host com SSH. As VMs estão ligadas com OpenSSH?"; exit 1; }

# --- login compartilhado (mesmo nas três VMs) -------------------------------
echo
say "login SSH das VMs (geralmente o mesmo nas três):"
read -r -p "  usuário: " SU
read -r -s -p "  senha: " SP; echo

# --- sondagem: descobre o papel provável de cada host -----------------------
declare -A SUG INFO AUTHOK
role_from() { # $1 hostname, $2 ips
  case "$(echo "$1" | tr 'A-Z' 'a-z')" in mgm) echo MGM; return;; n1) echo N1; return;; n2) echo N2; return;; esac
  case " $2 " in *192.168.1.1\ *) echo MGM;; *192.168.1.2\ *) echo N1;; *192.168.1.3\ *) echo N2;; *) echo "";; esac
}
say "identificando as máquinas..."
for ip in "${FOUND[@]}"; do
  out="$(sshpass -p "$SP" ssh $SSH_OPTS "${SU}@${ip}" 'echo "H:$(hostname)"; echo "I:$(hostname -I)"' 2>/dev/null)"
  if [ -z "$out" ]; then INFO[$ip]="login falhou ou não é VM do cluster"; SUG[$ip]=""; continue; fi
  AUTHOK[$ip]=1
  h="$(echo "$out" | sed -n 's/^H://p')"
  ips="$(echo "$out" | sed -n 's/^I://p')"
  SUG[$ip]="$(role_from "$h" "$ips")"
  intern="$(echo " $ips " | grep -oE '192\.168\.1\.[123]' | head -n1)"
  INFO[$ip]="hostname=${h:-?}${intern:+, interna=$intern}"
done

# --- lista numerada com sugestão -------------------------------------------
echo
say "VMs encontradas:"
i=1
for ip in "${FOUND[@]}"; do
  tag=""
  [ -n "${SUG[$ip]}" ] && tag="${c_green}→ sugerido: ${SUG[$ip]}${c_off}"
  printf "  %d) %-15s %s  %s\n" "$i" "$ip" "${c_dim}${INFO[$ip]:-}${c_off}" "$tag"
  i=$((i+1))
done

# --- monta sugestão limpa (cada papel em exatamente um host) ----------------
declare -A ROLE_IP
clean=1
for role in MGM N1 N2; do
  hits=(); for ip in "${FOUND[@]}"; do [ "${SUG[$ip]}" = "$role" ] && hits+=("$ip"); done
  if [ "${#hits[@]}" -eq 1 ]; then ROLE_IP[$role]="${hits[0]}"; else clean=0; fi
done

use_suggestion=0
if [ "$clean" = 1 ] && [ "${#ROLE_IP[@]}" -eq 3 ]; then
  echo
  say "sugestão automática:"
  for role in MGM N1 N2; do printf "  %s -> %s\n" "$role" "${ROLE_IP[$role]}"; done
  read -r -p "Aceitar a sugestão? [S/n]: " a
  case "$a" in n|N) use_suggestion=0;; *) use_suggestion=1;; esac
fi

# --- seleção manual (se não aceitou a sugestão) -----------------------------
if [ "$use_suggestion" = 0 ]; then
  unset ROLE_IP; declare -A ROLE_IP; used=""
  pick() {
    local role="$1" sel
    while :; do
      read -r -p "Qual host é o ${role}? [número, ou Enter para pular]: " sel
      [ -z "$sel" ] && return 0
      if ! [[ "$sel" =~ ^[0-9]+$ ]] || [ "$sel" -lt 1 ] || [ "$sel" -gt "${#FOUND[@]}" ]; then echo "  inválido."; continue; fi
      case " $used " in *" $sel "*) echo "  já usado."; continue;; esac
      ROLE_IP[$role]="${FOUND[$((sel-1))]}"; used="$used $sel"; return 0
    done
  }
  echo; pick MGM; pick N1; pick N2
fi
[ "${#ROLE_IP[@]}" -gt 0 ] || { err "nada selecionado."; exit 1; }

echo
say "vai instalar em:"
for role in MGM N1 N2; do [ -n "${ROLE_IP[$role]:-}" ] && printf "  %s -> %s\n" "$role" "${ROLE_IP[$role]}"; done
read -r -p "Confirma? [s/N]: " ok
case "$ok" in s|S|sim|y|Y) ;; *) say "cancelado."; exit 0;; esac

# --- injeção ----------------------------------------------------------------
inject_one() {
  local role="$1" ip="$2" rl user pass
  rl="$(echo "$role" | tr 'A-Z' 'a-z')"
  echo; say "=== ${role} (${ip}) ==="
  if [ "${AUTHOK[$ip]:-0}" = 1 ]; then
    user="$SU"; pass="$SP"
  else
    while :; do
      read -r -p "  usuário SSH de ${ip}: " user
      read -r -s -p "  senha: " pass; echo
      sshpass -p "$pass" ssh $SSH_OPTS "${user}@${ip}" true 2>/dev/null && break
      err "autenticação falhou, tente de novo."
    done
  fi
  say "  copiando binário..."
  sshpass -p "$pass" scp $SSH_OPTS "$TMP_BIN" "${user}@${ip}:/tmp/bdd" || { err "scp falhou."; return 1; }
  say "  instalando e definindo papel ${role}..."
  sshpass -p "$pass" ssh $SSH_OPTS "${user}@${ip}" \
    "echo '$pass' | sudo -S sh -c 'install -m 0755 /tmp/bdd /usr/local/bin/bdd && mkdir -p /var/lib/bdd && chmod 777 /var/lib/bdd && rm -f /tmp/bdd' && /usr/local/bin/bdd id ${rl} >/dev/null" \
    && say "  ${c_green}ok${c_off}: bdd em ${ip} como ${role}" \
    || err "instalação remota falhou em ${ip}"
}
for role in MGM N1 N2; do [ -n "${ROLE_IP[$role]:-}" ] && inject_one "$role" "${ROLE_IP[$role]}"; done

echo
say "${c_green}pronto.${c_off}"
echo "Próximo: em cada VM (via SSH) descubra onde parou e adote o progresso já feito:"
echo "  bdd check    (mostra o que já está pronto na máquina)"
echo "  bdd sync     (adota o progresso anterior para o bdd)"
echo "  bdd next     (mostra o próximo passo)"
