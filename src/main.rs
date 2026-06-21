//! bdd - CLI dos exercícios de Projeto de Banco de Dados Distribuídos (FATEC-SJC).

mod detect;
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
        "sync" => cmd_sync(),
        "check" => cmd_check(),
        "id" => cmd_id(args.get(1).map(|s| s.as_str())),
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
    println!("  bdd log     lista todos os passos e o estado de cada um");
    println!("  bdd next    mostra o próximo passo a executar");
    println!("  bdd ok      marca o próximo passo como feito (passo de OUTRA máquina)");
    println!("  bdd sync    adota o progresso já feito (antes do bdd) checando a máquina");
    println!("  bdd check   roda as validações e diz o que está pendente");
    println!("  bdd id      mostra/define qual máquina é esta (MGM/N1/N2)");
    println!();
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

    println!(
        "{}",
        ui::paint(ui::BOLD, &format!("== bdd {}  ({})  [{}] ==", id, step.title, role.name()))
    );
    let ok = run_script(step.script, role);
    let mut st = State::load();
    if ok {
        st.mark_ran(id);
        println!("{}", ui::paint(ui::GREEN, &format!("{} passo {} concluído.", ui::CHECK, id)));
        hint_after_run(&steps, &st);
    } else {
        eprintln!("{}", ui::paint(ui::RED, &format!("{} passo {} falhou.", ui::CROSS, id)));
        ui::proximo(&[
            "leia o erro acima, ajuste e rode de novo o mesmo comando".to_string(),
            format!("os scripts são idempotentes: pode repetir `bdd {}`", id),
        ]);
        std::process::exit(1);
    }
}

