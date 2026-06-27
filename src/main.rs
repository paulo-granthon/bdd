//! bdd - CLI dos exercícios de Projeto de Banco de Dados Distribuídos (FATEC-SJC).

mod detect;
mod inject;
mod model;
mod state;
mod ui;

use model::{find, manifest, Role, Step};
use state::State;
use std::io::Write;
use std::process::Command;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("");
    match cmd {
        "" | "help" | "-h" | "--help" => usage(),
        "--version" | "-V" => println!("bdd {}", VERSION),
        "log" => cmd_log(),
        "next" => cmd_next(),
        "ok" => cmd_ok(),
        "check" => cmd_check(),
        "validate" | "validar" => cmd_validate(&args[1..]),
        "run" => cmd_run_next(),
        "upgrade" | "update" => cmd_upgrade(),
        "id" => cmd_id(args.get(1).map(|s| s.as_str())),
        "inject" => inject::run(),
        s if is_step_id(s) => cmd_run(s),
        other => {
            eprintln!("{}", ui::paint(ui::RED, &format!("Comando desconhecido: '{}'", other)));
            usage();
        }
    }
}

fn is_step_id(s: &str) -> bool {
    let mut it = s.split('.');
    matches!((it.next(), it.next(), it.next()), (Some(a), Some(b), None)
        if !a.is_empty() && a.chars().all(|c| c.is_ascii_digit())
        && !b.is_empty() && b.chars().all(|c| c.is_ascii_digit()))
}

fn usage() {
    ui::header("bdd - exercícios de Banco de Dados Distribuídos");
    println!("Uso: bdd <comando>");
    println!();
    println!("  bdd X.Y     executa o passo Y do exercício X (ex: bdd 3.1)");
    println!("  bdd run     executa o próximo passo, se for desta máquina (sem digitar o número)");
    println!("  bdd log     lista todos os passos e o estado de cada um");
    println!("  bdd next    mostra o próximo passo a executar");
    println!("  bdd validate [X] [--clean]  imprime as provas (saída); X = EX0X; --clean limpa a tela e mostra só as provas");
    println!("  bdd ok      marca o próximo passo como feito (passo de OUTRA máquina)");
    println!("  bdd check   valida a máquina, adota o que já está pronto e ajusta o next");
    println!("  bdd upgrade baixa a última versão do bdd e se substitui (pede sudo se preciso)");
    println!("  bdd id      mostra/define qual máquina é esta (MGM/N1/N2)");
    println!("  bdd inject  (no HOST) instala o bdd nas VMs por SSH (TUI)");
    println!();

    // Primeira vez que rodam `bdd` nesta máquina: mostra o passo-a-passo inicial.
    let mut st = State::load();
    if !st.seen_intro {
        st.seen_intro = true;
        st.save();
        ui::proximo(&[
            "veja o que já está pronto e ajuste o ponto: bdd check".to_string(),
            "e então, o próximo passo a executar:        bdd next".to_string(),
        ]);
        return;
    }

    let (role, origin) = current_role();
    match role {
        Some(r) => ui::proximo(&[format!(
            "esta máquina é {} ({}). Veja o que falta: bdd next",
            ui::paint(ui::BOLD, r.name()),
            origin
        )]),
        None => ui::proximo(&[
            "defina qual máquina é esta: bdd id".to_string(),
            "depois veja o próximo passo: bdd next".to_string(),
        ]),
    }
}

/// Papel efetivo + origem ("detectado" / "você definiu" / "").
fn current_role() -> (Option<Role>, &'static str) {
    let st = State::load();
    detect::effective(st.user_role)
}

fn next_step(steps: &[Step], st: &State) -> Option<usize> {
    steps.iter().position(|s| !st.has_ran(&s.id()))
}

// ----------------------------------------------------------------- run X.Y

fn cmd_run(id: &str) {
    let steps = manifest();
    let step = match find(&steps, id) {
        Some(s) => s,
        None => {
            eprintln!("{}", ui::paint(ui::RED, &format!("Passo {} não existe.", id)));
            ui::proximo(&["veja os passos: bdd log".to_string()]);
            return;
        }
    };
    let (role, origin) = current_role();
    let role = match role {
        Some(r) => r,
        None => {
            eprintln!("{}", ui::paint(ui::RED, "Papel da máquina indefinido."));
            ui::proximo(&["defina: bdd id".to_string()]);
            return;
        }
    };

    if !step.for_role(role) {
        println!(
            "{}",
            ui::paint(
                ui::YELLOW,
                &format!(
                    "O passo {} é da máquina {}. Esta é {} ({}).",
                    id,
                    step.machines_label(),
                    role.name(),
                    origin
                )
            )
        );
        ui::proximo(&[
            format!("rode esse passo na máquina {}", step.machines_label()),
            "se já rodou lá, avance aqui com: bdd ok".to_string(),
        ]);
        std::process::exit(1);
    }

    exec_step(step, role, &steps);
}

