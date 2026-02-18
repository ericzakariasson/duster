//! Interactive TUI to visualize disk usage (ncdu-style, simplified).

use crate::cli::TuiOptions;
use crate::{disk, disk_usage, ui};
use anyhow::Result;
use colored::Colorize;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use crossterm::{
    cursor::Show,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Row, Table, Wrap},
    Terminal,
};

pub fn run(options: &TuiOptions) -> Result<()> {
    let start_path = disk::resolve_target_path(options.path.as_ref())?;

    // Cursor's execution environment (and CI) typically does not provide a TTY. In that case,
    // fall back to a one-shot report that can be captured as stdout.
    let no_tty = !io::stdin().is_terminal() || !io::stdout().is_terminal();
    if options.plain || no_tty {
        return run_plain(&start_path, options);
    }

    run_interactive(&start_path, options)
}

fn run_plain(start_path: &Path, options: &TuiOptions) -> Result<()> {
    let space = disk::fs_space_for_path(start_path)?;
    let usage = disk_usage::scan_directory_children(start_path, options.hidden)?;

    ui::print_header("Disk usage (plain report)");
    println!(
        "{} {}",
        "Path:".dimmed(),
        ui::format_path(start_path).bold()
    );
    println!(
        "{} {}",
        "Mount:".dimmed(),
        space.mount_point.display().to_string().bold()
    );
    println!(
        "{}  {}  {}  {}",
        format!("Total: {}", ui::format_size(space.total_bytes)).yellow(),
        format!("Used: {}", ui::format_size(disk::used_bytes(&space))).red(),
        format!("Free: {}", ui::format_size(space.free_bytes)).green(),
        format!("Used%: {}%", disk::used_percent(&space)).dimmed()
    );
    println!();

    let limit = options.limit.max(1);
    let shown = usage.children.iter().take(limit).collect::<Vec<_>>();
    let max_size = shown.iter().map(|c| c.size_bytes).max().unwrap_or(0);

    println!(
        "{:<40} {:>12} {:>6}  {}",
        "Name".bold(),
        "Size".bold(),
        "%".bold(),
        "Bar".bold()
    );
    ui::print_table_separator(76);

    for child in shown {
        let pct = percent(child.size_bytes, usage.total_bytes);
        let bar = bar(child.size_bytes, max_size, 18);
        let mut name = child.name.clone();
        if child.kind == disk_usage::EntryKind::Directory {
            name.push('/');
        }
        println!("{:<40} {:>12} {:>5}%  {}", name, ui::format_size(child.size_bytes), pct, bar);
    }

    if usage.children.len() > limit {
        println!(
            "\n{} {} more entries (use `--limit` to change).",
            "…".dimmed(),
            usage.children.len() - limit
        );
    }
    if usage.error_count > 0 {
        println!(
            "\n{} {} filesystem error(s) were ignored while scanning.",
            "⚠".yellow().bold(),
            usage.error_count
        );
    }

    Ok(())
}

fn run_interactive(start_path: &Path, options: &TuiOptions) -> Result<()> {
    let _guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    run_app(&mut terminal, start_path.to_path_buf(), options.hidden)
}

