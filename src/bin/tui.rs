//! Real-time EEG chart viewer for the Neurofield Q21 amplifier.
//!
//! Usage:
//!   cargo run --bin tui --release
//!   cargo run --bin tui --release -- --simulate
//!
//! Keys:
//!   +  / =   zoom out  (increase µV scale)
//!   -        zoom in   (decrease µV scale)
//!   a        auto-scale Y axis to current peak amplitude
//!   v        toggle smooth overlay (dim raw + bright moving-average)
//!   p        pause streaming
//!   r        resume streaming
//!   c        clear waveform buffers
//!   q / Esc  quit

use std::collections::VecDeque;
use std::f64::consts::PI;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph},
    Frame, Terminal,
};

use neurofield::prelude::*;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Width of the scrolling waveform window in seconds.
const WINDOW_SECS: f64 = 2.0;

/// Number of samples retained per channel.
const BUF_SIZE: usize = (WINDOW_SECS * SAMPLING_RATE) as usize;

/// Discrete Y-axis scale steps in µV (half the full symmetric range).
const Y_SCALES: &[f64] = &[10.0, 25.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0];

/// Default scale index (±500 µV).
const DEFAULT_SCALE: usize = 5;

/// Per-channel colours (first 4, then repeat).
const COLORS: [Color; 4] = [Color::Cyan, Color::Yellow, Color::Green, Color::Magenta];

/// Dimmed versions for smooth background.
const DIM_COLORS: [Color; 4] = [
    Color::Rgb(0, 90, 110),
    Color::Rgb(110, 90, 0),
    Color::Rgb(0, 110, 0),
    Color::Rgb(110, 0, 110),
];

/// Moving-average window in samples.
const SMOOTH_WINDOW: usize = 9;

// ── App state ─────────────────────────────────────────────────────────────────

struct App {
    bufs: Vec<VecDeque<f64>>,
    total_samples: u64,
    pkt_times: VecDeque<Instant>,
    scale_idx: usize,
    paused: bool,
    smooth: bool,
    connected: bool,
    simulated: bool,
}

impl App {
    fn new() -> Self {
        Self {
            bufs: (0..NUM_CHANNELS)
                .map(|_| VecDeque::with_capacity(BUF_SIZE + 16))
                .collect(),
            total_samples: 0,
            pkt_times: VecDeque::with_capacity(256),
            scale_idx: DEFAULT_SCALE,
            paused: false,
            smooth: true,
            connected: false,
            simulated: false,
        }
    }

    fn push(&mut self, ch: usize, value: f64) {
        if self.paused || ch >= NUM_CHANNELS {
            return;
        }
        let buf = &mut self.bufs[ch];
        buf.push_back(value);
        while buf.len() > BUF_SIZE {
            buf.pop_front();
        }
        if ch == 0 {
            self.total_samples += 1;
            let now = Instant::now();
            self.pkt_times.push_back(now);
            while self
                .pkt_times
                .front()
                .map(|t| now.duration_since(*t) > Duration::from_secs(2))
                .unwrap_or(false)
            {
                self.pkt_times.pop_front();
            }
        }
    }

    fn clear(&mut self) {
        for b in &mut self.bufs {
            b.clear();
        }
        self.total_samples = 0;
        self.pkt_times.clear();
    }

    fn pkt_rate(&self) -> f64 {
        let n = self.pkt_times.len();
        if n < 2 {
            return 0.0;
        }
        let span = self
            .pkt_times
            .back()
            .unwrap()
            .duration_since(self.pkt_times[0])
            .as_secs_f64();
        if span < 1e-9 { 0.0 } else { (n as f64 - 1.0) / span }
    }

    fn y_range(&self) -> f64 {
        Y_SCALES[self.scale_idx]
    }

    fn scale_up(&mut self) {
        if self.scale_idx + 1 < Y_SCALES.len() {
            self.scale_idx += 1;
        }
    }

    fn scale_down(&mut self) {
        if self.scale_idx > 0 {
            self.scale_idx -= 1;
        }
    }