fn exec_step(step: &Step, role: Role, steps: &[Step]) {
    let id = step.id();
    println!(
        "{}",
        ui::paint(ui::BOLD, &format!("== bdd {}  ({})  [{}] ==", id, step.title, role.name()))
    );
    let ok = run_script(step.script, role, &id);
    let mut st = State::load();
    if ok {
        st.mark_ran(&id);
        println!("{}", ui::paint(ui::GREEN, &format!("{} passo {} concluído.", ui::CHECK, id)));
        hint_after_run(steps, &st);
    } else {
        eprintln!("{}", ui::paint(ui::RED, &format!("{} passo {} falhou.", ui::CROSS, id)));
        ui::proximo(&[
            "leia o erro acima, ajuste e rode de novo o mesmo comando".to_string(),
            format!("os scripts são idempotentes: pode repetir `bdd {}`", id),
        ]);
        std::process::exit(1);
    }
}

// ----------------------------------------------------------------- exercícios

fn all_ran_ex(steps: &[Step], st: &State, ex: u8) -> bool {
    steps.iter().filter(|s| s.ex == ex).all(|s| st.has_ran(&s.id()))
}
fn ex_validated(st: &State, ex: u8) -> bool {
    st.validated.iter().any(|v| v == &ex.to_string())
}
/// Semente para os rascunhos: hostname + tempo + pid. Cada máquina/execução
/// gera uma combinação diferente, então não saem dois textos idênticos.
fn draft_seed() -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    if let Ok(o) = Command::new("hostname").output() {
        o.stdout.hash(&mut h);
    }
    if let Ok(d) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        d.as_nanos().hash(&mut h);
    }
    std::process::id().hash(&mut h);
    h.finish()
}
/// Escolhe uma variante de forma determinística por (semente, passo).
fn pick_variant<'a>(opts: &[&'a str], seed: u64, salt: &str) -> &'a str {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    if opts.is_empty() {
        return "";
    }
    let mut h = DefaultHasher::new();
    seed.hash(&mut h);
    salt.hash(&mut h);
    opts[(h.finish() % opts.len() as u64) as usize]
}
/// Frases-rascunho por passo observacional. Várias por passo, para variar entre
/// alunos. NÃO são a resposta final: o aluno reescreve com as próprias palavras.
fn step_draft_variants(id: &str) -> &'static [&'static str] {
    match id {
        "4.1" => &[
            "Funcionou: com NoOfReplicas=2 o N1 guarda cópia completa, então sozinho atende a escrita mesmo com o N2 fora.",
            "O INSERT no N1 foi aceito sem o N2, porque a réplica local cobre todos os fragmentos.",
            "A escrita ocorreu normalmente; a redundância mantém o dado disponível com um único nó de dados ativo.",
        ],
        "4.2" => &[
            "Funcionou: o MGM não entra na transação de dados, então a escrita segue mesmo com ele desligado.",
            "INSERT aceito sem o MGM, pois com o cluster já no ar as operações de dados não passam por ele.",
            "A inserção não dependeu do MGM, que cuida de gestão e arbitragem, não do caminho dos dados.",
        ],
        "4.3" => &[
            "A criação da tabela concluiu com o N2 fora; o schema é sincronizado quando ele reingressa.",
            "CREATE TABLE funcionou com um nó de dados só, ficando o N2 atualizado ao voltar.",
            "A tabela foi criada mesmo sem o N2: um nó ativo basta e o ausente recebe o schema depois.",
        ],
        "4.4" => &[
            "CREATE TABLE funcionou sem o MGM: a DDL é coordenada pelos nós de dados.",
            "A tabela foi criada com o MGM desligado, já que ele não participa da criação de schema.",
            "A criação concluiu sem o MGM, confirmando que DDL não depende do gerenciador.",
        ],
        "4.5" => &[
            "CREATE DATABASE funcionou: é operação local do SQL node, não distribuída pelo NDB.",
            "O database foi criado no N1 sem o N2, pois não depende do outro nó (operação local).",
            "Criou normalmente; o database é local ao SQL node e não exige os dois nós.",
        ],
        "4.6" => &[
            "CREATE DATABASE funcionou sem o MGM: operação local do SQL node, independe dele.",
            "O database foi criado com o MGM fora, já que essa operação não envolve o gerenciador.",
            "Criou sem problema; não há participação do MGM em CREATE DATABASE.",
        ],
        "4.7" => &[
            "Funcionou com só o N1 no ar: ele tem cópia completa e segue operando, embora sem rede de segurança (sem MGM nem segundo nó).",
            "A escrita ocorreu apenas com o N1, mostrando que um nó com réplica completa basta; porém fica frágil a falhas e reinício.",
            "INSERT aceito só no N1; o dado é gravado, mas sem MGM e sem o N2 não há recuperação se algo cair.",
        ],
        "4.8" => &[
            "CREATE TABLE no N2 funcionou sozinho: ele tem cópia completa e coordena a DDL.",
            "A tabela foi criada só com o N2 ativo, simétrico ao caso do N1.",
            "Concluiu apenas com o N2; um nó com réplica completa cria o schema, ainda que sem redundância no momento.",
        ],
        "4.9" => &[
            "CREATE DATABASE no N2 funcionou: operação local do SQL node, independe do resto.",
            "O database foi criado só com o N2, por ser operação local.",
            "Criou normalmente no N2, sem precisar do MGM nem do N1.",
        ],
        "4.10" => &[
            "Os 1000 registros entraram no N1; ao religar, o N2 ressincronizou e ficou com todos, comprovando replicação automática.",
            "Inseri 1000 no N1 com o N2 fora; quando o N2 voltou, a recuperação de nó copiou tudo, e a leitura nele mostrou os 1000.",
            "Após religar o N2, ele recuperou os 1000 registros inseridos durante a ausência, confirmando consistência na reentrada.",
        ],
        "4.11" => &[
            "O MGM é necessário para iniciar/configurar o cluster e, principalmente, para arbitragem contra split-brain; com tudo estável, os dados não dependem dele.",
            "Sem o MGM o cluster não sobe nem recupera falhas: ele registra os nós, monitora e arbitra partições, mesmo sem entrar no caminho dos dados.",
            "O papel do MGM é gestão e arbitragem (evitar que dois lados se achem o cluster); operações de dados rodam sem ele, mas a recuperação e o reinício de nós, não.",
        ],
        _ => &[],
    }
}
/// O que o aluno deve escrever num exercício observacional (sem saída a capturar).
fn observational_hint(ex: u8) -> &'static str {
    match ex {
        4 => "Cenários de cluster degradado (nós de dados e/ou MGM desligados). Para cada passo, escreva o que aconteceu: a operação foi aceita ou recusada, qual foi a mensagem/erro, e por que o cluster se comportou assim (réplicas, papel do MGM, etc.).",
        _ => "Exercício observacional. Para cada passo, escreva com suas palavras o que você observou ao rodar e por que o sistema se comportou daquele jeito.",
    }
}
/// Último exercício com pelo menos um passo já executado nesta máquina (o que
/// acabamos de fazer / estamos fazendo). Evita o `validate` pular para um
/// exercício onde nada rodou ainda.
fn last_ran_exercise(steps: &[Step], st: &State) -> Option<u8> {
    model::exercises(steps)
        .into_iter()
        .rev()
        .find(|&ex| steps.iter().any(|s| s.ex == ex && st.has_ran(&s.id())))
}
/// Primeiro exercício que ainda não está (todo feito E validado).
fn current_exercise(steps: &[Step], st: &State) -> Option<u8> {
    for ex in model::exercises(steps) {
        if !(all_ran_ex(steps, st, ex) && ex_validated(st, ex)) {
            return Some(ex);
        }
    }
    None
}
/// Índice do primeiro passo não feito dentro do exercício.
fn first_pending_in_ex(steps: &[Step], st: &State, ex: u8) -> Option<usize> {
    steps.iter().position(|s| s.ex == ex && !st.has_ran(&s.id()))
}

