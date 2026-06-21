//! `bdd inject` (roda no HOST): acha as VMs na rede, você marca quem é
//! MGM/N1/N2 numa TUI, e instala o bdd em cada uma por SSH. Não precisa
//! digitar nada dentro da VM.

use crate::model::Role;
use crate::ui;
use std::collections::HashSet;
use std::io::{stdout, Write};
use std::net::{SocketAddr, TcpStream};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossterm::cursor::MoveTo;
use crossterm::event::{read, Event, KeyCode, KeyModifiers};
use crossterm::style::Print;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType};
use crossterm::{execute, queue};

const SSH_OPTS: &[&str] = &[
    "-o", "StrictHostKeyChecking=no",
    "-o", "UserKnownHostsFile=/dev/null",
    "-o", "ConnectTimeout=6",
    "-o", "PubkeyAuthentication=no",
    "-o", "PreferredAuthentications=password",
];

struct Cand {
    ip: String,
    info: String,
    suggest: Option<Role>,
    creds: Option<(String, String)>, // par que autenticou
    role: Option<Role>,              // atribuído pelo usuário
}

pub fn run() {
    // dependências do host
    for dep in ["ssh", "scp"] {
        if !have(dep) {
            eprintln!("{}", ui::paint(ui::RED, &format!("[inject] falta '{}' no host.", dep)));
            return;
        }
    }
    if !have("sshpass") {
        eprintln!("{}", ui::paint(ui::RED, "[inject] falta 'sshpass' no host (autenticação por senha)."));
        eprintln!("Instale conforme a sua distro:");
        eprintln!("  Arch:          sudo pacman -S sshpass");
        eprintln!("  Debian/Ubuntu: sudo apt-get install -y sshpass");
        eprintln!("  Fedora:        sudo dnf install -y sshpass");
        eprintln!("  openSUSE:      sudo zypper install -y sshpass");
        return;
    }

    let bin = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => {
            eprintln!("[inject] não achei o próprio binário para enviar.");
            return;
        }
    };

    // sub-rede do host
    let host_ips = host_ips();
    let base = match host_ips.iter().find_map(|ip| subnet_base(ip)) {
        Some(b) => b,
        None => {
            eprintln!("[inject] não achei a rede do host.");
            return;
        }
    };

    // 1) credenciais (TUI: até 3 pares user/senha)
    let creds = match tui_creds() {
        Some(c) if c.iter().any(|(u, _)| !u.is_empty()) => c,
        _ => {
            println!("[inject] cancelado.");
            return;
        }
    };
    let creds: Vec<(String, String)> = creds.into_iter().filter(|(u, _)| !u.is_empty()).collect();

    // 2) scan
    println!("[inject] procurando VMs (SSH) em {}.0/24 ...", base);
    let exclude: HashSet<String> = host_ips.into_iter().collect();
    let ips = scan(&base, &exclude);
    if ips.is_empty() {
        println!("[inject] o scan não achou nenhum host. Você poderá adicionar IPs na mão.");
    }

    // 3) sonda cada IP com os pares de credenciais
    println!("[inject] identificando {} host(s)...", ips.len());
    let mut cands: Vec<Cand> = Vec::new();
    for ip in ips {
        let (info, suggest, creds_ok) = probe(&ip, &creds);
        cands.push(Cand { ip, info, suggest, creds: creds_ok, role: None });
    }

    // 4) seleção (TUI)
    let chosen = match tui_select(&mut cands, &creds) {
        Some(v) if !v.is_empty() => v,
        Some(_) => { println!("[inject] nada selecionado."); return; }
        None => { println!("[inject] cancelado."); return; }
    };

    // 5) injeção (tudo de uma vez)
    println!();
    let exe = bin.to_string_lossy().to_string();
    for (role, ip, (u, p)) in &chosen {
        print!("[inject] {} ({}) ... ", role.name(), ip);
        let _ = stdout().flush();
        if inject_one(ip, *role, u, p, &exe) {
            println!("{}", ui::paint(ui::GREEN, "ok"));
        } else {
            println!("{}", ui::paint(ui::RED, "falhou"));
        }
    }

    println!();
    println!("{}", ui::paint(ui::GREEN, "[inject] pronto."));
    ui::proximo(&[
        "em cada VM: bdd check   (mostra o que já está pronto)".to_string(),
        "depois:     bdd sync    (adota o progresso anterior)".to_string(),
        "e:          bdd next    (próximo passo)".to_string(),
    ]);
}

// --------------------------------------------------------------- rede / ssh

fn have(cmd: &str) -> bool {
    Command::new("sh").arg("-c").arg(format!("command -v {} >/dev/null 2>&1", cmd)).status().map(|s| s.success()).unwrap_or(false)
}

