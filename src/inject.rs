//! `bdd inject` (roda no HOST): TUI para achar as VMs, marcar a função de cada
//! uma (MGM/N1/N2) e instalar o bdd por SSH. Renderiza "em linha" (sem limpar o
//! terminal) num painel de tamanho fixo.

use crate::model::Role;
use crate::ui;
use std::collections::HashSet;
use std::io::{stdout, Write};
use std::net::{SocketAddr, TcpStream};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossterm::cursor::{Hide, MoveToColumn, MoveToPreviousLine, Show};
use crossterm::event::{read, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::style::Print;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType};
use crossterm::{execute, queue};

const SSH_OPTS: &[&str] = &[
    "-o", "StrictHostKeyChecking=no",
    "-o", "UserKnownHostsFile=/dev/null",
    "-o", "ConnectTimeout=6",
    "-o", "PubkeyAuthentication=no",
    "-o", "PreferredAuthentications=password",
    "-o", "LogLevel=ERROR", // silencia "Permanently added" e avisos do cliente
];

const BG: &str = "\x1b[48;5;236m";
const RESET: &str = "\x1b[0m";
const SR: &str = "\x1b[22;39m";
const FG_BRIGHT: &str = "\x1b[1m\x1b[97m";
const FG_NORM: &str = "\x1b[37m";
const FG_DIM: &str = "\x1b[90m";
const FG_CYAN: &str = "\x1b[1m\x1b[96m";
const FG_CYAN_DIM: &str = "\x1b[36m";

const W: usize = 12; // largura interna da caixa de input
const SEG: usize = W + 4; // marcador(2)+borda(2)+interno(W)
const GUT: usize = 7; // gutter dos rótulos de linha
const PW: usize = GUT + 3 * (SEG + 1); // largura fixa do painel (3 colunas)
const PH: usize = 14; // altura fixa do corpo do painel

#[derive(Clone, Copy, PartialEq)]
enum ColLabel {
    Any,
    Mgm,
    N1,
    N2,
}
impl ColLabel {
    fn text(self) -> &'static str {
        match self {
            ColLabel::Any => "Any",
            ColLabel::Mgm => "MGM",
            ColLabel::N1 => "N1",
            ColLabel::N2 => "N2",
        }
    }
    fn role(self) -> Option<Role> {
        match self {
            ColLabel::Mgm => Some(Role::Mgm),
            ColLabel::N1 => Some(Role::N1),
            ColLabel::N2 => Some(Role::N2),
            ColLabel::Any => None,
        }
    }
}

#[derive(Clone)]
struct Cred {
    label: ColLabel,
    user: String,
    pass: String,
}
impl Cred {
    fn filled(&self) -> bool {
        !self.user.is_empty() || !self.pass.is_empty()
    }
}

struct Cand {
    ip: String,
    host: String,
    intern: String,
    suggest: Option<Role>,
    working: Vec<Cred>, // credenciais que autenticaram
    role: Option<Role>,
}
impl Cand {
    fn accessible(&self) -> bool {
        !self.working.is_empty()
    }
    /// credencial a usar para a função atribuída (label da função, senão Any, senão a 1a que funcionou)
    fn cred_for(&self, r: Role) -> Option<&Cred> {
        self.working.iter().find(|c| c.label.role() == Some(r))
            .or_else(|| self.working.iter().find(|c| c.label == ColLabel::Any))
            .or_else(|| self.working.first())
    }
}

pub fn run() {
    if std::env::var("BDD_INJECT_PREVIEW").is_ok() {
        preview();
        return;
    }
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
        Err(_) => { eprintln!("[inject] não achei o próprio binário."); return; }
    };
    let host_ips = host_ips();
    let base = match host_ips.iter().find_map(|ip| subnet_base(ip)) {
        Some(b) => b,
        None => { eprintln!("[inject] não achei a rede do host."); return; }
    };

    let creds = match tui_creds() {
        Some(c) if !c.is_empty() => c,
        _ => { println!("[inject] cancelado."); return; }
    };

    let exclude: HashSet<String> = host_ips.into_iter().collect();
    let chosen = match tui_select(&base, &exclude, &creds) {
        Some(v) if !v.is_empty() => v,
        Some(_) => { println!("[inject] nada selecionado."); return; }
        None => { println!("[inject] cancelado."); return; }
    };

    println!();
    let exe = bin.to_string_lossy().to_string();
    for (role, ip, cred) in &chosen {
        print!("[inject] {} ({}) ... ", role.name(), ip);
        let _ = stdout().flush();
        if inject_one(ip, *role, &cred.user, &cred.pass, &exe) {
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
        "e então:    bdd next    (mostra qual é o próximo passo)".to_string(),
    ]);
}

// --------------------------------------------------------------- rede / ssh

fn have(cmd: &str) -> bool {
    Command::new("sh").arg("-c").arg(format!("command -v {} >/dev/null 2>&1", cmd)).status().map(|s| s.success()).unwrap_or(false)
}

