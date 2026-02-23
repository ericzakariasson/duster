//! Interactive TUI for visualizing cleanable disk usage

use anyhow::{Context, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::{
    io,
    path::{Path, PathBuf},
    time::Duration,
};
use sysinfo::Disks;

use crate::scanner::{CleanableFile, ScanResult};
use crate::ui;

#[derive(Debug, Clone)]
struct FileView {
    path: String,
    size: u64,
    reason: String,
}

#[derive(Debug, Clone)]
struct CategoryView {
    name: &'static str,
    size: u64,
    count: usize,
    files: Vec<FileView>,
}

#[derive(Debug, Clone)]
struct DiskSpaceInfo {
    total: u64,
    free: u64,
    mount_point: String,
}

#[derive(Debug)]
struct App {
    categories: Vec<CategoryView>,
    selected: usize,
    total_cleanable: u64,
    total_files: usize,
    errors: Vec<String>,
    disk_space: Option<DiskSpaceInfo>,
}

impl App {
    fn from_result(result: &ScanResult, scan_path: Option<&Path>) -> Self {
        let mut categories: Vec<CategoryView> = result
            .by_category()
            .into_iter()
            .map(|(category, files)| {
                let mut files: Vec<FileView> = files
                    .iter()
                    .map(|f| file_to_view(f))
                    .collect::<Vec<FileView>>();
                files.sort_by(|a, b| b.size.cmp(&a.size));

                let size = files.iter().map(|f| f.size).sum();
                let count = files.len();

                CategoryView {
                    name: category.display_name(),
                    size,
                    count,
                    files,
                }
            })
            .collect();

        categories.sort_by(|a, b| b.size.cmp(&a.size));

        Self {
            categories,
            selected: 0,
            total_cleanable: result.total_size(),
            total_files: result.total_count(),
            errors: result.errors.clone(),
            disk_space: lookup_disk_space(scan_path),
        }
    }

    fn next_category(&mut self) {
        if self.categories.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.categories.len();
    }

    fn previous_category(&mut self) {
        if self.categories.is_empty() {
            return;
        }
        self.selected = if self.selected == 0 {
            self.categories.len() - 1
        } else {
            self.selected - 1
        };
    }

    fn select_first(&mut self) {
        self.selected = 0;
    }

    fn select_last(&mut self) {
        if self.categories.is_empty() {
            return;
        }
        self.selected = self.categories.len() - 1;
    }

    fn selected_category(&self) -> Option<&CategoryView> {
        self.categories.get(self.selected)
    }
}

/// Launch interactive TUI and block until user exits with `q` / `Esc`.
pub fn run(result: &ScanResult, scan_path: Option<&Path>) -> Result<()> {
    let mut app = App::from_result(result, scan_path);

    let mut stdout = io::stdout();
    enable_raw_mode().context("Failed to enable raw mode")?;
    execute!(stdout, EnterAlternateScreen).context("Failed to enter alternate screen")?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("Failed to create terminal backend")?;
    terminal.clear().context("Failed to clear terminal")?;

    let event_loop_result = run_event_loop(&mut terminal, &mut app);

    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .context("Failed to leave alternate screen")?;
    terminal.show_cursor().context("Failed to restore cursor")?;

    event_loop_result
}

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|frame| draw(frame, app))?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Down | KeyCode::Char('j') => app.next_category(),
                    KeyCode::Up | KeyCode::Char('k') => app.previous_category(),
                    KeyCode::Home => app.select_first(),
                    KeyCode::End => app.select_last(),
                    _ => {}
                }
            }
        }
    }
}

fn draw(frame: &mut Frame<'_>, app: &App) {
    let mut constraints = vec![Constraint::Length(3), Constraint::Min(8)];
    if !app.errors.is_empty() {
        constraints.push(Constraint::Length(4));
    }
    constraints.push(Constraint::Length(2));

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(frame.area());

    render_header(frame, app, sections[0]);
    render_main(frame, app, sections[1]);

    let footer_index = if app.errors.is_empty() {
        2
    } else {
        render_errors(frame, app, sections[2]);
        3
    };
    render_footer(frame, sections[footer_index]);
}

fn render_header(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let line = if let Some(disk) = &app.disk_space {
        let used = disk.total.saturating_sub(disk.free);
        let cleanable_percent = if disk.total == 0 {
            0.0
        } else {
            (app.total_cleanable as f64 / disk.total as f64) * 100.0
        };

        format!(
            "Mount {} | Total {} | Free {} | Used {} | Cleanable {} ({:.1}% of disk)",
            disk.mount_point,
            ui::format_size(disk.total),
            ui::format_size(disk.free),
            ui::format_size(used),
            ui::format_size(app.total_cleanable),
            cleanable_percent
        )
    } else {
        format!(
            "Total cleanable: {} across {} files",
            ui::format_size(app.total_cleanable),
            ui::format_number(app.total_files as u64)
        )
    };

    let widget = Paragraph::new(line)
        .style(Style::default().fg(Color::Cyan))
        .block(Block::default().borders(Borders::ALL).title("Duster TUI"))
        .wrap(Wrap { trim: true });

    frame.render_widget(widget, area);
}

fn render_main(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    render_category_list(frame, app, columns[0]);
    render_category_details(frame, app, columns[1]);
}