fn host_ips() -> Vec<String> {
    let out = Command::new("hostname").arg("-I").output();
    let mut v = Vec::new();
    if let Ok(o) = out {
        for t in String::from_utf8_lossy(&o.stdout).split_whitespace() {
            if t.contains('.') {
                v.push(t.to_string());
            }
        }
    }
    v
}

fn subnet_base(ip: &str) -> Option<String> {
    let p: Vec<&str> = ip.split('.').collect();
    if p.len() == 4 && p[0] != "127" {
        Some(format!("{}.{}.{}", p[0], p[1], p[2]))
    } else {
        None
    }
}

fn port_open(ip: &str, port: u16) -> bool {
    match format!("{}:{}", ip, port).parse::<SocketAddr>() {
        Ok(a) => TcpStream::connect_timeout(&a, Duration::from_millis(1500)).is_ok(),
        Err(_) => false,
    }
}

fn scan_pass(targets: &[String]) -> Vec<String> {
    let q = Arc::new(Mutex::new(targets.to_vec()));
    let res = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();
    for _ in 0..64 {
        let q = q.clone();
        let res = res.clone();
        handles.push(std::thread::spawn(move || loop {
            let ip = { q.lock().unwrap().pop() };
            let ip = match ip {
                Some(x) => x,
                None => break,
            };
            if port_open(&ip, 22) {
                res.lock().unwrap().push(ip);
            }
        }));
    }
    for h in handles {
        let _ = h.join();
    }
    Arc::try_unwrap(res).unwrap().into_inner().unwrap()
}

fn scan(base: &str, exclude: &HashSet<String>) -> Vec<String> {
    let targets: Vec<String> = (1..=254)
        .map(|o| format!("{}.{}", base, o))
        .filter(|ip| !exclude.contains(ip))
        .collect();
    let open = scan_pass(&targets);
    let openset: HashSet<String> = open.iter().cloned().collect();
    let missed: Vec<String> = targets.into_iter().filter(|t| !openset.contains(t)).collect();
    let mut all = open;
    all.extend(scan_pass(&missed)); // retry dos que não responderam
    let mut uniq: Vec<String> = all.into_iter().collect::<HashSet<_>>().into_iter().collect();
    uniq.sort_by_key(|ip| ip.rsplit('.').next().unwrap_or("0").parse::<u16>().unwrap_or(0));
    uniq
}

fn role_from(host: &str, ips: &str) -> Option<Role> {
    match host.trim().to_lowercase().as_str() {
        "mgm" => return Some(Role::Mgm),
        "n1" => return Some(Role::N1),
        "n2" => return Some(Role::N2),
        _ => {}
    }
    let s = format!(" {} ", ips);
    if s.contains(" 192.168.1.1 ") {
        Some(Role::Mgm)
    } else if s.contains(" 192.168.1.2 ") {
        Some(Role::N1)
    } else if s.contains(" 192.168.1.3 ") {
        Some(Role::N2)
    } else {
        None
    }
}

fn probe(ip: &str, creds: &[(String, String)]) -> (String, Option<Role>, Option<(String, String)>) {
    for (u, p) in creds {
        let out = Command::new("sshpass")
            .args(["-p", p])
            .arg("ssh")
            .args(SSH_OPTS)
            .arg(format!("{}@{}", u, ip))
            .arg("echo H:$(hostname); echo I:$(hostname -I)")
            .output();
        if let Ok(o) = out {
            if o.status.success() {
                let s = String::from_utf8_lossy(&o.stdout);
                let host = s.lines().find_map(|l| l.strip_prefix("H:")).unwrap_or("").trim().to_string();
                let ips = s.lines().find_map(|l| l.strip_prefix("I:")).unwrap_or("").trim().to_string();
                let role = role_from(&host, &ips);
                let intern = ips.split_whitespace().find(|t| t.starts_with("192.168.1."));
                let info = match intern {
                    Some(i) => format!("hostname={}, interna={}", host, i),
                    None => format!("hostname={}", host),
                };
                return (info, role, Some((u.clone(), p.clone())));
            }
        }
    }
    ("login falhou (creds não bateram)".to_string(), None, None)
}

fn inject_one(ip: &str, role: Role, user: &str, pass: &str, exe: &str) -> bool {
    let scp = Command::new("sshpass")
        .args(["-p", pass])
        .arg("scp")
        .args(SSH_OPTS)
        .arg(exe)
        .arg(format!("{}@{}:/tmp/bdd", user, ip))
        .status();
    if !matches!(scp, Ok(s) if s.success()) {
        return false;
    }
    let remote = format!(
        "echo '{p}' | sudo -S sh -c 'install -m 0755 /tmp/bdd /usr/local/bin/bdd && mkdir -p /var/lib/bdd && chmod 777 /var/lib/bdd && rm -f /tmp/bdd' && /usr/local/bin/bdd id {r} >/dev/null",
        p = pass,
        r = role.code()
    );
    let ssh = Command::new("sshpass")
        .args(["-p", pass])
        .arg("ssh")
        .args(SSH_OPTS)
        .arg(format!("{}@{}", user, ip))
        .arg(remote)
        .status();
    matches!(ssh, Ok(s) if s.success())
}

