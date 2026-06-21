# bdd - Projeto de Banco de Dados Distribuídos (FATEC-SJC)

`bdd` é um programa de linha de comando que executa os passos dos exercícios da
disciplina dentro das máquinas virtuais, no lugar de você digitar os comandos na
mão. Ele também sabe qual máquina é qual, registra o que já foi feito e diz
sempre qual é o próximo passo.

É um binário único, estático (não depende de nada instalado na VM). Depois de
instalado, você usa de qualquer lugar: `bdd 3.1`, `bdd next`, `bdd log`, etc.

## As máquinas

O cluster tem três VMs (montadas no EX02):

| Papel | IP interno   | Função |
|-------|--------------|--------|
| MGM   | 192.168.1.1  | Gerenciador do cluster |
| N1    | 192.168.1.2  | Nó de dados 1 |
| N2    | 192.168.1.3  | Nó de dados 2 |

Cada VM tem duas placas de rede (EX02): uma em **bridge** (DHCP, é por ela que o
host alcança a VM por SSH) e uma **interna** com o IP estático acima.

## Como instalar

Duas opções. A primeira não exige digitar nada dentro da VM.

### Opção A (recomendada): injetar do host via SSH

Roda no **seu computador (host)**, não na VM. O host baixa o binário e instala
em cada VM por SSH. Funciona assim que as VMs estão ligadas com OpenSSH (vem do
EX02), mesmo antes de configurar os IPs internos.

Pré-requisitos no host: `ssh`, `scp`, `sshpass` e `curl` ou `wget`.

```
git clone https://github.com/paulo-granthon/bdd.git
cd bdd
./inject.sh
```

O `inject.sh`:

1. procura as VMs na sua rede (hosts com SSH aberto);
2. mostra a lista numerada; você marca qual é o **MGM**, qual é o **N1** e qual é
   o **N2** (pode marcar menos de três);
3. para cada uma, pede usuário e senha do SSH (re-pergunta se errar);
4. copia o binário, instala em `/usr/local/bin/bdd` e já define o papel da
   máquina (`bdd id`).

Pronto: `bdd` está em todas, cada uma já sabe quem é.

### Opção B: instalar na própria VM (on-box)

Dentro da VM. Tente primeiro a linha curta; se der erro de SSL, use a de baixo
(o `wget` velho do Ubuntu 16.04 às vezes falha o TLS; o `curl` resolve).

```
wget -qO- paulo-granthon.github.io/bdd | sh
```

```
sudo apt-get install -y curl && curl -L paulo-granthon.github.io/bdd | sh
```

Depois, diga qual máquina é esta:

```
bdd id
```

## Como funciona (arquitetura)

- **Binário Rust estático (musl)**, sem dependências de runtime, então roda no
  Ubuntu 16.04 das VMs sem instalar mais nada.
- Os **scripts dos passos** (bash) ficam **embutidos** no binário. `bdd 3.1`
  extrai e executa o script certo. Depois de instalado, não precisa baixar mais
  nada (bom para a rede instável das VMs).
- **CI/CD**: cada push na `main` compila o binário e publica numa release
  `latest` no GitHub; o instalador (`install.sh`) é servido pelo GitHub Pages em
  `paulo-granthon.github.io/bdd`.
- **Estado** em `/var/lib/bdd/state`: o que já rodou e o que já validou. Por isso
  você pode fechar a sessão e voltar depois que o `bdd` sabe onde parou.
- **Identidade da máquina**: detectada pelo IP interno (`192.168.1.x`) definido
  no EX02. Enquanto esse IP não existe, você diz o papel com `bdd id`. Quando o
  IP passa a existir, ele manda (sobrepõe o que você definiu na mão).

## Comandos

| Comando | O que faz |
|---------|-----------|
| `bdd X.Y` | executa o passo Y do exercício X nesta máquina (ex: `bdd 3.1`) |
| `bdd log` | lista todos os passos, coloridos por estado, com legenda |
| `bdd next` | mostra só o próximo passo, e se é nesta máquina ou em outra |
| `bdd ok` | marca o próximo passo como feito (quando ele é de **outra** máquina) |
| `bdd check` | roda as validações e diz, por severidade, o que está pendente |
| `bdd id` | mostra/define qual máquina é esta (`bdd id` interativo, `bdd id mgm` direto) |

Depois de qualquer comando, o `bdd` imprime o **próximo passo** a executar.

### O ponteiro `next` e o `ok`

`next` é sempre a próxima coisa a fazer. Ele anda sozinho:

- se o próximo passo é **desta** máquina, você roda `bdd X.Y` aqui e o `next`
  avança;
- se o próximo é de **outra** máquina, você roda lá e, de volta aqui, faz
  `bdd ok` para o `next` avançar (o `ok` recusa se o próximo for desta máquina,
  justamente para você não pular um passo seu).

### Cores do `log` / `check`

- verde + `✓`: feito;
- ciano + `←`: próximo, nesta máquina;
- amarelo + `←`: próximo, em outra máquina;
- vermelho apagado: não roda nesta máquina;
- azul: assumido feito (já passamos dele);
- apagado: ainda não feito.

No `check`, falhas ganham severidade: amarelo `●` (passo atual, ainda em
andamento), vermelho `✗` (passo que já deveria estar pronto) e vermelho escuro
`✗!` (passo concluído depois de um incompleto, ordem furada).

## Exercícios

| Exercício | Conteúdo | Detalhes |
|-----------|----------|----------|
| EX02 | Rede das VMs (IP interno + hostname + ping) | [EX02/README.md](EX02/README.md) |
| EX03 | Instalação do MySQL Cluster | [EX03/README.md](EX03/README.md) |

Cada exercício tem o seu README com o estado antes/depois, o que ele faz, como
testar e o que capturar como prova.

## Desenvolvimento

Binário em Rust (`src/`), só biblioteca padrão, sem crates externos. Build local:

```
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
```

Os passos ficam em `EX0N/*.bash` e são embutidos via `include_str!` em
`src/model.rs` (que também guarda papel da máquina e o comando de validação de
cada passo).

O workflow `.github/workflows/release.yml` compila e publica a release `latest` a
cada push na `main`, e serve o `install.sh` pelo GitHub Pages. Para a Opção B de
instalação funcionar, habilite o Pages uma vez em Settings > Pages > Source:
GitHub Actions. A Opção A (inject pelo host) não depende do Pages.