fn render_category_list(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let items = if app.categories.is_empty() {
        vec![ListItem::new(Line::from("No cleanable categories found"))]
    } else {
        app.categories
            .iter()
            .map(|category| {
                let ratio = if app.total_cleanable == 0 {
                    0.0
                } else {
                    category.size as f64 / app.total_cleanable as f64
                };

                let label = format!(
                    "{:<16} {:>10} {:>5} {}",
                    truncate_text(category.name, 16),
                    ui::format_size(category.size),
                    category.count,
                    ascii_bar(ratio, 10)
                );

                ListItem::new(Line::from(label))
            })
            .collect::<Vec<ListItem>>()
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Categories (size | files | usage)"),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    let mut state = ListState::default();
    if !app.categories.is_empty() {
        state.select(Some(app.selected));
    }
    frame.render_stateful_widget(list, area, &mut state);
}

fn render_category_details(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Largest items in selected category");

    if let Some(category) = app.selected_category() {
        let path_width = area.width.saturating_sub(14) as usize;
        let mut lines = vec![
            Line::from(vec![Span::styled(
                format!(
                    "{} | {} files",
                    ui::format_size(category.size),
                    ui::format_number(category.count as u64)
                ),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
        ];

        for file in category.files.iter().take(12) {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{:>10} ", ui::format_size(file.size)),
                    Style::default().fg(Color::Magenta),
                ),
                Span::raw(truncate_text(&file.path, path_width)),
            ]));
        }

        if category.files.len() > 12 {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("... {} more items", category.files.len() - 12),
                Style::default().fg(Color::DarkGray),
            )));
        }

        if let Some(file) = category.files.first() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("Top file reason: {}", file.reason),
                Style::default().fg(Color::DarkGray),
            )));
        }

        frame.render_widget(
            Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
            area,
        );
    } else {
        frame.render_widget(
            Paragraph::new("No cleanable files found.")
                .block(block)
                .wrap(Wrap { trim: true }),
            area,
        );
    }
}

fn render_errors(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let mut lines = vec![Line::from(Span::styled(
        format!("{} scanner error(s) detected:", app.errors.len()),
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    ))];

    for error in app.errors.iter().take(2) {
        lines.push(Line::from(format!("- {}", truncate_text(error, 90))));
    }
    if app.errors.len() > 2 {
        lines.push(Line::from(format!("... {} more", app.errors.len() - 2)));
    }

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Scanner errors"),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(widget, area);
}

fn render_footer(frame: &mut Frame<'_>, area: Rect) {
    let footer = Paragraph::new("Keys: up/down or j/k move | Home/End jump | q or Esc quit")
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(footer, area);
}

fn file_to_view(file: &CleanableFile) -> FileView {
    FileView {
        path: ui::format_path(&file.path),
        size: file.size,
        reason: file.reason.clone(),
    }
}

fn ascii_bar(ratio: f64, width: usize) -> String {
    let ratio = ratio.clamp(0.0, 1.0);
    let filled = (ratio * width as f64).round() as usize;
    format!(
        "{}{}",
        "#".repeat(filled.min(width)),
        "-".repeat(width.saturating_sub(filled))
    )
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    let len = text.chars().count();
    if len <= max_chars {
        return text.to_string();
    }

    if max_chars <= 3 {
        return ".".repeat(max_chars);
    }

    format!(
        "{}...",
        text.chars()
            .take(max_chars.saturating_sub(3))
            .collect::<String>()
    )
}

fn lookup_disk_space(scan_path: Option<&Path>) -> Option<DiskSpaceInfo> {
    let target = resolve_target_path(scan_path)?;
    let disks = Disks::new_with_refreshed_list();

    let mut matching = disks
        .list()
        .iter()
        .filter(|disk| target.starts_with(disk.mount_point()))
        .map(|disk| {
            (
                disk.mount_point().to_path_buf(),
                disk.total_space(),
                disk.available_space(),
            )
        })
        .collect::<Vec<(PathBuf, u64, u64)>>();

    matching.sort_by(|a, b| b.0.as_os_str().len().cmp(&a.0.as_os_str().len()));
    let (mount_point, total, free) = matching.into_iter().next()?;

    Some(DiskSpaceInfo {
        total,
        free,
        mount_point: mount_point.display().to_string(),
    })
}

fn resolve_target_path(scan_path: Option<&Path>) -> Option<PathBuf> {
    let path = if let Some(path) = scan_path {
        path.to_path_buf()
    } else if let Some(home) = dirs::home_dir() {
        home
    } else {
        std::env::current_dir().ok()?
    };

    path.canonicalize().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::{Category, ScanResult};
    use chrono::Utc;

    fn cleanable(path: &str, size: u64, category: Category) -> CleanableFile {
        CleanableFile {
            path: PathBuf::from(path),
            size,
            category,
            last_accessed: Utc::now(),
            reason: "test".to_string(),
            is_directory: false,
        }
    }

    #[test]
    fn sorts_categories_by_size_desc() {
        let mut result = ScanResult::new();
        result.add_files(vec![
            cleanable("/tmp/cache-a", 100, Category::Cache),
            cleanable("/tmp/build-a", 500, Category::BuildArtifact),
            cleanable("/tmp/cache-b", 150, Category::Cache),
        ]);

        let app = App::from_result(&result, None);
        assert_eq!(app.categories.len(), 2);
        assert_eq!(
            app.categories[0].name,
            Category::BuildArtifact.display_name()
        );
        assert_eq!(app.categories[1].name, Category::Cache.display_name());
    }

    #[test]
    fn truncates_text_with_ellipsis() {
        assert_eq!(truncate_text("short", 10), "short");
        assert_eq!(truncate_text("abcdef", 3), "...");
        assert_eq!(truncate_text("abcdef", 5), "ab...");
    }

    #[test]
    fn renders_ascii_bar_with_clamped_ratio() {
        assert_eq!(ascii_bar(0.0, 5), "-----");
        assert_eq!(ascii_bar(0.5, 4), "##--");
        assert_eq!(ascii_bar(1.5, 4), "####");
    }
}