// --------------------------------------------------------------- TUI: creds

fn draw(lines: &[String]) {
    let mut o = stdout();
    let _ = queue!(o, MoveTo(0, 0), Clear(ClearType::All));
    for l in lines {
        let _ = queue!(o, Print(l), Print("\r\n"));
    }
    let _ = o.flush();
}

/// Grade 3x2 (MGM/N1/N2 x user/senha). Retorna os 3 pares (user, senha).
fn tui_creds() -> Option<Vec<(String, String)>> {
    let labels = ["MGM", "N1", "N2"];
    let mut user = [String::new(), String::new(), String::new()];
    let mut pass = [String::new(), String::new(), String::new()];
    let mut active = 0usize; // 0..5 -> row-major: 0,1,2 user ; 3,4,5 pass
    if enable_raw_mode().is_err() {
        return None;
    }
    let result;
    loop {
        let mut lines = vec![
            "Credenciais das VMs  (setas/Tab move, Enter confirma, Esc cancela)".to_string(),
            "Preencha quantas precisar; cada VM e testada com todos os pares.".to_string(),
            String::new(),
        ];
        // cabecalho colunas
        let mut head = String::from("       ");
        for l in &labels {
            head.push_str(&format!("{:<16}", l));
        }
        lines.push(head);
        // linha user
        lines.push(field_row("user ", &user, &['\0'; 3], active, 0));
        // linha senha (mascarada)
        lines.push(field_row("senha", &pass, &['*'; 3], active, 3));
        lines.push(String::new());
        lines.push("Enter = confirmar    Esc = cancelar".to_string());
        draw(&lines);

        let ev = match read() {
            Ok(e) => e,
            Err(_) => {
                result = None;
                break;
            }
        };
        if let Event::Key(k) = ev {
            match k.code {
                KeyCode::Esc => {
                    result = None;
                    break;
                }
                KeyCode::Enter => {
                    result = Some(vec![
                        (user[0].clone(), pass[0].clone()),
                        (user[1].clone(), pass[1].clone()),
                        (user[2].clone(), pass[2].clone()),
                    ]);
                    break;
                }
                KeyCode::Tab | KeyCode::Right => active = (active + 1) % 6,
                KeyCode::BackTab | KeyCode::Left => active = (active + 5) % 6,
                KeyCode::Up => { if active >= 3 { active -= 3; } }
                KeyCode::Down => { if active < 3 { active += 3; } }
                KeyCode::Backspace => {
                    let f = if active < 3 { &mut user[active] } else { &mut pass[active - 3] };
                    f.pop();
                }
                KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::CONTROL) => {
                    let f = if active < 3 { &mut user[active] } else { &mut pass[active - 3] };
                    f.push(c);
                }
                _ => {}
            }
        }
    }
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), Clear(ClearType::All), MoveTo(0, 0));
    result
}

fn field_row(label: &str, vals: &[String; 3], mask: &[char; 3], active: usize, base: usize) -> String {
    let mut s = format!("{}  ", label);
    for i in 0..3 {
        let shown = if mask[i] == '*' {
            "*".repeat(vals[i].chars().count())
        } else {
            vals[i].clone()
        };
        let marker = if active == base + i { ">" } else { " " };
        s.push_str(&format!("{}[{:<12}]", marker, shown));
    }
    s
}

// --------------------------------------------------------------- TUI: select