struct TerminalGuard;
impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        Ok(Self)
    }
}
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, Show);
    }
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    start_path: PathBuf,
    show_hidden: bool,
) -> Result<()> {
    let mut app = App::new(start_path, show_hidden);
    app.start_scan();

    let tick_rate = Duration::from_millis(200);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| draw(f, &mut app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    handle_key(&mut app, key);
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = Instant::now();
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

struct App {
    current_path: PathBuf,
    show_hidden: bool,
    should_quit: bool,

    // Scanning & results
    space: Option<disk::FsSpace>,
    usage: Option<disk_usage::DirectoryUsage>,
    scan: Option<ScanInFlight>,
    scan_error: Option<String>,
    error_count: usize,

    // List state
    selected: usize,
    scroll: usize,
    last_table_height: u16,

    // UI chrome
    spinner_idx: usize,
    last_space_refresh: Instant,
}

struct ScanInFlight {
    started_at: Instant,
    rx: Receiver<anyhow::Result<disk_usage::DirectoryUsage>>,
}

impl App {
    fn new(start_path: PathBuf, show_hidden: bool) -> Self {
        Self {
            current_path: start_path,
            show_hidden,
            should_quit: false,
            space: None,
            usage: None,
            scan: None,
            scan_error: None,
            error_count: 0,
            selected: 0,
            scroll: 0,
            last_table_height: 0,
            spinner_idx: 0,
            last_space_refresh: Instant::now(),
        }
    }

    fn start_scan(&mut self) {
        self.scan_error = None;

        self.space = disk::fs_space_for_path(&self.current_path).ok();
        self.last_space_refresh = Instant::now();

        let path = self.current_path.clone();
        let show_hidden = self.show_hidden;
        let (tx, rx) = mpsc::channel();
        let started_at = Instant::now();
        std::thread::spawn(move || {
            let result = disk_usage::scan_directory_children(&path, show_hidden);
            let _ = tx.send(result);
        });

        self.scan = Some(ScanInFlight {
            started_at,
            rx,
        });
    }

    fn on_tick(&mut self) {
        self.spinner_idx = (self.spinner_idx + 1) % SPINNER.len();

        // Refresh free space occasionally (cheap; no directory scanning).
        if self.last_space_refresh.elapsed() >= Duration::from_secs(2) {
            self.space = disk::fs_space_for_path(&self.current_path).ok();
            self.last_space_refresh = Instant::now();
        }

        if let Some(scan) = self.scan.take() {
            match scan.rx.try_recv() {
                Ok(Ok(usage)) => {
                    if usage.path == self.current_path {
                        self.error_count = usage.error_count;
                        self.usage = Some(usage);
                        self.selected = 0;
                        self.scroll = 0;
                    }
                }
                Ok(Err(e)) => {
                    self.usage = None;
                    self.scan_error = Some(e.to_string());
                }
                Err(mpsc::TryRecvError::Empty) => {
                    self.scan = Some(scan);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.scan_error = Some("scan thread disconnected".to_string());
                }
            }
        }
    }

    fn entries(&self) -> &[disk_usage::ChildUsage] {
        match self.usage.as_ref() {
            Some(u) => &u.children,
            None => &[],
        }
    }

    fn total_bytes(&self) -> u64 {
        self.usage.as_ref().map(|u| u.total_bytes).unwrap_or(0)
    }

    fn selected_entry(&self) -> Option<&disk_usage::ChildUsage> {
        self.entries().get(self.selected)
    }

    fn select_prev(&mut self) {
        let len = self.entries().len();
        if len == 0 {
            self.selected = 0;
            self.scroll = 0;
            return;
        }
        self.selected = self.selected.saturating_sub(1);
        self.ensure_visible(len);
    }

    fn select_next(&mut self) {
        let len = self.entries().len();
        if len == 0 {
            self.selected = 0;
            self.scroll = 0;
            return;
        }
        self.selected = (self.selected + 1).min(len - 1);
        self.ensure_visible(len);
    }

    fn page_up(&mut self) {
        let len = self.entries().len();
        if len == 0 {
            return;
        }
        let step = self.visible_rows().max(1);
        self.selected = self.selected.saturating_sub(step);
        self.ensure_visible(len);
    }

    fn page_down(&mut self) {
        let len = self.entries().len();
        if len == 0 {
            return;
        }
        let step = self.visible_rows().max(1);
        self.selected = (self.selected + step).min(len - 1);
        self.ensure_visible(len);
    }

    fn visible_rows(&self) -> usize {
        // Border + header + border ~= 3 rows. This is approximate but good enough.
        (self.last_table_height.saturating_sub(3)) as usize
    }

    fn ensure_visible(&mut self, len: usize) {
        let vis = self.visible_rows().max(1);
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + vis {
            self.scroll = self.selected + 1 - vis;
        }

        if len <= vis {
            self.scroll = 0;
        } else if self.scroll + vis > len {
            self.scroll = len - vis;
        }
    }

    fn go_into_selected(&mut self) {
        let Some(entry) = self.selected_entry() else {
            return;
        };
        if entry.kind != disk_usage::EntryKind::Directory {
            return;
        }
        self.current_path = entry.path.clone();
        self.selected = 0;
        self.scroll = 0;
        self.start_scan();
    }

    fn go_up(&mut self) {
        let Some(parent) = self.current_path.parent() else {
            return;
        };
        self.current_path = parent.to_path_buf();
        self.selected = 0;
        self.scroll = 0;
        self.start_scan();
    }

    fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.selected = 0;
        self.scroll = 0;
        self.start_scan();
    }
}

const SPINNER: &[char] = &['|', '/', '-', '\\'];

fn handle_key(app: &mut App, key: KeyEvent) {
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), _) | (KeyCode::Esc, _) => app.should_quit = true,
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => app.should_quit = true,

        (KeyCode::Up, _) | (KeyCode::Char('k'), _) => app.select_prev(),
        (KeyCode::Down, _) | (KeyCode::Char('j'), _) => app.select_next(),
        (KeyCode::PageUp, _) => app.page_up(),
        (KeyCode::PageDown, _) => app.page_down(),

        (KeyCode::Enter, _) => app.go_into_selected(),
        (KeyCode::Backspace, _) => app.go_up(),

        (KeyCode::Char('r'), _) => app.start_scan(),
        (KeyCode::Char('h'), _) => app.toggle_hidden(),
        _ => {}
    }
}

fn draw(f: &mut ratatui::Frame, app: &mut App) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(area);

    draw_header(f, chunks[0], app);
    draw_table(f, chunks[1], app);
    draw_footer(f, chunks[2], app);
}

