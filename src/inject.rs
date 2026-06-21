//! `bdd inject` (roda no HOST): acha as VMs na rede, você marca quem é
//! MGM/N1/N2 numa TUI, e instala o bdd em cada uma por SSH. Não precisa
//! digitar nada dentro da VM.
//!
//! A TUI é renderizada "em linha" (sem limpar o terminal): aparece no fim, como
//! qualquer comando, e é redesenhada no lugar.

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
];

// estilos (usa "soft reset" 22;39 para não apagar o fundo no meio da linha)
const BG: &str = "\x1b[48;5;236m";
const RESET: &str = "\x1b[0m";
const SR: &str = "\x1b[22;39m";
const FG_BRIGHT: &str = "\x1b[1m\x1b[97m";
const FG_NORM: &str = "\x1b[37m";
const FG_DIM: &str = "\x1b[90m";
const FG_CYAN: &str = "\x1b[1m\x1b[96m";

const W: usize = 12; // largura interna da caixa
const SEG: usize = W + 4; // marcador(2) + borda(2) + interno(W)

struct Cand {
    ip: String,
    info: String,
    suggest: Option<Role>,
    creds: Option<(String, String)>,
    role: Option<Role>,
}

pub fn run() {
    // Pré-visualização estática da TUI de credenciais (para inspeção visual).
    if std::env::var("BDD_INJECT_PREVIEW").is_ok() {
        let cols = vec![
            Col { label: ColLabel::Mgm, user: "paulo".into(), pass: "secret".into() },
            Col { label: ColLabel::Any, user: String::new(), pass: String::new() },
        ];
        for l in render_creds(&cols, true, 1, 0) {
            println!("{}", l);
        }
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
        Err(_) => {
            eprintln!("[inject] não achei o próprio binário para enviar.");
            return;
        }
    };

    let host_ips = host_ips();
    let base = match host_ips.iter().find_map(|ip| subnet_base(ip)) {
        Some(b) => b,
        None => {
            eprintln!("[inject] não achei a rede do host.");
            return;
        }
    };

    let creds = match tui_creds() {
        Some(c) if !c.is_empty() => c,
        _ => {
            println!("[inject] cancelado.");
            return;
        }
    };

    println!("[inject] procurando VMs (SSH) em {}.0/24 ...", base);
    let exclude: HashSet<String> = host_ips.into_iter().collect();
    let ips = scan(&base, &exclude);

    println!("[inject] identificando {} host(s)...", ips.len());
    let mut cands: Vec<Cand> = Vec::new();
    for ip in ips {
        let (info, suggest, creds_ok) = probe(&ip, &creds);
        cands.push(Cand { ip, info, suggest, creds: creds_ok, role: None });
    }

    let chosen = match tui_select(&mut cands, &creds) {
        Some(v) if !v.is_empty() => v,
        Some(_) => { println!("[inject] nada selecionado."); return; }
        None => { println!("[inject] cancelado."); return; }
    };

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
    all.extend(scan_pass(&missed));
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

// --------------------------------------------------------------- tela inline

/// Renderiza um bloco de linhas no lugar (sem limpar o terminal todo).
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

fn key_press() -> Option<crossterm::event::KeyEvent> {
    loop {
        match read() {
            Ok(Event::Key(k)) if k.kind == KeyEventKind::Press => return Some(k),
            Ok(Event::Key(_)) => continue,
            Ok(_) => continue,
            Err(_) => return None,
        }
    }
}

// --------------------------------------------------------------- TUI: creds

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
    fn next(self) -> ColLabel {
        match self {
            ColLabel::Any => ColLabel::Mgm,
            ColLabel::Mgm => ColLabel::N1,
            ColLabel::N1 => ColLabel::N2,
            ColLabel::N2 => ColLabel::Any,
        }
    }
    fn prev(self) -> ColLabel {
        match self {
            ColLabel::Any => ColLabel::N2,
            ColLabel::Mgm => ColLabel::Any,
            ColLabel::N1 => ColLabel::Mgm,
            ColLabel::N2 => ColLabel::N1,
        }
    }
}