fn tui_select(cands: &mut [Cand], creds: &[(String, String)]) -> Option<Vec<(Role, String, (String, String))>> {
    // aplica sugestões iniciais (sem conflito)
    for r in [Role::Mgm, Role::N1, Role::N2] {
        let hits: Vec<usize> = cands.iter().enumerate().filter(|(_, c)| c.suggest == Some(r) && c.creds.is_some()).map(|(i, _)| i).collect();
        if hits.len() == 1 {
            cands[hits[0]].role = Some(r);
        }
    }
    let mut cursor = 0usize;
    if enable_raw_mode().is_err() {
        return None;
    }
    let mut manual: Vec<Cand> = Vec::new();
    let result;
    loop {
        let total = cands.len() + manual.len();
        let mut lines = vec![
            "Selecione as VMs  (setas movem, Enter define papel, 'a' adiciona IP, F2 confirma, Esc cancela)".to_string(),
            String::new(),
        ];
        let all: Vec<&Cand> = cands.iter().chain(manual.iter()).collect();
        if all.is_empty() {
            lines.push("  (nenhuma VM; use 'a' para adicionar um IP na mão)".to_string());
        }
        for (i, c) in all.iter().enumerate() {
            let cur = if i == cursor { ">" } else { " " };
            let role = c.role.map(|r| format!("[{}]", r.name())).unwrap_or_else(|| {
                c.suggest.map(|r| format!("(sug: {})", r.name())).unwrap_or_default()
            });
            let acc = if c.creds.is_some() { "" } else { "  (sem acesso)" };
            lines.push(format!("{} {:<15} {:<34} {} {}", cur, c.ip, c.info, role, acc));
        }
        lines.push(String::new());
        lines.push("Enter=papel   a=adicionar IP   F2=confirmar   Esc=cancelar".to_string());
        draw(&lines);

        let ev = match read() {
            Ok(e) => e,
            Err(_) => { result = None; break; }
        };
        if let Event::Key(k) = ev {
            match k.code {
                KeyCode::Esc => { result = None; break; }
                KeyCode::F(2) => {
                    let mut out = Vec::new();
                    for c in cands.iter().chain(manual.iter()) {
                        if let (Some(r), Some(cr)) = (c.role, c.creds.clone()) {
                            out.push((r, c.ip.clone(), cr));
                        }
                    }
                    result = Some(out);
                    break;
                }
                KeyCode::Up => { if cursor > 0 { cursor -= 1; } }
                KeyCode::Down => { if total > 0 && cursor + 1 < total { cursor += 1; } }
                KeyCode::Char('a') => {
                    if let Some(c) = manual_add(creds) {
                        manual.push(c);
                    }
                    let _ = enable_raw_mode();
                }
                KeyCode::Enter => {
                    if total == 0 { continue; }
                    let chosen = pick_role();
                    let _ = enable_raw_mode();
                    // aplica ao cursor; remove o papel de quem já tinha
                    let n_scan = cands.len();
                    let set_role = |slot: &mut Option<Role>| *slot = chosen;
                    if let Some(role) = chosen {
                        for c in cands.iter_mut() { if c.role == Some(role) { c.role = None; } }
                        for c in manual.iter_mut() { if c.role == Some(role) { c.role = None; } }
                        if cursor < n_scan { set_role(&mut cands[cursor].role); }
                        else { set_role(&mut manual[cursor - n_scan].role); }
                    } else {
                        // remover papel
                        if cursor < n_scan { cands[cursor].role = None; }
                        else { manual[cursor - n_scan].role = None; }
                    }
                }
                _ => {}
            }
        }
    }
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), Clear(ClearType::All), MoveTo(0, 0));
    result
}

/// Menu de papel. Some(Some(role)) = atribui; Some(None) tratado como remover.
fn pick_role() -> Option<Role> {
    let opts = [Some(Role::Mgm), Some(Role::N1), Some(Role::N2), None];
    let names = ["MGM", "N1", "N2", "(remover papel)"];
    let mut sel = 0usize;
    loop {
        let mut lines = vec!["Papel desta VM:".to_string(), String::new()];
        for (i, n) in names.iter().enumerate() {
            lines.push(format!("{} {}", if i == sel { ">" } else { " " }, n));
        }
        lines.push(String::new());
        lines.push("setas + Enter; Esc cancela".to_string());
        draw(&lines);
        if let Ok(Event::Key(k)) = read() {
            match k.code {
                KeyCode::Up => { if sel > 0 { sel -= 1; } }
                KeyCode::Down => { if sel + 1 < opts.len() { sel += 1; } }
                KeyCode::Enter => return opts[sel],
                KeyCode::Esc => return None,
                _ => {}
            }
        }
    }
}

/// Adiciona um IP na mão: pede IP, testa conexão com os pares, devolve Cand.
fn manual_add(creds: &[(String, String)]) -> Option<Cand> {
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), Clear(ClearType::All), MoveTo(0, 0));
    print!("IP para adicionar (vazio cancela): ");
    let _ = stdout().flush();
    let mut ip = String::new();
    if std::io::stdin().read_line(&mut ip).is_err() {
        return None;
    }
    let ip = ip.trim().to_string();
    if ip.is_empty() {
        return None;
    }
    println!("testando {} ...", ip);
    let (info, suggest, creds_ok) = probe(&ip, creds);
    if creds_ok.is_none() {
        println!("não consegui conectar em {} com as credenciais dadas.", ip);
    }
    Some(Cand { ip, info, suggest, creds: creds_ok, role: None })
}