fn run_script(script: &str, role: Role, id: &str) -> bool {
    let mut path = std::env::temp_dir();
    path.push(format!("bdd-step-{}.sh", std::process::id()));
    if std::fs::write(&path, script).is_err() {
        eprintln!("[bdd] não consegui escrever o script temporário.");
        return false;
    }
    let status = Command::new("bash")
        .arg(&path)
        .env("BDD_ROLE", role.code())
        .env("BDD_STEP", id)
        .status();
    let _ = std::fs::remove_file(&path);
    matches!(status, Ok(s) if s.success())
}

fn hint_after_run(steps: &[Step], st: &State) {
    match next_step(steps, st) {
        None => ui::proximo(&[format!("{} todos os passos foram feitos.", ui::CHECK)]),
        Some(i) => {
            let n = &steps[i];
            let (role, _) = current_role();
            if role.map(|r| n.for_role(r)).unwrap_or(false) {
                ui::proximo(&[format!("próximo aqui: bdd {}  ({})", n.id(), n.title)]);
            } else {
                ui::proximo(&[
                    format!("próximo: {} na máquina {}", n.id(), n.machines_label()),
                    "quando rodar lá, volte aqui e faça: bdd ok".to_string(),
                    "ou só acompanhe: bdd next".to_string(),
                ]);
            }
        }
    }
}

// ----------------------------------------------------------------- next