struct Col {
    label: ColLabel,
    user: String,
    pass: String,
}
impl Col {
    fn filled(&self) -> bool {
        !self.user.is_empty() || !self.pass.is_empty()
    }
}

// foco: (row, col). row 0=label,1=user,2=pass,3=botoes. col em botoes: 0=Enter,1=Esc
fn tui_creds() -> Option<Vec<(String, String)>> {
    let mut cols: Vec<Col> = vec![Col { label: ColLabel::Any, user: String::new(), pass: String::new() }];
    let (mut row, mut col) = (1usize, 0usize); // começa no user da primeira coluna
    let mut scr = Screen::new();
    if enable_raw_mode().is_err() {
        return None;
    }
    let _ = execute!(stdout(), Hide);
    let result;
    loop {
        // promoção: enquanto a última coluna estiver preenchida e houver espaço, abre a próxima
        while cols.len() < 3 && cols.last().map(|c| c.filled()).unwrap_or(false) {
            cols.push(Col { label: ColLabel::Any, user: String::new(), pass: String::new() });
        }
        let ncols = cols.len();
        let placeholder = ncols < 3;
        clamp_focus(&mut row, &mut col, ncols);

        scr.render(&render_creds(&cols, placeholder, row, col));

        let k = match key_press() {
            Some(k) => k,
            None => { result = None; break; }
        };
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let shift = k.modifiers.contains(KeyModifiers::SHIFT);
        match k.code {
            KeyCode::Esc => { result = None; break; }
            KeyCode::Tab => tab_next(&mut row, &mut col, ncols),
            KeyCode::BackTab => {
                if row == 0 {
                    cols[col].label = cols[col].label.prev(); // back-cycle no rótulo
                } else {
                    tab_prev(&mut row, &mut col, ncols);
                }
            }
            KeyCode::Up => move_2d(&mut row, &mut col, -1, 0, ncols),
            KeyCode::Down => move_2d(&mut row, &mut col, 1, 0, ncols),
            KeyCode::Left => move_2d(&mut row, &mut col, 0, -1, ncols),
            KeyCode::Right => move_2d(&mut row, &mut col, 0, 1, ncols),
            KeyCode::Char('-') if row == 0 => cols[col].label = cols[col].label.prev(),
            KeyCode::Char('u') | KeyCode::Char('U') if ctrl => {
                if row == 1 { cols[col].user.clear(); }
                if row == 2 { cols[col].pass.clear(); }
            }
            KeyCode::Enter => {
                if row == 0 {
                    cols[col].label = if shift { cols[col].label.prev() } else { cols[col].label.next() };
                } else if row == 3 {
                    if col == 0 { result = Some(collect(&cols)); break; }
                    else { result = None; break; }
                } else {
                    result = Some(collect(&cols)); // Enter em campo = confirmar
                    break;
                }
            }
            KeyCode::Char(' ') if row == 0 => cols[col].label = cols[col].label.next(),
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

fn collect(cols: &[Col]) -> Vec<(String, String)> {
    cols.iter().filter(|c| !c.user.is_empty()).map(|c| (c.user.clone(), c.pass.clone())).collect()
}

fn clamp_focus(row: &mut usize, col: &mut usize, ncols: usize) {
    if *row > 3 { *row = 3; }
    let maxcol = if *row == 3 { 1 } else { ncols.saturating_sub(1) };
    if *col > maxcol { *col = maxcol; }
}

fn tab_next(row: &mut usize, col: &mut usize, ncols: usize) {
    // ordem: (0,0)(1,0)(2,0)(0,1)... depois (3,0)(3,1)
    if *row == 3 {
        if *col == 0 { *col = 1; } else { *row = 0; *col = 0; }
        return;
    }
    if *row < 2 {
        *row += 1;
    } else {
        *row = 0;
        if *col + 1 < ncols { *col += 1; } else { *row = 3; *col = 0; }
    }
}

fn tab_prev(row: &mut usize, col: &mut usize, ncols: usize) {
    if *row == 3 {
        if *col == 1 { *col = 0; } else { *row = 2; *col = ncols.saturating_sub(1); }
        return;
    }
    if *row > 0 {
        *row -= 1;
    } else {
        *row = 2;
        if *col > 0 { *col -= 1; } else { *row = 3; *col = 1; }
    }
}

fn move_2d(row: &mut usize, col: &mut usize, dr: i32, dc: i32, ncols: usize) {
    let nr = (*row as i32 + dr).clamp(0, 3) as usize;
    *row = nr;
    let maxcol = if *row == 3 { 1 } else { ncols.saturating_sub(1) };
    let nc = (*col as i32 + dc).clamp(0, maxcol as i32) as usize;
    *col = nc.min(maxcol);
}

fn pad(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w {
        s.chars().take(w).collect()
    } else {
        format!("{}{}", s, " ".repeat(w - n))
    }
}
fn center(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w {
        return s.chars().take(w).collect();
    }
    let left = (w - n) / 2;
    format!("{}{}{}", " ".repeat(left), s, " ".repeat(w - n - left))
}
fn fg(code: &str, s: &str) -> String {
    format!("{}{}{}", code, s, SR)
}

/// Monta as linhas da TUI de credenciais (já estilizadas, com fundo).
fn render_creds(cols: &[Col], placeholder: bool, frow: usize, fcol: usize) -> Vec<String> {
    let gut = 7usize; // gutter dos rótulos de linha (user/senha)
    let total_cols = cols.len() + if placeholder { 1 } else { 0 };

    // segmentos por coluna para cada um dos 7 tipos de linha
    // tipos: 0 label, 1 utop, 2 umid, 3 ubot, 4 ptop, 5 pmid, 6 pbot
    let mut body: Vec<String> = Vec::new();
    let row_label = |kind: usize| -> &str {
        match kind {
            2 => "user",
            5 => "senha",
            _ => "",
        }
    };
    for kind in 0..7 {
        let mut line = String::new();
        // gutter
        line.push_str(&fg(FG_DIM, &pad(&format!("{:>5} ", row_label(kind)), gut)));
        for ci in 0..total_cols {
            let is_ph = ci >= cols.len();
            let seg = if is_ph {
                placeholder_seg(kind)
            } else {
                col_seg(&cols[ci], kind, ci, frow, fcol)
            };
            line.push_str(&seg);
            line.push(' ');
        }
        body.push(line);
    }

    // dica (espaço reservado mesmo quando vazia)
    let hint = if frow == 1 || frow == 2 {
        fg(FG_DIM, "Ctrl+U limpa o campo  -  Tab move  -  setas movem")
    } else if frow == 0 {
        fg(FG_DIM, "Enter cicla o rotulo (Any/MGM/N1/N2)  -  Tab move")
    } else {
        String::new()
    };

    // largura do painel
    let content_w = gut + total_cols * (SEG + 1);
    let panelw = content_w.max(50);

    // botoes em caixa, centralizados
    let enter_focus = frow == 3 && fcol == 0;
    let esc_focus = frow == 3 && fcol == 1;
    let eb = boxed_button("Enter", enter_focus);
    let sb = boxed_button("Esc", esc_focus);
    let gap = 3usize;
    let combined = button_box_w("Enter") + gap + button_box_w("Esc");
    let leftpad = panelw.saturating_sub(combined) / 2;
    let lp = " ".repeat(leftpad);
    let g = " ".repeat(gap);
    let btn_top = format!("{}{}{}{}", lp, eb[0], g, sb[0]);
    let btn_mid = format!("{}{}{}{}", lp, eb[1], g, sb[1]);
    let btn_bot = format!("{}{}{}{}", lp, eb[2], g, sb[2]);

    // monta painel com fundo dim
    let mut out: Vec<String> = Vec::new();
    out.push(panel_top("Credenciais", panelw));
    out.push(panel_line("", panelw));
    for l in &body {
        out.push(panel_line(l, panelw));
    }
    out.push(panel_line("", panelw));
    out.push(panel_line(&btn_top, panelw));
    out.push(panel_line(&btn_mid, panelw));
    out.push(panel_line(&btn_bot, panelw));
    out.push(panel_line(&hint, panelw));
    out.push(panel_bottom(panelw));
    out
}

fn button_box_w(txt: &str) -> usize {
    txt.chars().count() + 4 // " txt " + 2 bordas
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

fn col_seg(c: &Col, kind: usize, ci: usize, frow: usize, fcol: usize) -> String {
    let col_focused = fcol == ci && frow <= 2;
    let label_focused = frow == 0 && fcol == ci;
    let user_focused = frow == 1 && fcol == ci;
    let pass_focused = frow == 2 && fcol == ci;
    let unfilled = !c.filled();

    let base = if unfilled { FG_DIM } else { FG_NORM };

    match kind {
        0 => {
            let mut lbl = c.label.text().to_string();
            if label_focused {
                lbl.push_str(" \u{21bb}"); // ↻
            }
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
            format!("{}{}{}{}{}", marker, fg(bcode, "\u{2502}"), fg(tcode, &pad(&content, W)), fg(bcode, "\u{2502}"), "")
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

// painel ------------------------------------------------------------------

fn visible_len(s: &str) -> usize {
    // conta caracteres ignorando sequências ANSI
    let mut n = 0;
    let mut in_esc = false;
    for ch in s.chars() {
        if in_esc {
            if ch == 'm' { in_esc = false; }
            continue;
        }
        if ch == '\x1b' { in_esc = true; continue; }
        n += 1;
    }
    n
}

fn panel_top(title: &str, w: usize) -> String {
    // largura total visível das linhas internas = w + 4 ("│ " + w + " │")
    let head = format!("\u{256d}\u{2500} {} ", title); // ╭─ titulo (espaço)
    let dashes = (w + 4).saturating_sub(visible_len(&head) + 1); // -1 do ╮
    format!("{}{}{}{}\u{256e}{}", BG, FG_DIM, head, "\u{2500}".repeat(dashes), RESET)
}
fn panel_bottom(w: usize) -> String {
    format!("{}{}\u{2570}{}\u{256f}{}", BG, FG_DIM, "\u{2500}".repeat(w + 2), RESET)
}
fn panel_line(content: &str, w: usize) -> String {
    let vis = visible_len(content);
    let padn = w.saturating_sub(vis);
    format!(
        "{}{}\u{2502} {}{}{} \u{2502}{}",
        BG, FG_DIM, content, SR, " ".repeat(padn), RESET
    )
}

// --------------------------------------------------------------- TUI: select

fn tui_select(cands: &mut [Cand], creds: &[(String, String)]) -> Option<Vec<(Role, String, (String, String))>> {
    for r in [Role::Mgm, Role::N1, Role::N2] {
        let hits: Vec<usize> = cands.iter().enumerate().filter(|(_, c)| c.suggest == Some(r) && c.creds.is_some()).map(|(i, _)| i).collect();
        if hits.len() == 1 {
            cands[hits[0]].role = Some(r);
        }
    }
    let mut cursor = 0usize;
    let mut manual: Vec<Cand> = Vec::new();
    let mut scr = Screen::new();
    if enable_raw_mode().is_err() {
        return None;
    }
    let _ = execute!(stdout(), Hide);
    let result;
    loop {
        let total = cands.len() + manual.len();
        let mut lines = vec![
            fg(FG_BRIGHT, "Selecione as VMs"),
            fg(FG_DIM, "setas movem - Enter define papel - 'a' adiciona IP - F2 confirma - Esc cancela"),
            String::new(),
        ];
        let all: Vec<&Cand> = cands.iter().chain(manual.iter()).collect();
        if all.is_empty() {
            lines.push(fg(FG_DIM, "  (nenhuma VM; use 'a' para adicionar um IP)"));
        }
        for (i, c) in all.iter().enumerate() {
            let marker = if i == cursor { fg(FG_CYAN, "\u{25b6} ") } else { "  ".to_string() };
            let role = c.role.map(|r| fg(FG_BRIGHT, &format!("[{}]", r.name())))
                .unwrap_or_else(|| c.suggest.map(|r| fg(FG_DIM, &format!("(sug: {})", r.name()))).unwrap_or_default());
            let acc = if c.creds.is_some() { String::new() } else { fg(FG_DIM, "  (sem acesso)") };
            lines.push(format!("{}{} {} {}{}", marker, fg(FG_NORM, &pad(&c.ip, 15)), fg(FG_DIM, &pad(&c.info, 34)), role, acc));
        }
        scr.render(&lines);

        let k = match key_press() { Some(k) => k, None => { result = None; break; } };
        match k.code {
            KeyCode::Esc => { result = None; break; }
            KeyCode::F(2) => { result = Some(gather(cands, &manual)); break; }
            KeyCode::Up => { if cursor > 0 { cursor -= 1; } }
            KeyCode::Down => { if total > 0 && cursor + 1 < total { cursor += 1; } }
            KeyCode::Char('a') => {
                scr.clear();
                let _ = disable_raw_mode();
                if let Some(c) = manual_add(creds) { manual.push(c); }
                let _ = enable_raw_mode();
                scr = Screen::new();
            }
            KeyCode::Enter => {
                if total == 0 { continue; }
                scr.clear();
                let chosen = pick_role();
                scr = Screen::new();
                let n = cands.len();
                if let Some(role) = chosen {
                    for c in cands.iter_mut() { if c.role == Some(role) { c.role = None; } }
                    for c in manual.iter_mut() { if c.role == Some(role) { c.role = None; } }
                    if cursor < n { cands[cursor].role = Some(role); } else { manual[cursor - n].role = Some(role); }
                } else {
                    if cursor < n { cands[cursor].role = None; } else { manual[cursor - n].role = None; }
                }
            }
            _ => {}
        }
    }
    scr.clear();
    let _ = execute!(stdout(), Show);
    let _ = disable_raw_mode();
    result
}

fn gather(cands: &[Cand], manual: &[Cand]) -> Vec<(Role, String, (String, String))> {
    let mut out = Vec::new();
    for c in cands.iter().chain(manual.iter()) {
        if let (Some(r), Some(cr)) = (c.role, c.creds.clone()) {
            out.push((r, c.ip.clone(), cr));
        }
    }
    out
}

fn pick_role() -> Option<Role> {
    let opts = [Some(Role::Mgm), Some(Role::N1), Some(Role::N2), None];
    let names = ["MGM", "N1", "N2", "(remover papel)"];
    let mut sel = 0usize;
    let mut scr = Screen::new();
    if enable_raw_mode().is_err() {
        return None;
    }
    let _ = execute!(stdout(), Hide);
    let res;
    loop {
        let mut lines = vec![fg(FG_BRIGHT, "Papel desta VM:"), String::new()];
        for (i, n) in names.iter().enumerate() {
            let m = if i == sel { fg(FG_CYAN, "\u{25b6} ") } else { "  ".to_string() };
            lines.push(format!("{}{}", m, fg(if i == sel { FG_BRIGHT } else { FG_NORM }, n)));
        }
        lines.push(String::new());
        lines.push(fg(FG_DIM, "setas + Enter; Esc cancela"));
        scr.render(&lines);
        match key_press() {
            Some(k) => match k.code {
                KeyCode::Up => { if sel > 0 { sel -= 1; } }
                KeyCode::Down => { if sel + 1 < opts.len() { sel += 1; } }
                KeyCode::Enter => { res = opts[sel]; break; }
                KeyCode::Esc => { res = None; break; }
                _ => {}
            },
            None => { res = None; break; }
        }
    }
    scr.clear();
    let _ = execute!(stdout(), Show);
    let _ = disable_raw_mode();
    res
}

fn manual_add(creds: &[(String, String)]) -> Option<Cand> {
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
