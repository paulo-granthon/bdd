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
            for s in lista.iter().filter(|s| ran.is_empty() || st.has_ran(&s.id())) {
                println!("  - {} {}", s.id(), s.title);
            }
            println!();
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