fn cmd_next() {
    let steps = manifest();
    let st = State::load();
    let (role, _) = current_role();
    println!(
        "{}",
        ui::paint(ui::DIM, "`next` é sempre a próxima coisa a fazer; ele anda sozinho conforme você avança.")
    );
    let ex = match current_exercise(&steps, &st) {
        None => {
            println!("{}", ui::paint(ui::GREEN, &format!("{} tudo feito e validado.", ui::CHECK)));
            ui::proximo(&["(opcional) reimprimir as provas: bdd validate".to_string()]);
            return;
        }
        Some(ex) => ex,
    };

    match first_pending_in_ex(&steps, &st, ex) {
        // ainda há passo a fazer no exercício atual
        Some(i) => {
            let n = &steps[i];
            let mine = role.map(|r| n.for_role(r)).unwrap_or(false);
            if mine {
                println!("{}", ui::paint(ui::CYAN, &format!("{} {}  {}  (nesta máquina, {})", n.id(), ui::ARROW, n.title, role.unwrap().name())));
                ui::proximo(&[format!("rode: bdd run   (ou bdd {})", n.id())]);
            } else {
                let m = n.machines_label();
                println!("{}", ui::paint(ui::YELLOW, &format!("{} {}  {}  (na máquina {}, não nesta {})", n.id(), ui::ARROW, n.title, m, role.map(|r| r.name()).unwrap_or("?"))));
                if role.is_none() {
                    ui::proximo(&["esta máquina não tem papel; defina: bdd id".to_string()]);
                } else {
                    ui::proximo(&[
                        format!("rode na máquina {}", m),
                        "depois, aqui, marque como feito: bdd ok".to_string(),
                    ]);
                }
            }
        }
        // exercício todo feito, falta validar para fechá-lo
        None => {
            println!("{}", ui::paint(ui::CYAN, &format!("EX0{} concluído. Falta gerar as provas para fechar o exercício.", ex)));
            ui::proximo(&[
                "gere/imprima as provas: bdd validate".to_string(),
                "(depois) veja o próximo exercício: bdd next".to_string(),
            ]);
        }
    }
}

// ----------------------------------------------------------------- ok

fn cmd_ok() {
    let steps = manifest();
    let mut st = State::load();
    let (role, _) = current_role();
    let role = match role {
        Some(r) => r,
        None => {
            eprintln!("{}", ui::paint(ui::RED, "Papel da máquina indefinido."));
            ui::proximo(&["defina: bdd id".to_string()]);
            return;
        }
    };
    match next_step(&steps, &st) {
        None => {
            println!("{}", ui::paint(ui::GREEN, "Nada pendente."));
        }
        Some(i) => {
            let n = &steps[i];
            if n.for_role(role) {
                eprintln!(
                    "{}",
                    ui::paint(ui::RED, &format!(
                        "O próximo passo ({}) é DESTA máquina ({}). `bdd ok` é só para passos de outra máquina.",
                        n.id(), role.name()
                    ))
                );
                ui::proximo(&[format!("rode aqui mesmo: bdd {}", n.id())]);
                std::process::exit(1);
            }
            st.mark_ran(&n.id());
            println!("{}", ui::paint(ui::GREEN, &format!("{} {} marcado como feito (rodou na {}).", ui::CHECK, n.id(), n.machines_label())));
            hint_after_run(&steps, &st);
        }
    }
}

// ----------------------------------------------------------------- id

fn cmd_id(arg: Option<&str>) {
    let mut st = State::load();
    let det = detect::detected();
    print!("Detectado pelo ambiente: ");
    match det {
        Some(r) => println!("{}", ui::paint(ui::BOLD, r.name())),
        None => println!("{}", ui::paint(ui::FADED, "indefinido (sem IP interno ainda)")),
    }
    print!("Definido por você:       ");
    match st.user_role {
        Some(r) => println!("{}", ui::paint(ui::BOLD, r.name())),
        None => println!("{}", ui::paint(ui::FADED, "indefinido")),
    }
    let (eff, origin) = detect::effective(st.user_role);
    print!("Em uso (efetivo):        ");
    match eff {
        Some(r) => println!("{} ({})", ui::paint(ui::BOLD, r.name()), origin),
        None => println!("{}", ui::paint(ui::FADED, "indefinido")),
    }
    if det.is_some() {
        println!("{}", ui::paint(ui::DIM, "(o IP interno manda; o que você definir só vale enquanto não houver IP.)"));
    }

    if let Some(a) = arg {
        match Role::from_str(a) {
            Some(r) => {
                st.user_role = Some(r);
                st.save();
                println!("{}", ui::paint(ui::GREEN, &format!("Definido: esta máquina é {}.", r.name())));
                ui::proximo(&["veja o próximo passo: bdd next".to_string()]);
            }
            None => eprintln!("{}", ui::paint(ui::RED, &format!("Papel inválido: '{}'. Use mgm, n1 ou n2.", a))),
        }
        return;
    }

    println!();
    println!("Definir esta máquina como:");
    println!("  1) MGM   (gerenciador, 192.168.1.1)");
    println!("  2) N1    (nó de dados, 192.168.1.2)");
    println!("  3) N2    (nó de dados, 192.168.1.3)");
    print!("Escolha [1/2/3] ou Enter para cancelar: ");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line).is_err() {
        return;
    }
    let sel = line.trim();
    if sel.is_empty() {
        println!("{}", ui::paint(ui::FADED, "Cancelado, nada alterado."));
        return;
    }
    match Role::from_str(sel) {
        Some(r) => {
            st.user_role = Some(r);
            st.save();
            println!("{}", ui::paint(ui::GREEN, &format!("Definido: esta máquina é {}.", r.name())));
            ui::proximo(&["veja o próximo passo: bdd next".to_string()]);
        }
        None => eprintln!("{}", ui::paint(ui::RED, "Opção inválida.")),
    }
}