fn draw_header(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let title = if let Some(scan) = &app.scan {
        let secs = scan.started_at.elapsed().as_secs_f32();
        format!(
            "Duster disk usage  {} Scanning… ({secs:.1}s)",
            SPINNER[app.spinner_idx]
        )
    } else {
        "Duster disk usage".to_string()
    };

    let path_line = Line::from(vec![
        Span::styled("Path: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            ui::format_path(&app.current_path),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
    ]);

    let mut lines = vec![Line::from(Span::styled(
        title,
        Style::default().add_modifier(Modifier::BOLD),
    ))];
    lines.push(path_line);

    if let Some(space) = &app.space {
        let used = disk::used_bytes(space);
        let pct = disk::used_percent(space);
        lines.push(Line::from(vec![
            Span::styled("Mount: ", Style::default().fg(Color::DarkGray)),
            Span::raw(space.mount_point.display().to_string()),
            Span::raw("   "),
            Span::styled("Total ", Style::default().fg(Color::DarkGray)),
            Span::styled(ui::format_size(space.total_bytes), Style::default().fg(Color::Yellow)),
            Span::raw("   "),
            Span::styled("Used ", Style::default().fg(Color::DarkGray)),
            Span::styled(ui::format_size(used), Style::default().fg(Color::Red)),
            Span::raw("   "),
            Span::styled("Free ", Style::default().fg(Color::DarkGray)),
            Span::styled(ui::format_size(space.free_bytes), Style::default().fg(Color::Green)),
            Span::raw("   "),
            Span::styled(format!("{}%", pct), Style::default().fg(Color::DarkGray)),
        ]));
    } else {
        lines.push(Line::from(Span::styled(
            "Filesystem info unavailable",
            Style::default().fg(Color::Yellow),
        )));
    }

    let header_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(3)])
        .split(area);

    let header = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(header, header_chunks[0]);

    let gauge = if let Some(space) = &app.space {
        let pct = disk::used_percent(space);
        Gauge::default()
            .block(Block::default().title("Disk used").borders(Borders::ALL))
            .gauge_style(Style::default().fg(Color::Red))
            .percent(pct)
            .label(format!(
                "{} used / {} total",
                ui::format_size(disk::used_bytes(space)),
                ui::format_size(space.total_bytes)
            ))
    } else {
        Gauge::default()
            .block(Block::default().title("Disk used").borders(Borders::ALL))
            .gauge_style(Style::default().fg(Color::DarkGray))
            .percent(0)
            .label("unknown".to_string())
    };
    f.render_widget(gauge, header_chunks[1]);
}

fn draw_table(f: &mut ratatui::Frame, area: Rect, app: &mut App) {
    app.last_table_height = area.height;

    let entries = app.entries();
    let total = app.total_bytes();
    let max_size = entries.iter().map(|e| e.size_bytes).max().unwrap_or(0);

    let vis = app.visible_rows().max(1);
    let start = app.scroll.min(entries.len());
    let end = (start + vis).min(entries.len());
    let slice = &entries[start..end];

    let rows = slice.iter().enumerate().map(|(i, entry)| {
        let idx = start + i;
        let mut name = entry.name.clone();
        if entry.kind == disk_usage::EntryKind::Directory {
            name.push('/');
        }

        let pct = percent(entry.size_bytes, total);
        let bar_str = bar(entry.size_bytes, max_size, 12);

        let mut row = Row::new(vec![
            name,
            ui::format_size(entry.size_bytes),
            format!("{pct}%"),
            bar_str,
        ]);

        if idx == app.selected {
            row = row.style(Style::default().add_modifier(Modifier::REVERSED));
        }
        row
    });

    let title = if entries.is_empty() {
        "Entries".to_string()
    } else {
        format!(
            "Entries ({}/{})",
            (app.selected + 1).min(entries.len()),
            entries.len()
        )
    };

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(55),
            Constraint::Length(12),
            Constraint::Length(6),
            Constraint::Length(12),
        ],
    )
    .header(Row::new(vec![
        ratatui::widgets::Cell::from(Span::styled(
            "Name",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        ratatui::widgets::Cell::from(Span::styled(
            "Size",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        ratatui::widgets::Cell::from(Span::styled(
            "%",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        ratatui::widgets::Cell::from(Span::styled(
            "Bar",
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ]))
    .block(Block::default().borders(Borders::ALL).title(title));

    f.render_widget(table, area);

    if let Some(err) = &app.scan_error {
        let msg = Paragraph::new(err.clone())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(Span::styled("Error", Style::default().fg(Color::Red))),
            )
            .wrap(Wrap { trim: true });
        let popup = centered_rect(80, 30, area);
        f.render_widget(msg, popup);
    }
}

fn draw_footer(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let help = "↑/↓ (j/k) move  Enter open  Backspace up  r refresh  h hidden  q quit";
    let mut right = String::new();
    if app.error_count > 0 {
        right.push_str(&format!("⚠ {} error(s) ignored", app.error_count));
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(75), Constraint::Percentage(25)])
        .split(area);

    let left = Paragraph::new(help)
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(left, chunks[0]);

    let right = Paragraph::new(right)
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(right, chunks[1]);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn percent(part: u64, total: u64) -> u16 {
    if total == 0 {
        return 0;
    }
    let pct = (part as f64 / total as f64) * 100.0;
    pct.round().clamp(0.0, 100.0) as u16
}

fn bar(size: u64, max: u64, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if max == 0 {
        return "░".repeat(width);
    }
    let filled = ((size as f64 / max as f64) * width as f64)
        .round()
        .clamp(0.0, width as f64) as usize;
    let filled = filled.min(width);
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