    fn auto_scale(&mut self) {
        let peak = self
            .bufs
            .iter()
            .flat_map(|b| b.iter())
            .fold(0.0_f64, |acc, &v| acc.max(v.abs()));
        let needed = peak * 1.1;
        self.scale_idx = Y_SCALES
            .iter()
            .position(|&s| s >= needed)
            .unwrap_or(Y_SCALES.len() - 1);
    }
}

// ── Simulator ─────────────────────────────────────────────────────────────────

fn sim_sample(t: f64, ch: usize) -> f64 {
    let phi = ch as f64 * PI / 2.5;
    let alpha = 20.0 * (2.0 * PI * 10.0 * t + phi).sin();
    let beta = 6.0 * (2.0 * PI * 22.0 * t + phi * 1.7).sin();
    let theta = 10.0 * (2.0 * PI * 6.0 * t + phi * 0.9).sin();
    let nx = t * 1000.7 + ch as f64 * 137.508;
    let noise = ((nx.sin() * 9973.1).fract() - 0.5) * 8.0;
    alpha + beta + theta + noise
}

fn spawn_simulator(app: Arc<Mutex<App>>) {
    std::thread::spawn(move || {
        let dt = 1.0 / SAMPLING_RATE;
        let pkt_interval = Duration::from_secs_f64(1.0 / SAMPLING_RATE);
        let mut t = 0.0_f64;
        loop {
            std::thread::sleep(pkt_interval);
            let mut s = app.lock().unwrap();
            if s.paused {
                t += dt;
                continue;
            }
            for ch in 0..NUM_CHANNELS {
                let v = sim_sample(t, ch);
                s.push(ch, v);
            }
            t += dt;
        }
    });
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn smooth_signal(data: &[(f64, f64)], window: usize) -> Vec<(f64, f64)> {
    if data.len() < 3 || window < 2 {
        return data.to_vec();
    }
    let half = window / 2;
    data.iter()
        .enumerate()
        .map(|(i, &(x, _))| {
            let start = i.saturating_sub(half);
            let end = (i + half + 1).min(data.len());
            let sum: f64 = data[start..end].iter().map(|&(_, y)| y).sum();
            (x, sum / (end - start) as f64)
        })
        .collect()
}

#[inline]
fn sep<'a>() -> Span<'a> {
    Span::styled(" │ ", Style::default().fg(Color::DarkGray))
}

#[inline]
fn key(s: &str) -> Span<'_> {
    Span::styled(
        s,
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )
}

// ── Rendering ─────────────────────────────────────────────────────────────────

fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let root = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(3),
    ])
    .split(area);

    draw_header(frame, root[0], app);
    draw_charts(frame, root[1], app);
    draw_footer(frame, root[2], app);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let (label, color) = if app.simulated {
        ("◆ Simulated".to_owned(), Color::Cyan)
    } else if app.connected {
        ("● Connected".to_owned(), Color::Green)
    } else {
        ("○ Disconnected".to_owned(), Color::Red)
    };

    let rate = format!("{:.1} smp/s", app.pkt_rate());
    let scale = format!("±{:.0} µV", app.y_range());
    let total = format!("{}K smp", app.total_samples / 1_000);

    let line = Line::from(vec![
        Span::styled(
            " Neurofield Q21 Monitor ",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        sep(),
        Span::styled(label, Style::default().fg(color).add_modifier(Modifier::BOLD)),
        sep(),
        Span::styled(rate, Style::default().fg(Color::White)),
        sep(),
        Span::styled(
            scale,
            Style::default()
                .fg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        ),
        sep(),
        Span::styled(total, Style::default().fg(Color::DarkGray)),
    ]);

    frame.render_widget(
        Paragraph::new(line).block(Block::default().borders(Borders::ALL)),
        area,
    );
}