// ----------------------------------------------------------------- log

fn cmd_log() {
    let steps = manifest();
    let st = State::load();
    let (role, origin) = current_role();
    let next_idx = next_step(&steps, &st);
    let last_ran_idx = steps.iter().enumerate().rev().find(|(_, s)| st.has_ran(&s.id())).map(|(i, _)| i);

    ui::header("Passos dos exercícios");
    match role {
        Some(r) => println!("Máquina: {} ({})", ui::paint(ui::BOLD, r.name()), origin),
        None => println!("Máquina: {}, rode `bdd id`", ui::paint(ui::FADED, "indefinida")),
    }
    println!();

    let mut cur_ex = 0u8;
    for (i, s) in steps.iter().enumerate() {
        if s.ex != cur_ex {
            cur_ex = s.ex;
            println!("{}", ui::paint(ui::BOLD, &format!("EX0{}", cur_ex)));
        }
        let id = s.id();
        let line: String;
        if st.has_ran(&id) {
            line = ui::paint(ui::GREEN, &format!("  {} {}  {}", id, ui::CHECK, s.title));
        } else if Some(i) == next_idx {
            if role.map(|r| s.for_role(r)).unwrap_or(false) {
                line = ui::paint(ui::CYAN, &format!("  {} {} próximo  {}", id, ui::ARROW, s.title));
            } else {
                line = ui::paint(ui::YELLOW, &format!("  {} {} próximo (na {})  {}", id, ui::ARROW, s.machines_label(), s.title));
            }
        } else if role.map(|r| !s.for_role(r)).unwrap_or(false) {
            line = ui::paint(ui::FADED_RED, &format!("  {}, não roda nesta máquina  {}", id, s.title));
        } else if last_ran_idx.map(|li| li > i).unwrap_or(false) {
            line = ui::paint(ui::BLUE, &format!("  {}, assumido feito  {}", id, s.title));
        } else {
            line = ui::paint(ui::FADED, &format!("  {}  {}", id, s.title));
        }
        println!("{}", line);
    }

    println!();
    legenda();
    ui::proximo(&["próximo passo: bdd next".to_string(), "o que falta validar: bdd check".to_string()]);
}

fn legenda() {
    println!("{}", ui::paint(ui::DIM, "Legenda:"));
    println!("  {}   feito (via bdd)", ui::paint(ui::GREEN, &format!("x.y {}", ui::CHECK)));
    println!("  {}   próximo, nesta máquina", ui::paint(ui::CYAN, &format!("x.y {} próximo", ui::ARROW)));
    println!("  {}   próximo, mas em outra máquina", ui::paint(ui::YELLOW, &format!("x.y {} próximo (na ...)", ui::ARROW)));
    println!("  {}   não roda nesta máquina", ui::paint(ui::FADED_RED, "x.y"));
    println!("  {}   assumido feito (já avançamos além dele)", ui::paint(ui::BLUE, "x.y"));
    println!("  {}   ainda não feito", ui::paint(ui::FADED, "x.y"));
}

// ----------------------------------------------------------------- check