fn host_ips() -> Vec<String> {
    let mut v = Vec::new();
    if let Ok(o) = Command::new("ip").args(["-o", "-4", "addr", "show", "scope", "global"]).output() {
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            if let Some(cidr) = line.split_whitespace().nth(3) {
                if let Some(ip) = cidr.split('/').next() {
                    if !ip.starts_with("127.") {
                        v.push(ip.to_string());
                    }
                }
            }
        }
    }
    if v.is_empty() {
        if let Ok(o) = Command::new("hostname").arg("-I").output() {
            for t in String::from_utf8_lossy(&o.stdout).split_whitespace() {
                if t.contains('.') && !t.starts_with("127.") {
                    v.push(t.to_string());
                }
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
        Ok(a) => TcpStream::connect_timeout(&a, Duration::from_millis(2000)).is_ok(),
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
            let ip = match ip { Some(x) => x, None => break };
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
    let mut targets: Vec<String> = (1..=254)
        .map(|o| format!("{}.{}", base, o))
        .filter(|ip| !exclude.contains(ip))
        .collect();
    let mut found: HashSet<String> = HashSet::new();
    for _ in 0..3 {
        // re-tenta só os que ainda não responderam (cobre VMs lentas)
        targets.retain(|t| !found.contains(t));
        if targets.is_empty() {
            break;
        }
        for ip in scan_pass(&targets) {
            found.insert(ip);
        }
    }
    let mut v: Vec<String> = found.into_iter().collect();
    v.sort_by_key(|ip| ip.rsplit('.').next().unwrap_or("0").parse::<u16>().unwrap_or(0));
    v
}

fn role_from(host: &str, ips: &str) -> Option<Role> {
    match host.trim().to_lowercase().as_str() {
        "mgm" => return Some(Role::Mgm),
        "n1" => return Some(Role::N1),
        "n2" => return Some(Role::N2),
        _ => {}
    }
    let s = format!(" {} ", ips);
    if s.contains(" 192.168.1.1 ") { Some(Role::Mgm) }
    else if s.contains(" 192.168.1.2 ") { Some(Role::N1) }
    else if s.contains(" 192.168.1.3 ") { Some(Role::N2) }
    else { None }
}

/// Tenta cada credencial; devolve (host, interna, suggest, creds que funcionaram).
fn probe(ip: &str, creds: &[Cred]) -> (String, String, Option<Role>, Vec<Cred>) {
    let mut working = Vec::new();
    let mut host = String::new();
    let mut intern = String::new();
    let mut suggest = None;
    for c in creds {
        let out = Command::new("sshpass")
            .args(["-p", &c.pass])
            .arg("ssh")
            .args(SSH_OPTS)
            .arg(format!("{}@{}", c.user, ip))
            .arg("echo H:$(hostname); echo I:$(hostname -I)")
            .output();
        if let Ok(o) = out {
            if o.status.success() {
                working.push(c.clone());
                if host.is_empty() {
                    let s = String::from_utf8_lossy(&o.stdout);
                    host = s.lines().find_map(|l| l.strip_prefix("H:")).unwrap_or("").trim().to_string();
                    let ips = s.lines().find_map(|l| l.strip_prefix("I:")).unwrap_or("").trim().to_string();
                    suggest = role_from(&host, &ips);
                    intern = ips.split_whitespace().find(|t| t.starts_with("192.168.1.")).unwrap_or("").to_string();
                }
            }
        }
    }
    (host, intern, suggest, working)
}

fn inject_one(ip: &str, role: Role, user: &str, pass: &str, exe: &str) -> bool {
    // saída capturada: só mostra se algo falhar (evita o ruído de avisos do ssh/sudo)
    let scp = Command::new("sshpass").args(["-p", pass]).arg("scp").args(SSH_OPTS).arg(exe).arg(format!("{}@{}:/tmp/bdd", user, ip)).output();
    match &scp {
        Ok(o) if o.status.success() => {}
        Ok(o) => { eprint!("\n{}", String::from_utf8_lossy(&o.stderr)); return false; }
        Err(_) => return false,
    }
    let remote = format!(
        "echo '{p}' | sudo -S -p '' sh -c 'install -m 0755 /tmp/bdd /usr/local/bin/bdd && mkdir -p /var/lib/bdd && chmod 777 /var/lib/bdd && rm -f /tmp/bdd' && /usr/local/bin/bdd id {r} >/dev/null",
        p = pass, r = role.code()
    );
    let ssh = Command::new("sshpass").args(["-p", pass]).arg("ssh").args(SSH_OPTS).arg(format!("{}@{}", user, ip)).arg(remote).output();
    match ssh {
        Ok(o) if o.status.success() => true,
        Ok(o) => { eprint!("\n{}", String::from_utf8_lossy(&o.stderr)); false }
        Err(_) => false,
    }
}

// --------------------------------------------------------------- tela inline

struct Screen {
    prev: u16,
}
impl Screen {
    fn new() -> Self {
        Screen { prev: 0 }
    }
    fn render(&mut self, lines: &[String]) {
        let mut o = stdout();
        if self.prev > 0 {
            let _ = queue!(o, MoveToPreviousLine(self.prev));
        }
        let _ = queue!(o, MoveToColumn(0), Clear(ClearType::FromCursorDown));
        for (i, l) in lines.iter().enumerate() {
            let _ = queue!(o, Print(l));
            if i + 1 < lines.len() {
                let _ = queue!(o, Print("\r\n"));
            }
        }
        let _ = o.flush();
        self.prev = lines.len().saturating_sub(1) as u16;
    }
    fn clear(&mut self) {
        let mut o = stdout();
        if self.prev > 0 {
            let _ = queue!(o, MoveToPreviousLine(self.prev));
        }
        let _ = queue!(o, MoveToColumn(0), Clear(ClearType::FromCursorDown));
        let _ = o.flush();
        self.prev = 0;
    }
}

/// Roda `job` numa thread e anima um spinner no título até terminar.
fn spin<T: Send + 'static>(scr: &mut Screen, log: &[String], job: impl FnOnce() -> T + Send + 'static) -> T {
    let h = std::thread::spawn(job);
    let frames = ['\u{280b}', '\u{2819}', '\u{2839}', '\u{2838}', '\u{283c}', '\u{2834}', '\u{2826}', '\u{2827}', '\u{2807}', '\u{280f}'];
    let mut i = 0usize;
    while !h.is_finished() {
        let title = format!("Procurando VMs {}", frames[i % frames.len()]);
        scr.render(&frame(&title, log_body(log)));
        std::thread::sleep(Duration::from_millis(90));
        i += 1;
    }
    h.join().unwrap()
}

fn key_press() -> Option<crossterm::event::KeyEvent> {
    loop {
        match read() {
            Ok(Event::Key(k)) if k.kind == KeyEventKind::Press => return Some(k),
            Ok(_) => continue,
            Err(_) => return None,
        }
    }
}

// painel ----------------------------------------------------------------

fn visible_len(s: &str) -> usize {
    let mut n = 0;
    let mut esc = false;
    for ch in s.chars() {
        if esc {
            if ch == 'm' { esc = false; }
            continue;
        }
        if ch == '\x1b' { esc = true; continue; }
        n += 1;
    }
    n
}

fn pad(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w { s.chars().take(w).collect() } else { format!("{}{}", s, " ".repeat(w - n)) }
}
fn center(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w { return s.chars().take(w).collect(); }
    let l = (w - n) / 2;
    format!("{}{}{}", " ".repeat(l), s, " ".repeat(w - n - l))
}
fn fg(code: &str, s: &str) -> String {
    format!("{}{}{}", code, s, SR)
}

fn panel_top(title: &str) -> String {
    let head = format!("\u{256d}\u{2500} {} ", title);
    let dashes = (PW + 4).saturating_sub(visible_len(&head) + 1);
    format!("{}{}{}{}\u{256e}{}", BG, FG_DIM, head, "\u{2500}".repeat(dashes), RESET)
}
fn panel_bottom() -> String {
    format!("{}{}\u{2570}{}\u{256f}{}", BG, FG_DIM, "\u{2500}".repeat(PW + 2), RESET)
}
fn clip(s: &str, max: usize) -> String {
    let mut out = String::new();
    let mut vis = 0;
    let mut esc = false;
    for ch in s.chars() {
        if esc {
            out.push(ch);
            if ch == 'm' { esc = false; }
            continue;
        }
        if ch == '\x1b' {
            esc = true;
            out.push(ch);
            continue;
        }
        if vis >= max { continue; }
        out.push(ch);
        vis += 1;
    }
    out
}
fn panel_line(content: &str) -> String {
    let content = clip(content, PW);
    let padn = PW.saturating_sub(visible_len(&content));
    format!("{}{}\u{2502} {}{}{} \u{2502}{}", BG, FG_DIM, content, SR, " ".repeat(padn), RESET)
}

/// Envolve `body` (já PW de largura) num painel de altura fixa PH.
fn frame(title: &str, body: Vec<String>) -> Vec<String> {
    let mut out = vec![panel_top(title)];
    for i in 0..PH {
        out.push(panel_line(body.get(i).map(|s| s.as_str()).unwrap_or("")));
    }
    out.push(panel_bottom());
    out
}

fn boxed_button(txt: &str, focus: bool) -> [String; 3] {
    let inner = format!(" {} ", txt);
    let w = inner.chars().count();
    let code = if focus { FG_CYAN } else { FG_NORM };
    let tcode = if focus { FG_BRIGHT } else { FG_NORM };
    [
        fg(code, &format!("\u{250c}{}\u{2510}", "\u{2500}".repeat(w))),
        format!("{}{}{}", fg(code, "\u{2502}"), fg(tcode, &inner), fg(code, "\u{2502}")),
        fg(code, &format!("\u{2514}{}\u{2518}", "\u{2500}".repeat(w))),
    ]
}
fn button_box_w(txt: &str) -> usize {
    txt.chars().count() + 4
}

/// Caixa de input ascii de 3 linhas, com o rótulo embutido na borda de cima.
fn boxed_input(title: &str, content: &str, focus: bool, w: usize) -> [String; 3] {
    let code = if focus { FG_CYAN } else { FG_NORM };
    let tcode = if focus { FG_BRIGHT } else { FG_NORM };
    let head = format!("\u{2500} {} ", title); // ─ titulo
    let dashes = w.saturating_sub(visible_len(&head));
    [
        fg(code, &format!("\u{250c}{}{}\u{2510}", head, "\u{2500}".repeat(dashes))),
        format!("{}{}{}", fg(code, "\u{2502}"), fg(tcode, &pad(content, w)), fg(code, "\u{2502}")),
        fg(code, &format!("\u{2514}{}\u{2518}", "\u{2500}".repeat(w))),
    ]
}
/// 3 linhas com Enter/Esc centralizados.
fn buttons_rows(enter_focus: bool, esc_focus: bool) -> [String; 3] {
    let eb = boxed_button("Enter", enter_focus);
    let sb = boxed_button("Esc", esc_focus);
    let gap = 3;
    let combined = button_box_w("Enter") + gap + button_box_w("Esc");
    let lp = " ".repeat(PW.saturating_sub(combined) / 2);
    let g = " ".repeat(gap);
    [
        format!("{}{}{}{}", lp, eb[0], g, sb[0]),
        format!("{}{}{}{}", lp, eb[1], g, sb[1]),
        format!("{}{}{}{}", lp, eb[2], g, sb[2]),
    ]
}

// --------------------------------------------------------------- TUI: creds

// foco creds: (row,col). row 0=label,1=user,2=pass,3=botoes; col em botoes 0=Enter,1=Esc
fn tui_creds() -> Option<Vec<Cred>> {
    let mut cols: Vec<Cred> = vec![Cred { label: ColLabel::Any, user: String::new(), pass: String::new() }];
    let (mut row, mut col) = (1usize, 0usize);
    let mut scr = Screen::new();
    if enable_raw_mode().is_err() {
        return None;
    }
    let _ = execute!(stdout(), Hide);
    let result;
    loop {
        compact_cols(&mut cols);
        normalize_labels(&mut cols);
        let ncols = cols.len();
        if row != 3 && col >= ncols { col = ncols - 1; }
        if row == 3 && col > 1 { col = 1; }

        scr.render(&frame("Credenciais", render_creds_body(&cols, row, col)));

        let k = match key_press() { Some(k) => k, None => { result = None; break; } };
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        match k.code {
            KeyCode::Esc => { result = None; break; }
            KeyCode::Tab => tab_next(&mut row, &mut col, ncols),
            KeyCode::BackTab => tab_prev(&mut row, &mut col, ncols),
            KeyCode::Up => move2(&mut row, &mut col, -1, 0, ncols),
            KeyCode::Down => move2(&mut row, &mut col, 1, 0, ncols),
            KeyCode::Left => move2(&mut row, &mut col, 0, -1, ncols),
            KeyCode::Right => move2(&mut row, &mut col, 0, 1, ncols),
            KeyCode::Char('u') | KeyCode::Char('U') if ctrl => {
                if row == 1 { cols[col].user.clear(); }
                if row == 2 { cols[col].pass.clear(); }
            }
            KeyCode::Enter => {
                if row == 0 {
                    cycle_label(&mut cols, col);
                } else if row == 3 {
                    if col == 0 { result = Some(collect(&cols)); } else { result = None; }
                    break;
                } else {
                    result = Some(collect(&cols));
                    break;
                }
            }
            KeyCode::Backspace => {
                if row == 1 { cols[col].user.pop(); }
                if row == 2 { cols[col].pass.pop(); }
            }
            KeyCode::Char(c) if !ctrl => {
                if row == 1 && cols[col].user.chars().count() < W { cols[col].user.push(c); }
                if row == 2 && cols[col].pass.chars().count() < W { cols[col].pass.push(c); }
            }
            _ => {}
        }
    }
    scr.clear();
    let _ = execute!(stdout(), Show);
    let _ = disable_raw_mode();
    match result {
        Some(v) if !v.is_empty() => Some(v),
        Some(_) => Some(Vec::new()),
        None => None,
    }
}

/// mantém: colunas preenchidas + no máximo UMA vazia (no fim).
fn compact_cols(cols: &mut Vec<Cred>) {
    let mut kept: Vec<Cred> = cols.drain(..).filter(|c| c.filled()).collect();
    if kept.len() < 3 {
        kept.push(Cred { label: ColLabel::Any, user: String::new(), pass: String::new() });
    }
    *cols = kept;
}

// só colunas PREENCHIDAS contam (uma coluna vazia não reserva o rótulo)
fn labels_used_by_others(cols: &[Cred], idx: usize) -> Vec<ColLabel> {
    cols.iter().enumerate().filter(|(i, c)| *i != idx && c.filled()).map(|(_, c)| c.label).collect()
}

fn options_for(cols: &[Cred], idx: usize) -> Vec<ColLabel> {
    let used = labels_used_by_others(cols, idx);
    let nfilled = cols.iter().filter(|c| c.filled()).count();
    let mut opts = Vec::new();
    if nfilled < 3 && !used.contains(&ColLabel::Any) {
        opts.push(ColLabel::Any);
    }
    for l in [ColLabel::Mgm, ColLabel::N1, ColLabel::N2] {
        if !used.contains(&l) {
            opts.push(l);
        }
    }
    opts
}

fn normalize_labels(cols: &mut [Cred]) {
    for i in 0..cols.len() {
        let opts = options_for(cols, i);
        if !opts.contains(&cols[i].label) {
            cols[i].label = opts.first().copied().unwrap_or(ColLabel::Any);
        }
    }
}

fn cycle_label(cols: &mut [Cred], idx: usize) {
    let opts = options_for(cols, idx);
    if opts.is_empty() {
        return;
    }
    let cur = opts.iter().position(|l| *l == cols[idx].label).unwrap_or(0);
    cols[idx].label = opts[(cur + 1) % opts.len()];
}

fn collect(cols: &[Cred]) -> Vec<Cred> {
    cols.iter().filter(|c| !c.user.is_empty()).cloned().collect()
}

// navegação creds (col: nº de colunas reais)
fn tab_next(row: &mut usize, col: &mut usize, ncols: usize) {
    if *row == 3 {
        if *col == 0 { *col = 1; } else { *row = 0; *col = 0; }
    } else if *row < 2 {
        *row += 1;
    } else {
        *row = 0;
        if *col + 1 < ncols { *col += 1; } else { *row = 3; *col = 0; }
    }
}
fn tab_prev(row: &mut usize, col: &mut usize, ncols: usize) {
    if *row == 3 {
        if *col == 1 { *col = 0; } else { *row = 2; *col = ncols - 1; }
    } else if *row > 0 {
        *row -= 1;
    } else {
        *row = 2;
        if *col > 0 { *col -= 1; } else { *row = 3; *col = 1; }
    }
}
fn move2(row: &mut usize, col: &mut usize, dr: i32, dc: i32, ncols: usize) {
    let nr = (*row as i32 + dr).clamp(0, 3) as usize;
    *row = nr;
    let maxc = if *row == 3 { 1 } else { ncols.saturating_sub(1) };
    *col = (*col as i32 + dc).clamp(0, maxc as i32) as usize;
    if *col > maxc { *col = maxc; }
}

fn render_creds_body(cols: &[Cred], frow: usize, fcol: usize) -> Vec<String> {
    let mut body = Vec::new();
    body.push(String::new());
    // 7 linhas da grade
    let row_label = |k: usize| match k { 2 => "user", 5 => "senha", _ => "" };
    for kind in 0..7 {
        let mut line = fg(FG_DIM, &pad(&format!("{:>5} ", row_label(kind)), GUT));
        for slot in 0..3 {
            let seg = if slot < cols.len() {
                col_seg(&cols[slot], kind, slot, frow, fcol)
            } else if slot == cols.len() && cols.len() < 3 {
                placeholder_seg(kind)
            } else {
                fg(FG_DIM, &" ".repeat(SEG))
            };
            line.push_str(&seg);
            line.push(' ');
        }
        body.push(line);
    }
    body.push(String::new());
    let b = buttons_rows(frow == 3 && fcol == 0, frow == 3 && fcol == 1);
    body.push(b[0].clone());
    body.push(b[1].clone());
    body.push(b[2].clone());
    let hint = match frow {
        1 | 2 => fg(FG_DIM, "Ctrl+U limpa o campo  -  Tab move  -  Enter confirma"),
        0 => fg(FG_DIM, "Enter cicla o rotulo (so opcoes livres)  -  Tab move"),
        _ => String::new(),
    };
    body.push(hint);
    body
}

fn col_seg(c: &Cred, kind: usize, ci: usize, frow: usize, fcol: usize) -> String {
    let col_focused = fcol == ci && frow <= 2;
    let label_focused = frow == 0 && fcol == ci;
    let user_focused = frow == 1 && fcol == ci;
    let pass_focused = frow == 2 && fcol == ci;
    let unfilled = !c.filled();
    let base = if unfilled { FG_DIM } else { FG_NORM };
    match kind {
        0 => {
            let mut lbl = c.label.text().to_string();
            if label_focused { lbl.push_str(" \u{21bb}"); }
            let code = if label_focused { FG_CYAN } else if col_focused { FG_BRIGHT } else if unfilled { FG_DIM } else { FG_NORM };
            fg(code, &center(&lbl, SEG))
        }
        1 | 4 => {
            let focused = if kind == 1 { user_focused } else { pass_focused };
            let code = if focused { FG_CYAN } else { base };
            fg(code, &format!("  \u{250c}{}\u{2510}", "\u{2500}".repeat(W)))
        }
        3 | 6 => {
            let focused = if kind == 3 { user_focused } else { pass_focused };
            let code = if focused { FG_CYAN } else { base };
            fg(code, &format!("  \u{2514}{}\u{2518}", "\u{2500}".repeat(W)))
        }
        2 | 5 => {
            let (focused, content) = if kind == 2 {
                (user_focused, c.user.clone())
            } else {
                (pass_focused, "*".repeat(c.pass.chars().count()))
            };
            let marker = if focused { fg(FG_CYAN, "\u{25b6} ") } else { "  ".to_string() };
            let bcode = if focused { FG_CYAN } else { base };
            let tcode = if focused { FG_BRIGHT } else { base };
            format!("{}{}{}{}", marker, fg(bcode, "\u{2502}"), fg(tcode, &pad(&content, W)), fg(bcode, "\u{2502}"))
        }
        _ => " ".repeat(SEG),
    }
}

fn placeholder_seg(kind: usize) -> String {
    match kind {
        0 => fg(FG_DIM, &center("+", SEG)),
        _ => fg(FG_DIM, &" ".repeat(SEG)),
    }
}

// --------------------------------------------------------------- TUI: select

const LROWS: usize = 6; // linhas visíveis da lista

#[derive(PartialEq)]
enum Sel {
    List,
    AddIp,
    AddRole,
    Enter,
    Esc,
}

fn tui_select(base: &str, exclude: &HashSet<String>, creds: &[Cred]) -> Option<Vec<(Role, String, Cred)>> {
    let mut scr = Screen::new();
    if enable_raw_mode().is_err() {
        return None;
    }
    let _ = execute!(stdout(), Hide);

    // ---- carregamento com logs no painel (com spinner enquanto bloqueia) ----
    let mut log: Vec<String> = Vec::new();
    log.push("escaneando a rede...".to_string());
    let base2 = base.to_string();
    let ex = exclude.clone();
    let ips = spin(&mut scr, &log, move || scan(&base2, &ex));
    log.push(format!("{} host(s) com SSH encontrados.", ips.len()));

    let mut cands: Vec<Cand> = Vec::new();
    for ip in &ips {
        log.push(format!("testando credenciais em {} ...", ip));
        let ipc = ip.clone();
        let cr = creds.to_vec();
        let (host, intern, suggest, working) = spin(&mut scr, &log, move || probe(&ipc, &cr));
        if working.is_empty() {
            log.push(format!("  {}: sem acesso", ip));
        } else {
            let extra = suggest.map(|r| format!(" ({})", r.name())).unwrap_or_default();
            log.push(format!("  {}: acesso ok{}", ip, extra));
        }
        scr.render(&frame("Procurando VMs", log_body(&log)));
        cands.push(Cand { ip: ip.clone(), host, intern, suggest, working, role: None });
    }
    log.push("faltou VM? cheque o IP dela ou adicione abaixo.".to_string());
    scr.render(&frame("Procurando VMs", log_body(&log)));

    // ordena: bons candidatos (acessiveis com sugestao) primeiro MGM>N1>N2, resto por IP
    sort_cands(&mut cands);
    for c in cands.iter_mut() {
        if c.accessible() {
            c.role = c.suggest;
        }
    }
    fix_roles(&mut cands);

    // ---- interação ----
    let mut focus = Sel::List;
    let mut li = 0usize;
    let mut top = 0usize;
    let mut add_ip = String::new();
    let mut add_role: Option<Role> = None;
    let result;
    loop {
        let total = cands.len();
        if li >= total && total > 0 { li = total - 1; }
        if li < top { top = li; }
        if li >= top + LROWS { top = li + 1 - LROWS; }
        let show_add = !all_roles_taken(&cands);
        if !show_add && (focus == Sel::AddIp || focus == Sel::AddRole) {
            focus = Sel::List;
        }

        scr.render(&frame("Selecione as VMs", select_body(&cands, &focus, li, top, show_add, &add_ip, add_role)));

        let k = match key_press() { Some(k) => k, None => { result = None; break; } };
        match focus {
            Sel::List => match k.code {
                KeyCode::Esc => { result = None; break; }
                KeyCode::Tab => focus = if show_add { Sel::AddIp } else { Sel::Enter },
                KeyCode::BackTab => focus = Sel::Esc,
                KeyCode::Up => {
                    if total > 0 {
                        if li == 0 { li = total - 1; } else { li -= 1; }
                    }
                }
                KeyCode::Down => {
                    if total == 0 || li + 1 >= total {
                        focus = if show_add { Sel::AddIp } else { Sel::Enter };
                    } else {
                        li += 1;
                    }
                }
                KeyCode::Enter => {
                    if total > 0 {
                        if let Some(r) = pick_role_inline(&mut scr, &cands, li) {
                            assign_role(&mut cands, li, r);
                        } else {
                            cands[li].role = None;
                        }
                        let _ = execute!(stdout(), Hide);
                    }
                }
                _ => {}
            },
            Sel::AddIp => match k.code {
                KeyCode::Esc => { result = None; break; }
                KeyCode::Tab => focus = if add_ip.is_empty() { Sel::Enter } else { Sel::AddRole },
                KeyCode::BackTab => focus = Sel::List,
                KeyCode::Up => focus = Sel::List,
                KeyCode::Down => focus = Sel::Enter,
                KeyCode::Backspace => { add_ip.pop(); }
                KeyCode::Char(c) if !c.is_whitespace() => add_ip.push(c),
                KeyCode::Enter => do_add(&mut cands, &mut scr, &mut add_ip, &mut add_role, creds),
                _ => {}
            },
            Sel::AddRole => match k.code {
                KeyCode::Esc => { result = None; break; }
                KeyCode::Tab => focus = Sel::Enter,
                KeyCode::BackTab => focus = Sel::AddIp,
                KeyCode::Left => add_role = cycle_free_role(&cands, add_role, false),
                KeyCode::Right => add_role = cycle_free_role(&cands, add_role, true),
                KeyCode::Up => focus = Sel::AddIp,
                KeyCode::Down => focus = Sel::Enter,
                KeyCode::Enter => do_add(&mut cands, &mut scr, &mut add_ip, &mut add_role, creds),
                _ => {}
            },
            Sel::Enter => match k.code {
                KeyCode::Esc => { result = None; break; }
                KeyCode::Tab => focus = Sel::Esc,
                KeyCode::BackTab => focus = if show_add { Sel::AddIp } else { Sel::List },
                KeyCode::Up => focus = if show_add { Sel::AddIp } else { Sel::List },
                KeyCode::Right => focus = Sel::Esc,
                KeyCode::Enter => { result = Some(gather(&cands)); break; }
                _ => {}
            },
            Sel::Esc => match k.code {
                KeyCode::Esc => { result = None; break; }
                KeyCode::Tab => focus = Sel::List,
                KeyCode::BackTab | KeyCode::Left => focus = Sel::Enter,
                KeyCode::Up => focus = if show_add { Sel::AddIp } else { Sel::List },
                KeyCode::Enter => { result = None; break; }
                _ => {}
            },
        }
    }
    scr.clear();
    let _ = execute!(stdout(), Show);
    let _ = disable_raw_mode();
    result
}

fn log_body(log: &[String]) -> Vec<String> {
    let mut body = Vec::new();
    body.push(fg(FG_BRIGHT, "Carregando..."));
    body.push(String::new());
    let start = log.len().saturating_sub(PH - 2);
    for l in &log[start..] {
        body.push(fg(FG_NORM, &pad(l, PW)));
    }
    body
}

fn sort_cands(cands: &mut [Cand]) {
    cands.sort_by(|a, b| {
        let rank = |c: &Cand| -> (u8, u32) {
            let good = c.accessible() && c.suggest.is_some();
            let r = match c.suggest {
                Some(Role::Mgm) => 0,
                Some(Role::N1) => 1,
                Some(Role::N2) => 2,
                None => 9,
            };
            let ipnum = c.ip.rsplit('.').next().unwrap_or("0").parse::<u32>().unwrap_or(0);
            if good { (r, 0) } else { (8, ipnum) }
        };
        rank(a).cmp(&rank(b))
    });
}

fn all_roles_taken(cands: &[Cand]) -> bool {
    [Role::Mgm, Role::N1, Role::N2].iter().all(|r| cands.iter().any(|c| c.role == Some(*r)))
}
fn first_free_role(cands: &[Cand]) -> Option<Role> {
    [Role::Mgm, Role::N1, Role::N2].into_iter().find(|r| !cands.iter().any(|c| c.role == Some(*r)))
}
fn cycle_free_role(cands: &[Cand], cur: Option<Role>, fwd: bool) -> Option<Role> {
    let free: Vec<Role> = [Role::Mgm, Role::N1, Role::N2].into_iter().filter(|r| !cands.iter().any(|c| c.role == Some(*r))).collect();
    if free.is_empty() { return None; }
    let idx = cur.and_then(|c| free.iter().position(|r| *r == c)).unwrap_or(0);
    let n = free.len();
    Some(if fwd { free[(idx + 1) % n] } else { free[(idx + n - 1) % n] })
}
fn assign_role(cands: &mut [Cand], i: usize, r: Role) {
    for c in cands.iter_mut() {
        if c.role == Some(r) { c.role = None; }
    }
    cands[i].role = Some(r);
}
fn fix_roles(cands: &mut [Cand]) {
    let mut seen: Vec<Role> = Vec::new();
    for c in cands.iter_mut() {
        if let Some(r) = c.role {
            if seen.contains(&r) { c.role = None; } else { seen.push(r); }
        }
    }
}
fn do_add(cands: &mut Vec<Cand>, scr: &mut Screen, add_ip: &mut String, add_role: &mut Option<Role>, creds: &[Cred]) {
    if add_ip.is_empty() {
        return;
    }
    let r = match add_role.or_else(|| first_free_role(cands)) {
        Some(r) => r,
        None => return,
    };
    scr.clear();
    let _ = disable_raw_mode();
    println!("[inject] testando {} ...", add_ip);
    let (host, intern, suggest, working) = probe(add_ip, creds);
    let mut c = Cand { ip: add_ip.clone(), host, intern, suggest, working, role: None };
    if c.accessible() {
        c.role = Some(r);
    } else {
        println!("[inject] {}: sem acesso", add_ip);
    }
    cands.push(c);
    sort_cands(cands);
    let _ = enable_raw_mode();
    let _ = execute!(stdout(), Hide);
    *scr = Screen::new();
    add_ip.clear();
    *add_role = None;
}

fn gather(cands: &[Cand]) -> Vec<(Role, String, Cred)> {
    let mut out = Vec::new();
    for c in cands {
        if let Some(r) = c.role {
            if let Some(cr) = c.cred_for(r) {
                out.push((r, c.ip.clone(), cr.clone()));
            }
        }
    }
    out
}

fn select_body(cands: &[Cand], focus: &Sel, li: usize, top: usize, show_add: bool, add_ip: &str, add_role: Option<Role>) -> Vec<String> {
    let list_focus = *focus == Sel::List;
    let mut body = Vec::new();
    // cabecalho da tabela
    let head = format!("{}{}{}{}", pad("IP", 16), pad("hostname", 14), pad("interna", 16), "função");
    body.push(fg(FG_DIM, &pad(&head, PW)));
    // linhas
    for vi in 0..LROWS {
        let idx = top + vi;
        if idx >= cands.len() {
            body.push(String::new());
            continue;
        }
        let c = &cands[idx];
        let cursor = idx == li;
        let mk = if cursor { fg(if list_focus { FG_CYAN } else { FG_CYAN_DIM }, "\u{25b6} ") } else { "  ".to_string() };
        let txt = if c.accessible() {
            let f = c.role.map(|r| format!("[{}]", r.name())).unwrap_or_else(|| c.suggest.map(|r| format!("(sug {})", r.name())).unwrap_or_default());
            format!("{}{}{}{}", pad(&c.ip, 14), pad(&c.host, 14), pad(&c.intern, 16), f)
        } else {
            // erro ocupa o espaço das colunas (menos IP)
            format!("{}{}", pad(&c.ip, 14), "login falhou (sem acesso)")
        };
        let code = if cursor && list_focus { FG_BRIGHT } else if list_focus { FG_NORM } else { FG_DIM };
        body.push(format!("{}{}", mk, fg(code, &pad(&txt, PW - 2))));
    }
    // add (3 linhas, caixas ascii)
    if show_add {
        let ipbox = boxed_input("IP", add_ip, *focus == Sel::AddIp, 15);
        let rb = if !add_ip.is_empty() {
            let rt = add_role.or_else(|| first_free_role(cands)).map(|r| r.name()).unwrap_or("?");
            boxed_input("função", rt, *focus == Sel::AddRole, 9)
        } else {
            [String::new(), String::new(), String::new()]
        };
        body.push(format!("{}  {}", ipbox[0], rb[0]));
        body.push(format!("{}  {}", ipbox[1], rb[1]));
        body.push(format!("{}  {}", ipbox[2], rb[2]));
    } else {
        body.push(String::new());
        body.push(String::new());
        body.push(String::new());
    }
    // botoes
    let b = buttons_rows(*focus == Sel::Enter, *focus == Sel::Esc);
    body.push(b[0].clone());
    body.push(b[1].clone());
    body.push(b[2].clone());
    // hint
    let hint = match focus {
        Sel::List => "setas movem  -  Enter define a função  -  Tab sai da lista",
        Sel::AddIp => "digite um IP que o scan perdeu  -  Tab vai pra função",
        Sel::AddRole => "setas escolhem a função  -  Enter adiciona",
        _ => "Enter confirma tudo  -  Esc cancela",
    };
    body.push(fg(FG_DIM, hint));
    body
}

fn pick_role_inline(scr: &mut Screen, cands: &[Cand], li: usize) -> Option<Role> {
    // funções livres + a atual da linha
    let mut opts: Vec<Option<Role>> = Vec::new();
    for r in [Role::Mgm, Role::N1, Role::N2] {
        let taken_elsewhere = cands.iter().enumerate().any(|(i, c)| i != li && c.role == Some(r));
        if !taken_elsewhere {
            opts.push(Some(r));
        }
    }
    opts.push(None); // remover
    let names: Vec<String> = opts.iter().map(|o| o.map(|r| r.name().to_string()).unwrap_or_else(|| "(remover)".to_string())).collect();
    let mut sel = cands[li].role.and_then(|r| opts.iter().position(|o| *o == Some(r))).unwrap_or(0);
    loop {
        let mut body = vec![fg(FG_BRIGHT, "Função desta VM:"), String::new()];
        for (i, n) in names.iter().enumerate() {
            let m = if i == sel { fg(FG_CYAN, "\u{25b6} ") } else { "  ".to_string() };
            body.push(format!("{}{}", m, fg(if i == sel { FG_BRIGHT } else { FG_NORM }, n)));
        }
        scr.render(&frame("Selecione as VMs", body));
        match key_press() {
            Some(k) => match k.code {
                KeyCode::Up => { if sel > 0 { sel -= 1; } }
                KeyCode::Down => { if sel + 1 < opts.len() { sel += 1; } }
                KeyCode::Enter => return opts[sel],
                KeyCode::Esc => return cands[li].role,
                _ => {}
            },
            None => return cands[li].role,
        }
    }
}

// --------------------------------------------------------------- preview

fn preview() {
    let cols = vec![
        Cred { label: ColLabel::Mgm, user: "paulo".into(), pass: "secret".into() },
        Cred { label: ColLabel::Any, user: String::new(), pass: String::new() },
    ];
    for l in frame("Credenciais", render_creds_body(&cols, 1, 0)) {
        println!("{}", l);
    }
    println!();
    let cands = vec![
        Cand { ip: "192.168.0.21".into(), host: "N2".into(), intern: "192.168.1.3".into(), suggest: Some(Role::N2), working: vec![cols[0].clone()], role: Some(Role::N2) },
        Cand { ip: "192.168.0.20".into(), host: String::new(), intern: String::new(), suggest: None, working: vec![], role: None },
    ];
    for l in frame("Selecione as VMs", select_body(&cands, &Sel::List, 0, 0, true, "", None)) {
        println!("{}", l);
    }
}