fn draw_charts(frame: &mut Frame, area: Rect, app: &App) {
    // Show first 4 channels in large panels, rest in a compact grid below
    let main_ch = 4.min(NUM_CHANNELS);
    let extra_ch = NUM_CHANNELS.saturating_sub(main_ch);

    let mut constraints: Vec<Constraint> = (0..main_ch)
        .map(|_| Constraint::Ratio(1, main_ch as u32 + if extra_ch > 0 { 1 } else { 0 }))
        .collect();
    if extra_ch > 0 {
        constraints.push(Constraint::Ratio(1, main_ch as u32 + 1));
    }

    let rows = Layout::vertical(constraints).split(area);

    let y_range = app.y_range();

    // Main channels (one chart each)
    for ch in 0..main_ch {
        let data: Vec<(f64, f64)> = app.bufs[ch]
            .iter()
            .enumerate()
            .map(|(i, &v)| (i as f64 / SAMPLING_RATE, v.clamp(-y_range, y_range)))
            .collect();
        draw_channel(frame, rows[ch], ch, &data, app);
    }

    // Extra channels (compact grid in the remaining row)
    if extra_ch > 0 {
        let cols_per_row = 4;
        let grid_rows = extra_ch.div_ceil(cols_per_row);
        let row_constraints: Vec<Constraint> =
            (0..grid_rows).map(|_| Constraint::Ratio(1, grid_rows as u32)).collect();
        let grid_area = Layout::vertical(row_constraints).split(rows[main_ch]);

        for r in 0..grid_rows {
            let n_cols = cols_per_row.min(extra_ch - r * cols_per_row);
            let col_constraints: Vec<Constraint> =
                (0..n_cols).map(|_| Constraint::Ratio(1, n_cols as u32)).collect();
            let cols = Layout::horizontal(col_constraints).split(grid_area[r]);

            for c in 0..n_cols {
                let ch = main_ch + r * cols_per_row + c;
                let data: Vec<(f64, f64)> = app.bufs[ch]
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| (i as f64 / SAMPLING_RATE, v.clamp(-y_range, y_range)))
                    .collect();
                draw_channel(frame, cols[c], ch, &data, app);
            }
        }
    }
}