fn cmd_check() {
    let steps = manifest();
    let mut st = State::load();
    let (role, _) = current_role();
    let role = match role {
        Some(r) => r,
        None => {
            eprintln!("{}", ui::paint(ui::RED, "Papel da máquina indefinido."));
            ui::proximo(&["defina: bdd id".to_string()]);
            return;
        }
    };

    ui::header("Validação dos passos");
    println!("Máquina: {}", ui::paint(ui::BOLD, role.name()));
    println!();

    // 1) roda (ou usa cache) as validações desta máquina e ADOTA o que passou
    //    como feito, para o `next` ficar correto.
    let mut passed = vec![false; steps.len()];
    let mut cached = vec![false; steps.len()];
    for (i, s) in steps.iter().enumerate() {
        if !s.for_role(role) {
            continue;
        }
        let id = s.id();
        if s.validate.is_empty() {
            continue; // observacional: não valida nem adota (só conta se foi rodado)
        }
        if st.has_checked(&id) {
            passed[i] = true;
            cached[i] = true;
        } else if run_validation(s, role) {
            passed[i] = true;
            st.mark_checked(&id);
        }
        if passed[i] {
            st.mark_ran(&id);
        }
    }

    // 2) recalcula next/last_ran já com as adoções
    let next_idx = next_step(&steps, &st).unwrap_or(steps.len());
    let last_ran = steps.iter().enumerate().rev().find(|(_, s)| st.has_ran(&s.id())).map(|(i, _)| i);

    let cur_ex = current_exercise(&steps, &st);
    let mut high_plus = false;

    // "feito" = validação passou, ou (passo observacional) foi rodado
    let is_done = |i: usize, s: &Step| -> bool {
        passed[i] || (s.validate.is_empty() && st.has_ran(&s.id()))
    };

    // linha de um passo (usada quando o exercício é expandido)
    let step_line = |i: usize, s: &Step| -> String {
        if !s.for_role(role) {
            if st.has_ran(&s.id()) {
                ui::paint(ui::GREEN, &format!("  {} {}  feito em outra máquina ({}): {}", s.id(), ui::CHECK, s.machines_label(), s.title))
            } else {
                ui::paint(ui::FADED, &format!("  {}  n/a (máquina {}): {}", s.id(), s.machines_label(), s.title))
            }
        } else if is_done(i, s) {
            let lab = if s.validate.is_empty() { "feito" } else if cached[i] { "passou (cache)" } else { "passou" };
            ui::paint(ui::GREEN, &format!("  {} {}  {}: {}", s.id(), ui::CHECK, lab, s.title))
        } else if last_ran.map(|li| i < li).unwrap_or(false) {
            ui::paint(ui::DARK_RED, &format!("  {} {}{} FALHOU, incompleto antes de um passo já feito: {}", s.id(), ui::CROSS, ui::BANG, s.title))
        } else if i == next_idx {
            ui::paint(ui::YELLOW, &format!("  {} {} a fazer agora: {}", s.id(), ui::BALL, s.title))
        } else {
            ui::paint(ui::FADED, &format!("  {}  ainda não iniciado: {}", s.id(), s.title))
        }
    };

    for ex in model::exercises(&steps) {
        let idxs: Vec<usize> = steps.iter().enumerate().filter(|(_, s)| s.ex == ex).map(|(i, _)| i).collect();
        // problema = passo desta máquina que deveria estar pronto mas falhou (ordem furada)
        let problem = idxs.iter().any(|&i| steps[i].for_role(role) && !is_done(i, &steps[i]) && last_ran.map(|li| i < li).unwrap_or(false));
        if problem {
            high_plus = true;
        }
        if cur_ex == Some(ex) || problem {
            // expandido: passo a passo
            println!("{}", ui::paint(ui::BOLD, &format!("EX0{}", ex)));
            for &i in &idxs {
                println!("{}", step_line(i, &steps[i]));
            }
        } else if cur_ex.map(|c| ex < c).unwrap_or(true) {
            // exercício anterior, sem problemas: uma linha só
            println!("{}", ui::paint(ui::GREEN, &format!("EX0{} {}  tudo certo", ex, ui::CHECK)));
        } else {
            // exercício futuro: uma linha só
            println!("{}", ui::paint(ui::FADED, &format!("EX0{}  ainda não iniciado", ex)));
        }
    }

    println!();
    println!("{}", ui::paint(ui::DIM, "Legenda:"));
    println!("  {}   passou / feito (com (cache) se já validado antes)", ui::paint(ui::GREEN, &format!("x.y {}", ui::CHECK)));
    println!("  {}   a fazer agora (próximo desta máquina)", ui::paint(ui::YELLOW, &format!("x.y {}", ui::BALL)));
    println!("  {}   incompleto antes de um passo já feito (ordem furada)", ui::paint(ui::DARK_RED, &format!("x.y {}{}", ui::CROSS, ui::BANG)));
    println!("  {}   ainda não iniciado, ou passo de outra máquina", ui::paint(ui::FADED, "x.y"));
    println!();
    let resumo = if high_plus {
        ui::paint(ui::DARK_RED, &format!("{} Há passo concluído depois de um incompleto. Conserte o que falhou antes de seguir.", ui::BANG))
    } else if next_idx >= steps.len() {
        ui::paint(ui::GREEN, &format!("{} Tudo validado e completo.", ui::CHECK))
    } else {
        let n = &steps[next_idx];
        if n.for_role(role) {
            ui::paint(ui::CYAN, &format!("Próximo nesta máquina: {} ({}).", n.id(), n.title))
        } else {
            ui::paint(ui::YELLOW, &format!("Próximo é na máquina {}: {} ({}). Lá rode `bdd check`; aqui depois `bdd ok`.", n.machines_label(), n.id(), n.title))
        }
    };
    println!("{}", resumo);
    ui::proximo(&["próximo passo: bdd next".to_string()]);
}

