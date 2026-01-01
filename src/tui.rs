use crate::DayStats;
use anyhow::Result;
use chrono::{Datelike, Duration, Local, NaiveDate};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::Widget,
    Frame, Terminal,
};
use std::collections::HashMap;
use std::io::{self, Stdout};

pub fn run(stats: HashMap<NaiveDate, DayStats>, days: i64) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal, stats, days);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    stats: HashMap<NaiveDate, DayStats>,
    days: i64,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &stats, days))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if let KeyCode::Char('q') = key.code {
                    return Ok(());
                }
                if let KeyCode::Esc = key.code {
                    return Ok(());
                }
            }
        }
    }
}

fn ui(f: &mut Frame, stats: &HashMap<NaiveDate, DayStats>, days: i64) {
    let glob_area = f.area();
    let padded_area = Rect {
        x: glob_area.x + 4,
        y: glob_area.y + 2, // More top padding
        width: glob_area.width.saturating_sub(4),
        height: glob_area.height.saturating_sub(2),
    };

    // Layout: Two heatmaps vertically
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(11), // Height for heatmap
            Constraint::Length(1),  // Spacer
            Constraint::Length(11),
            Constraint::Min(0),
        ])
        .split(padded_area);

    let commits_total: usize = stats.values().map(|s| s.commits).sum();
    let lines_total: usize = stats.values().map(|s| s.lines_changed).sum();

    HeatmapWidget {
        stats: stats,
        days: days,
        title: "Commits",
        total: commits_total,
        get_val: |s: &DayStats| s.commits,
    }
    .render(areas[0], f.buffer_mut());

    HeatmapWidget {
        stats: stats,
        days: days,
        title: "Lines Changed",
        total: lines_total,
        get_val: |s: &DayStats| s.lines_changed,
    }
    .render(areas[2], f.buffer_mut());
}

struct HeatmapWidget<'a, F> {
    stats: &'a HashMap<NaiveDate, DayStats>,
    days: i64,
    title: &'a str,
    total: usize,
    get_val: F,
}

impl<'a, F> Widget for HeatmapWidget<'a, F>
where
    F: Fn(&DayStats) -> usize,
{
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Find max value for scaling logic (ignoring 0)
        let max_val = self
            .stats
            .values()
            .map(|s| (self.get_val)(s))
            .max()
            .unwrap_or(1)
            .max(1); // Ensure at least 1

        let now = Local::now().date_naive();
        let start_date = now - Duration::days(self.days);

        // --- Render Title ---
        let title_str = format!("{} contributions in the last {} days ({})", self.total, self.days, self.title);
        buf.set_string(area.x, area.y, &title_str, Style::default());

        // --- Grid Constants ---
        let graph_x_offset = area.x + 4; // Space for Mon/Wed/Fri labels
        let graph_y_offset = area.y + 2; // Space for Month labels

        // --- Render Month Labels ---
        // Iterate through weeks, if month changes, print label
        let mut current_month = 0;
        for i in (0..self.days).step_by(7) {
             let date = start_date + Duration::days(i);
             // Approximate column
             let col = (i / 7) as u16; 
             if col * 2 + graph_x_offset >= area.right() { break; }

             if date.month() != current_month {
                 current_month = date.month();
                 let month_name = date.format("%b").to_string(); // Jan, Feb...
                 buf.set_string(graph_x_offset + col * 2, area.y + 1, month_name, Style::default().fg(Color::DarkGray));
             }
        }

        let label_style = Style::default().fg(Color::DarkGray);
        buf.set_string(area.x, graph_y_offset + 1, "Mon", label_style);
        buf.set_string(area.x, graph_y_offset + 3, "Wed", label_style);
        buf.set_string(area.x, graph_y_offset + 5, "Fri", label_style);


        // --- Render Grid ---
        for i in 0..self.days {
            let date = start_date + Duration::days(i);
            if date > now { break; }

            let weekday = date.weekday().num_days_from_sunday() as u16; // Sun=0, Sat=6
            
            let start_sunday = start_date - Duration::days(start_date.weekday().num_days_from_sunday() as i64);
            let days_since_grid_start = (date - start_sunday).num_days();
            
            let col = (days_since_grid_start / 7) as u16;
            let row = weekday;

            let x = graph_x_offset + col * 2;
            let y = graph_y_offset + row;

            if x + 1 >= area.right() || y >= area.bottom() {
                continue;
            }
            
            let val = self.stats.get(&date).map(|s| (self.get_val)(s)).unwrap_or(0);
            let color = get_github_color(val, max_val);
            let symbol = "■"; // Square block
            
            buf.set_string(x, y, symbol, Style::default().fg(color));
        }
    }
}

fn get_github_color(val: usize, max_val: usize) -> Color {
    if val == 0 {
        return Color::Rgb(22, 27, 34); // GitHub empty (dark mode bg-ish) - actually let's use DarkGray for terminals
    }

    let ratio = val as f64 / max_val as f64;
    
    // #ebedf0 (0) -> not using, using dark
    // #9be9a8 (1) -> #bbdefb (Light Blue)
    // #40c463 (2) -> #64b5f6
    // #30a14e (3) -> #1976d2
    // #216e39 (4) -> #0d47a1 (Dark Blue)
    
    if ratio < 0.25 {
        Color::Rgb(144, 202, 249) 
    } else if ratio < 0.5 {
         Color::Rgb(66, 165, 245) 
    } else if ratio < 0.75 {
        Color::Rgb(30, 136, 229)
    } else {
        Color::Rgb(21, 101, 192) 
    }
}