fn run_script(script: &str, role: Role) -> bool {
    let mut path = std::env::temp_dir();
    path.push(format!("bdd-step-{}.sh", std::process::id()));
    if std::fs::write(&path, script).is_err() {
        eprintln!("[bdd] não consegui escrever o script temporário.");
        return false;
    }
    let status = Command::new("bash")
        .arg(&path)
        .env("BDD_ROLE", role.code())
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
        ui::paint(ui::DIM, "`next` é sempre a próxima coisa a fazer; ele anda sozinho conforme você executa os passos.")
    );
    match next_step(&steps, &st) {
        None => {
            println!("{}", ui::paint(ui::GREEN, &format!("{} nada pendente, tudo feito.", ui::CHECK)));
        }
        Some(i) => {
            let n = &steps[i];
            let mine = role.map(|r| n.for_role(r)).unwrap_or(false);
            if mine {
                println!(
                    "{}",
                    ui::paint(ui::CYAN, &format!("{} {}  {}  (nesta máquina, {})", n.id(), ui::ARROW, n.title, role.unwrap().name()))
                );
                ui::proximo(&[format!("rode: bdd {}  (ao rodar, `next` avança sozinho)", n.id())]);
            } else {
                let m = n.machines_label();
                println!(
                    "{}",
                    ui::paint(ui::YELLOW, &format!("{} {}  {}  (na máquina {}, não nesta {})", n.id(), ui::ARROW, n.title, m, role.map(|r| r.name()).unwrap_or("?")))
                );
                if role.is_none() {
                    ui::proximo(&["esta máquina não tem papel; defina: bdd id".to_string()]);
                } else {
                    ui::proximo(&[
                        format!("rode `bdd {}` na máquina {}", n.id(), m),
                        "depois, aqui, marque como feito: bdd ok".to_string(),
                    ]);
                }
            }
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

// ----------------------------------------------------------------- sync

fn cmd_sync() {
    let steps = manifest();
    let mut st = State::load();
    let (role, origin) = current_role();
    let role = match role {
        Some(r) => r,
        None => {
            eprintln!("{}", ui::paint(ui::RED, "Papel da máquina indefinido."));
            ui::proximo(&["defina: bdd id".to_string()]);
            return;
        }
    };

    ui::header("Sincronizando com o estado real da máquina");
    println!("Máquina: {} ({})", ui::paint(ui::BOLD, role.name()), origin);
    println!("{}", ui::paint(ui::DIM, "Roda as validações e adota como feito os passos desta máquina que já passam."));
    println!();

    let mut adotados = 0;
    for s in steps.iter() {
        if !s.for_role(role) {
            continue;
        }
        let id = s.id();
        if st.has_ran(&id) {
            continue;
        }
        if run_validation(s, role) {
            st.mark_ran(&id);
            st.mark_checked(&id);
            adotados += 1;
            println!("{}", ui::paint(ui::GREEN, &format!("  {} {}  adotado: {}", id, ui::CHECK, s.title)));
        }
    }

    if adotados == 0 {
        println!("{}", ui::paint(ui::FADED, "  nada novo a adotar (nenhum passo desta máquina passou que já não estivesse marcado)."));
    }
    println!();
    println!("{}", ui::paint(ui::DIM, "Passos de OUTRAS máquinas não são adotados aqui; rode `bdd sync` em cada VM e use `bdd ok` para os de outra máquina."));
    ui::proximo(&[
        "veja onde você está: bdd log".to_string(),
        "próximo passo: bdd next".to_string(),
    ]);
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
    let next_idx = next_step(&steps, &st).unwrap_or(steps.len());
    let last_ran_idx = steps.iter().enumerate().rev().find(|(_, s)| st.has_ran(&s.id())).map(|(i, _)| i);

    ui::header("Validação dos passos");
    println!("Máquina: {}", ui::paint(ui::BOLD, role.name()));
    println!();

    let mut high_plus = false;
    let mut high = false;

    for (i, s) in steps.iter().enumerate() {
        let id = s.id();
        let mine = s.for_role(role);
        let before = i < next_idx;
        let at = i == next_idx;

        if !mine {
            println!("{}", ui::paint(ui::FADED, &format!("  {}  n/a (máquina {}): {}", id, s.machines_label(), s.title)));
            continue;
        }

        let passed = if st.has_checked(&id) { true } else { run_validation(s, role) };

        if passed {
            if !st.has_checked(&id) {
                st.mark_checked(&id);
            }
            println!("{}", ui::paint(ui::GREEN, &format!("  {} {}  passou: {}", id, ui::CHECK, s.title)));
        } else if before {
            let progrediu = last_ran_idx.map(|li| li > i).unwrap_or(false);
            if progrediu {
                high_plus = true;
                println!("{}", ui::paint(ui::DARK_RED, &format!("  {} {}{} FALHOU, passo anterior incompleto e já avançamos: {}", id, ui::CROSS, ui::BANG, s.title)));
            } else {
                high = true;
                println!("{}", ui::paint(ui::RED, &format!("  {} {} FALHOU, deveria estar pronto: {}", id, ui::CROSS, s.title)));
            }
        } else if at {
            println!("{}", ui::paint(ui::YELLOW, &format!("  {} {} atual, ainda não concluído: {}", id, ui::BALL, s.title)));
        } else {
            println!("{}", ui::paint(ui::FADED, &format!("  {}  ainda não iniciado: {}", id, s.title)));
        }
    }

    println!();
    let resumo = if high_plus {
        ui::paint(ui::DARK_RED, &format!("{} Tem passo concluído depois de um incompleto. Volte e conserte o que falhou antes de seguir.", ui::BANG))
    } else if high {
        ui::paint(ui::RED, &format!("{} Um passo que deveria estar pronto falhou. Conserte antes de continuar.", ui::CROSS))
    } else if next_idx >= steps.len() {
        ui::paint(ui::GREEN, &format!("{} Tudo validado, exercícios completos.", ui::CHECK))
    } else {
        let n = &steps[next_idx];
        if n.for_role(role) {
            ui::paint(ui::CYAN, &format!("Tudo certo até aqui. O atual ({}) ainda não foi feito, é o próximo nesta máquina.", n.id()))
        } else {
            ui::paint(ui::YELLOW, &format!("Tudo certo até aqui. O próximo ({}) é na máquina {}.", n.id(), n.machines_label()))
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
        .status();
    matches!(status, Ok(st) if st.success())
}