fn run_validation(s: &Step, role: Role) -> bool {
    let status = Command::new("bash")
        .arg("-c")
        .arg(s.validate)
        .env("BDD_ROLE", role.code())
        .env("BDD_STEP", &s.id())
        .status();
    matches!(status, Ok(st) if st.success())
}

// ----------------------------------------------------------------- run (próximo)

fn cmd_run_next() {
    let steps = manifest();
    let st = State::load();
    let (role, _) = current_role();
    let role = match role {
        Some(r) => r,
        None => {
            eprintln!("{}", ui::paint(ui::RED, "Papel da máquina indefinido."));
            ui::proximo(&["defina: bdd id".to_string()]);
            return;
        }
    };
    let ex = match current_exercise(&steps, &st) {
        None => { println!("{}", ui::paint(ui::GREEN, "Tudo feito e validado.")); return; }
        Some(ex) => ex,
    };
    match first_pending_in_ex(&steps, &st, ex) {
        Some(i) => {
            let n = &steps[i];
            if n.for_role(role) {
                exec_step(n, role, &steps);
            } else {
                eprintln!("{}", ui::paint(ui::YELLOW, &format!(
                    "O próximo ({}) é da máquina {}, não desta ({}).", n.id(), n.machines_label(), role.name()
                )));
                ui::proximo(&[
                    format!("rode na máquina {}", n.machines_label()),
                    "quando rodar lá, aqui faça: bdd ok".to_string(),
                ]);
                std::process::exit(1);
            }
        }
        None => {
            // exercício todo feito, falta validar: roda a validação
            cmd_validate(&[]);
        }
    }
}

// ----------------------------------------------------------------- upgrade

fn cmd_upgrade() {
    // Onde o bdd está instalado (substitui a si mesmo); cai no padrão se não der.
    let dest = std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(String::from))
        .unwrap_or_else(|| "/usr/local/bin/bdd".to_string());
    println!("{}", ui::paint(ui::CYAN, &format!("Versão atual: {}", VERSION)));

    // 1) Baixa pro /tmp (sem sudo) e compara a versão ANTES de instalar.
    let fetch = r#"
set -eu
URL="https://paulo-granthon.github.io/bdd/bin"
fetch() {
  if command -v curl >/dev/null 2>&1; then curl -fsSL "$URL" -o /tmp/bdd.new
  elif command -v wget >/dev/null 2>&1; then wget -qO /tmp/bdd.new "$URL" || wget -q --secure-protocol=TLSv1_2 -O /tmp/bdd.new "$URL"
  else echo "preciso de curl ou wget (sudo apt-get install -y curl)" >&2; exit 2; fi
}
ok=0
for try in 1 2 3 4 5 6; do
  if fetch && [ -s /tmp/bdd.new ]; then ok=1; break; fi
  echo "ainda nao disponivel (tentativa $try), aguardando 10s..."; sleep 10
done
[ "$ok" = 1 ] || { echo "nao consegui baixar o binario" >&2; exit 1; }
chmod 0755 /tmp/bdd.new
"#;
    if !Command::new("sh").arg("-c").arg(fetch).status().map(|s| s.success()).unwrap_or(false) {
        eprintln!("{}", ui::paint(ui::RED, "Falha ao baixar o binário."));
        return;
    }

    let latest = Command::new("/tmp/bdd.new")
        .arg("--version")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    if latest == format!("bdd {}", VERSION) {
        let _ = std::fs::remove_file("/tmp/bdd.new");
        println!("{}", ui::paint(ui::GREEN, "Já está na última versão. Nada a fazer."));
        return;
    }

    // 2) Instala via rename no MESMO diretório do destino: troca a entrada sem
    // reabrir o binário em uso (evita ETXTBSY). sudo só se o dir não é gravável.
    let install = r#"
set -eu
SUDO=""
if [ ! -w "$(dirname "$DEST")" ] && [ "$(id -u)" -ne 0 ]; then SUDO="sudo"; fi
$SUDO sh -c 'cp /tmp/bdd.new "$0.tmp" && chmod 0755 "$0.tmp" && mv "$0.tmp" "$0"' "$DEST"
rm -f /tmp/bdd.new
"#;
    match Command::new("sh").arg("-c").arg(install).env("DEST", &dest).status() {
        Ok(s) if s.success() => {
            print!("{} ", ui::paint(ui::GREEN, "Atualizado. Versão agora:"));
            let _ = std::io::stdout().flush();
            let _ = Command::new(&dest).arg("--version").status();
        }
        _ => eprintln!("{}", ui::paint(ui::RED, "Falha ao instalar a atualização.")),
    }
}