fn draw_channel(frame: &mut Frame, area: Rect, ch: usize, data: &[(f64, f64)], app: &App) {
    let color = COLORS[ch % COLORS.len()];
    let dim_color = DIM_COLORS[ch % DIM_COLORS.len()];
    let y_range = app.y_range();
    let name = EEG_CHANNEL_NAMES.get(ch).copied().unwrap_or("??");

    let (min_v, max_v, rms_v) = {
        let buf = &app.bufs[ch];
        if buf.is_empty() {
            (0.0_f64, 0.0_f64, 0.0_f64)
        } else {
            let min = buf.iter().copied().fold(f64::INFINITY, f64::min);
            let max = buf.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let rms = (buf.iter().map(|&v| v * v).sum::<f64>() / buf.len() as f64).sqrt();
            (min, max, rms)
        }
    };

    let clipping = max_v > y_range || min_v < -y_range;
    let border_color = if clipping { Color::Red } else { color };
    let clip_tag = if clipping { " [CLIP]" } else { "" };
    let smooth_tag = if app.smooth { " [SMOOTH]" } else { "" };
    let title = format!(
        " {name}  min:{min_v:+6.1}  max:{max_v:+6.1}  rms:{rms_v:5.1} µV{clip_tag}{smooth_tag} "
    );

    let smoothed: Vec<(f64, f64)> = if app.smooth {
        smooth_signal(data, SMOOTH_WINDOW)
    } else {
        vec![]
    };

    let datasets: Vec<Dataset> = if app.smooth {
        vec![
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(dim_color))
                .data(data),
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(color))
                .data(&smoothed),
        ]
    } else {
        vec![Dataset::default()
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(color))
            .data(data)]
    };

    let y_labels: Vec<String> = [-1.0, 0.0, 1.0]
        .iter()
        .map(|&f| format!("{:+.0}", f * y_range))
        .collect();
    let x_labels = vec![
        "0s".to_string(),
        format!("{:.1}s", WINDOW_SECS / 2.0),
        format!("{:.0}s", WINDOW_SECS),
    ];

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .title(Span::styled(
                    title,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        )
        .x_axis(
            Axis::default()
                .bounds([0.0, WINDOW_SECS])
                .labels(x_labels)
                .style(Style::default().fg(Color::DarkGray)),
        )
        .y_axis(
            Axis::default()
                .bounds([-y_range, y_range])
                .labels(y_labels)
                .style(Style::default().fg(Color::DarkGray)),
        );

    frame.render_widget(chart, area);
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let pause_span = if app.paused {
        Span::styled(
            "  ⏸ PAUSED",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw("")
    };

    let keys = Line::from(vec![
        Span::raw(" "),
        key("[+/-]"),
        Span::raw("Scale  "),
        key("[a]"),
        Span::raw("Auto  "),
        key("[v]"),
        Span::raw(if app.smooth { "Raw  " } else { "Smooth  " }),
        key("[p]"),
        Span::raw("Pause  "),
        key("[r]"),
        Span::raw("Resume  "),
        key("[c]"),
        Span::raw("Clear  "),
        key("[q]"),
        Span::raw("Quit"),
        pause_span,
    ]);

    let info = Line::from(Span::styled(
        " Neurofield Q21 · 20 channels · 256 Hz · PCAN-USB",
        Style::default().fg(Color::DarkGray),
    ));

    frame.render_widget(
        Paragraph::new(vec![keys, info]).block(Block::default().borders(Borders::ALL)),
        area,
    );
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let simulate = std::env::args().any(|a| a == "--simulate");

    let app = Arc::new(Mutex::new(App::new()));

    if simulate {
        let mut s = app.lock().unwrap();
        s.simulated = true;
        s.scale_idx = 2; // ±50 µV for simulator
        drop(s);
        spawn_simulator(Arc::clone(&app));
    } else {
        // Connect in a background thread (blocking CAN I/O)
        let app2 = Arc::clone(&app);
        std::thread::spawn(move || {
            match Q21Api::new(UsbBus::USB1) {
                Ok(mut api) => {
                    {
                        let mut s = app2.lock().unwrap();
                        s.connected = true;
                    }
                    if let Err(e) = api.start_receiving_eeg() {
                        log::error!("Failed to start EEG: {e}");
                        return;
                    }
                    loop {
                        match api.get_single_sample() {
                            Ok(sample) => {
                                let mut s = app2.lock().unwrap();
                                if s.paused {
                                    continue;
                                }
                                for ch in 0..NUM_CHANNELS {
                                    s.push(ch, sample.data[ch]);
                                }
                            }
                            Err(e) => {
                                log::error!("EEG read error: {e}");
                                let mut s = app2.lock().unwrap();
                                s.connected = false;
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to connect: {e}");
                }
            }
        });
    }

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    let tick = Duration::from_millis(33);

    // Main loop
    loop {
        {
            let s = app.lock().unwrap();
            terminal.draw(|f| draw(f, &s))?;
        }

        if !event::poll(tick)? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };

        let ctrl_c = key.modifiers.contains(KeyModifiers::CONTROL)
            && key.code == KeyCode::Char('c');
        if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc || ctrl_c {
            break;
        }

        match key.code {
            KeyCode::Char('+') | KeyCode::Char('=') => app.lock().unwrap().scale_up(),
            KeyCode::Char('-') => app.lock().unwrap().scale_down(),
            KeyCode::Char('a') => app.lock().unwrap().auto_scale(),
            KeyCode::Char('v') => {
                let mut s = app.lock().unwrap();
                s.smooth = !s.smooth;
            }
            KeyCode::Char('p') => app.lock().unwrap().paused = true,
            KeyCode::Char('r') => app.lock().unwrap().paused = false,
            KeyCode::Char('c') => app.lock().unwrap().clear(),
            _ => {}
        }
    }

    // Teardown
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