// ----------------------------------------------------------------- validate

fn cmd_validate(args: &[String]) {
    // --clean: limpa a tela toda (e o scrollback) e imprime só as provas, sem o
    // cabeçalho "Provas do EX0X..." nem a seção "Próximo".
    let clean = args.iter().any(|a| a == "--clean");
    let arg = args.iter().map(|s| s.as_str()).find(|a| !a.starts_with("--"));

    let steps = manifest();
    let mut st = State::load();
    let (role, _) = current_role();
    let role = match role {
        Some(r) => r,
        None => {
            eprintln!("{}", ui::paint(ui::RED, "Papel da máquina indefinido."));
            ui::proximo(&["defina: bdd id".to_string()]);
            return;
        }
    };
    let exs = model::exercises(&steps);
    // Sem argumento: exercício atual. Com argumento (ex: "3" ou "EX03"): aquele exercício.
    let ex = match arg {
        // Padrão: o exercício que acabamos de fazer (último com passo executado),
        // não o próximo ainda intocado; cai para o atual/último se nada rodou.
        None => last_ran_exercise(&steps, &st)
            .or_else(|| current_exercise(&steps, &st))
            .unwrap_or_else(|| *exs.last().unwrap()),
        Some(a) => {
            let n: Option<u8> = a.trim_start_matches(|c: char| !c.is_ascii_digit()).parse().ok();
            match n.filter(|n| exs.contains(n)) {
                Some(n) => n,
                None => {
                    eprintln!("{}", ui::paint(ui::RED, &format!("Exercício inválido: '{}'", a)));
                    ui::proximo(&[format!("exercícios: {}", exs.iter().map(|e| format!("EX0{}", e)).collect::<Vec<_>>().join(", "))]);
                    return;
                }
            }
        }
    };

    if clean {
        // \x1b[2J apaga a tela, \x1b[3J o scrollback, \x1b[H leva o cursor ao topo.
        print!("\x1b[2J\x1b[3J\x1b[H");
        let _ = std::io::stdout().flush();
    } else {
        ui::header(&format!("Provas do EX0{} (capture a saída abaixo)", ex));
    }
    let _ = Command::new("sh").arg("-c").arg("hostname -I").status();
    println!();

    let role_steps: Vec<&Step> = steps.iter().filter(|s| s.ex == ex && s.for_role(role)).collect();
    let mut any = false;
    for s in role_steps.iter().filter(|s| !s.proof.is_empty()) {
        any = true;
        println!("{}", ui::paint(ui::BOLD, &format!("--- {} {} ---", s.id(), s.title)));
        let _ = Command::new("bash").arg("-c").arg(s.proof).env("BDD_ROLE", role.code()).env("BDD_STEP", &s.id()).status();
        println!();
    }
    if !any {
        if role_steps.is_empty() {
            println!("{}", ui::paint(ui::FADED, "Nenhuma prova desta máquina neste exercício."));
        } else {
            // Exercício observacional: não há saída de comando para capturar; a
            // prova é a conclusão que o aluno escreve sobre o que observou.
            println!("{}", ui::paint(ui::YELLOW, "Exercício observacional: a prova é a sua conclusão, escrita com suas palavras."));
            println!("{}", observational_hint(ex));
            println!();
            let ran: Vec<&&Step> = role_steps.iter().filter(|s| st.has_ran(&s.id())).collect();
            let (titulo, lista) = if ran.is_empty() {
                ("Passos desta máquina neste exercício (rode-os e descreva cada resultado):", &role_steps)
            } else {
                ("Passos que você rodou nesta máquina (descreva o resultado de cada um):", &role_steps)
            };
            println!("{}", titulo);
            let seed = draft_seed();
            let mut tem_rascunho = false;
            for s in lista.iter().filter(|s| ran.is_empty() || st.has_ran(&s.id())) {
                println!("  - {} {}", s.id(), s.title);
                let v = step_draft_variants(&s.id());
                if !v.is_empty() {
                    tem_rascunho = true;
                    println!("      {} {}", ui::paint(ui::FADED, "rascunho:"), pick_variant(v, seed, &s.id()));
                }
            }
            println!();
            if tem_rascunho {
                println!("{}", ui::paint(ui::FADED, "Os rascunhos são gerados localmente e variam por máquina/execução; reescreva com suas palavras antes de entregar."));
            }
            println!("{}", ui::paint(ui::FADED, "Dica: junte também os prints de cada 'bdd X.Y' que você rodou neste exercício."));
        }
    }

    if !ex_validated(&st, ex) {
        st.validated.push(ex.to_string());
        st.save();
    }
    if !clean {
        ui::proximo(&[
            "capture a saída acima como prova".to_string(),
            "próximo passo / exercício: bdd next".to_string(),
        ]);
    }
}
