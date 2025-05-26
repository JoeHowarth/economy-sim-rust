use std::collections::{HashMap, VecDeque};
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Sparkline},
};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use crate::events::{DeathCause, Event as SimEvent, EventLogger, EventType};

/// UI mode for viewing simulation data
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum UIMode {
    Replay,   // Playing through historical events
    Live,     // Following live simulation (not yet implemented)
    Paused,   // Manual stepping
    Analysis, // Post-simulation analysis view
}

/// Production history for a resource
#[derive(Debug, Default)]
struct ResourceHistory {
    last_production: Decimal,
    last_consumption: Decimal,
    production_history: VecDeque<Decimal>,  // Last 50 ticks
    consumption_history: VecDeque<Decimal>, // Last 50 ticks
}

impl ResourceHistory {
    fn record_production(&mut self, amount: Decimal) {
        self.last_production = amount;
        self.production_history.push_back(amount);
        if self.production_history.len() > 50 {
            self.production_history.pop_front();
        }
    }

    fn record_consumption(&mut self, amount: Decimal) {
        self.last_consumption = amount;
        self.consumption_history.push_back(amount);
        if self.consumption_history.len() > 50 {
            self.consumption_history.pop_front();
        }
    }

    fn avg_production(&self, ticks: usize) -> Decimal {
        let count = self.production_history.len().min(ticks);
        if count == 0 {
            return Decimal::ZERO;
        }
        let sum: Decimal = self.production_history.iter().rev().take(count).sum();
        sum / Decimal::from(count)
    }

    fn avg_consumption(&self, ticks: usize) -> Decimal {
        let count = self.consumption_history.len().min(ticks);
        if count == 0 {
            return Decimal::ZERO;
        }
        let sum: Decimal = self.consumption_history.iter().rev().take(count).sum();
        sum / Decimal::from(count)
    }
}

/// State for a single village reconstructed from events
#[derive(Debug, Default)]
struct VillageState {
    id: String,
    population: usize,
    food: Decimal,
    wood: Decimal,
    money: Decimal,
    houses: usize,
    food_workers: usize,
    wood_workers: usize,
    construction_workers: usize,
    idle_workers: usize,
    recent_deaths: Vec<(usize, DeathCause)>, // (tick, cause)
    last_birth: Option<usize>,               // tick
    // Production tracking
    food_history: ResourceHistory,
    wood_history: ResourceHistory,
    // Trade tracking
    last_food_trade: Option<(Decimal, Decimal)>, // (amount, price)
    last_wood_trade: Option<(Decimal, Decimal)>, // (amount, price)
}

/// Main UI state
pub struct UIState {
    events: Vec<SimEvent>,
    current_tick: usize,
    villages: HashMap<String, VillageState>,
    #[allow(dead_code)]
    mode: UIMode,
    seconds_per_tick: f32, // Changed from playback_speed
    max_tick: usize,
    recent_events: Vec<String>, // Formatted event strings
    paused: bool,
    last_tick_time: Instant,
}

impl UIState {
    pub fn new(events: Vec<SimEvent>) -> Self {
        let max_tick = events.iter().map(|e| e.tick).max().unwrap_or(0);

        let mut ui = Self {
            events,
            current_tick: 0,
            villages: HashMap::new(),
            mode: UIMode::Replay,
            seconds_per_tick: 0.5, // 2 ticks per second default
            max_tick,
            recent_events: Vec::new(),
            paused: false,
            last_tick_time: Instant::now(),
        };

        // Process all events up to tick 0 to get initial state
        ui.process_events_to_tick(0);
        ui
    }

    /// Process events up to and including the given tick
    fn process_events_to_tick(&mut self, target_tick: usize) {
        // Clone events to avoid borrow conflict
        let events_to_process: Vec<SimEvent> = self
            .events
            .iter()
            .filter(|e| e.tick <= target_tick && e.tick >= self.current_tick)
            .cloned()
            .collect();

        for event in events_to_process {
            self.process_event(&event);
        }
        self.current_tick = target_tick;
    }

    /// Process a single event and update state
    fn process_event(&mut self, event: &SimEvent) {
        let village = self.villages.entry(event.village_id.clone()).or_default();
        village.id = event.village_id.clone();

        // Add to recent events (keep last 10)
        if self.recent_events.len() >= 10 {
            self.recent_events.remove(0);
        }
        self.recent_events
            .push(format!("[{}] {}", event.tick, event));

        match &event.event_type {
            EventType::VillageStateSnapshot {
                population,
                houses,
                food,
                wood,
                money,
            } => {
                village.population = *population;
                village.houses = *houses;
                village.food = *food;
                village.wood = *wood;
                village.money = *money;
            }
            EventType::WorkerAllocation {
                food_workers,
                wood_workers,
                construction_workers,
                repair_workers: _,
                idle_workers,
            } => {
                village.food_workers = *food_workers;
                village.wood_workers = *wood_workers;
                village.construction_workers = *construction_workers;
                village.idle_workers = *idle_workers;
            }
            EventType::ResourceProduced {
                resource, amount, ..
            } => match resource {
                crate::events::ResourceType::Food => {
                    village.food_history.record_production(*amount);
                }
                crate::events::ResourceType::Wood => {
                    village.wood_history.record_production(*amount);
                }
            },
            EventType::ResourceConsumed {
                resource, amount, ..
            } => match resource {
                crate::events::ResourceType::Food => {
                    village.food_history.record_consumption(*amount);
                }
                crate::events::ResourceType::Wood => {
                    village.wood_history.record_consumption(*amount);
                }
            },
            EventType::WorkerBorn { .. } => {
                village.last_birth = Some(event.tick);
            }
            EventType::WorkerDied { cause, .. } => {
                village.recent_deaths.push((event.tick, cause.clone()));
                // Keep only recent deaths (last 5)
                if village.recent_deaths.len() > 5 {
                    village.recent_deaths.remove(0);
                }
            }
            EventType::TradeExecuted {
                resource,
                quantity,
                price,
                side,
                ..
            } => {
                let signed_quantity = match side {
                    crate::events::TradeSide::Buy => *quantity,
                    crate::events::TradeSide::Sell => -*quantity,
                };
                match resource {
                    crate::events::ResourceType::Food => {
                        village.last_food_trade = Some((signed_quantity, *price));
                    }
                    crate::events::ResourceType::Wood => {
                        village.last_wood_trade = Some((signed_quantity, *price));
                    }
                }
            }
            _ => {}
        }
    }

    /// Advance simulation by one tick
    fn step_forward(&mut self) {
        if self.current_tick < self.max_tick {
            let next_tick = self.current_tick + 1;

            // Process all events for the next tick
            for event in &self.events.clone() {
                if event.tick == next_tick {
                    self.process_event(event);
                }
            }

            self.current_tick = next_tick;
        }
    }

    /// Go back one tick
    fn step_backward(&mut self) {
        if self.current_tick > 0 {
            // Clear state and replay from beginning
            self.villages.clear();
            self.recent_events.clear();
            let target = self.current_tick - 1;
            self.current_tick = 0;
            self.process_events_to_tick(target);
        }
    }

    /// Jump to specific tick
    fn jump_to_tick(&mut self, tick: usize) {
        if tick <= self.max_tick {
            self.villages.clear();
            self.recent_events.clear();
            self.current_tick = 0;
            self.process_events_to_tick(tick);
        }
    }
}

/// Run the UI event viewer
pub fn run_ui(event_file: &str) -> io::Result<()> {
    // Load events from file
    let events = if Path::new(event_file).exists() {
        EventLogger::load_from_file(event_file)?
            .get_events()
            .to_vec()
    } else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Event file not found: {}", event_file),
        ));
    };

    if events.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "No events found in file",
        ));
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create UI state
    let mut ui_state = UIState::new(events);

    // Main loop
    let res = run_app(&mut terminal, &mut ui_state);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    ui_state: &mut UIState,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| draw_ui(f, ui_state))?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char(' ') => ui_state.paused = !ui_state.paused,
                        KeyCode::Right => {
                            ui_state.step_forward();
                            ui_state.last_tick_time = Instant::now();
                        }
                        KeyCode::Left => {
                            ui_state.step_backward();
                            ui_state.last_tick_time = Instant::now();
                        }
                        KeyCode::Home => {
                            ui_state.jump_to_tick(0);
                            ui_state.last_tick_time = Instant::now();
                        }
                        KeyCode::End => {
                            ui_state.jump_to_tick(ui_state.max_tick);
                            ui_state.last_tick_time = Instant::now();
                        }
                        KeyCode::Char('+') | KeyCode::Char('=') => {
                            ui_state.seconds_per_tick =
                                (ui_state.seconds_per_tick / 2.0).max(0.0625); // Max 16 ticks/sec
                        }
                        KeyCode::Char('-') => {
                            ui_state.seconds_per_tick = (ui_state.seconds_per_tick * 2.0).min(4.0); // Min 0.25 ticks/sec
                        }
                        _ => {}
                    }
                }
            }
        }

        // Auto-advance if not paused and enough time has passed
        if !ui_state.paused && ui_state.current_tick < ui_state.max_tick {
            let elapsed = ui_state.last_tick_time.elapsed().as_secs_f32();
            if elapsed >= ui_state.seconds_per_tick {
                ui_state.step_forward();
                ui_state.last_tick_time = Instant::now();
            }
        }
    }
}

fn draw_ui(f: &mut Frame, ui_state: &UIState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(10),    // Main content
            Constraint::Length(10), // Event log
            Constraint::Length(1),  // Footer
        ])
        .split(f.area());

    // Header
    let speed_display = if ui_state.seconds_per_tick >= 1.0 {
        format!("{:.1}s/tick", ui_state.seconds_per_tick)
    } else {
        format!("{:.1} ticks/s", 1.0 / ui_state.seconds_per_tick)
    };

    let header = Paragraph::new(format!(
        "Village Simulation Viewer - Day {}/{} - Speed: {} {}",
        ui_state.current_tick,
        ui_state.max_tick,
        speed_display,
        if ui_state.paused { "[PAUSED]" } else { "" }
    ))
    .style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(header, chunks[0]);

    // Main content - villages
    let village_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            ui_state
                .villages
                .iter()
                .map(|_| Constraint::Ratio(1, ui_state.villages.len() as u32))
                .collect::<Vec<_>>(),
        )
        .split(chunks[1]);

    for (i, (_, village)) in ui_state.villages.iter().enumerate() {
        draw_village(f, village_chunks[i], village);
    }

    // Event log
    let events: Vec<ListItem> = ui_state
        .recent_events
        .iter()
        .map(|e| ListItem::new(e.as_str()))
        .collect();

    let events_list = List::new(events)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Recent Events"),
        )
        .style(Style::default().fg(Color::White));
    f.render_widget(events_list, chunks[2]);

    // Footer
    let footer = Paragraph::new("[Q] Quit  [Space] Pause  [←→] Step  [Home/End] Jump  [+/-] Speed")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(footer, chunks[3]);
}

fn draw_village(f: &mut Frame, area: Rect, village: &VillageState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", village.id));
    f.render_widget(block, area);

    // Adjust chunks for inner area
    let inner = Block::default().borders(Borders::ALL).inner(area);
    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Basic stats
            Constraint::Length(2), // Resources
            Constraint::Length(2), // Workers (now more compact)
            Constraint::Length(6), // Production info
            Constraint::Length(3), // Sparkline trends
            Constraint::Min(1),    // Recent events or spacer
        ])
        .split(inner);

    // Basic stats
    let pop_color = if village.population == 0 {
        Color::Red
    } else if village.recent_deaths.len() > 2 {
        Color::Yellow
    } else {
        Color::Green
    };

    // Add status indicator
    let status_indicator = if village.population == 0 {
        "💀"
    } else if village.recent_deaths.len() > 2 {
        "⚠️"
    } else if village
        .last_birth
        .is_some_and(|tick| tick + 5 > village.food_history.production_history.len())
    {
        "👶"
    } else {
        "✓"
    };

    let stats = vec![Line::from(vec![
        Span::raw(format!("{} Pop: ", status_indicator)),
        Span::styled(
            format!("{:2}", village.population),
            Style::default().fg(pop_color).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  🏠 "),
        Span::styled(
            format!("{}", village.houses),
            Style::default().fg(Color::White),
        ),
        Span::raw(format!(" ({} cap)", village.houses * 5)),
    ])];
    let stats_para = Paragraph::new(stats);
    f.render_widget(stats_para, inner_chunks[0]);

    // Resources with better formatting
    let food_color = if village.food < Decimal::from(10) {
        Color::Red
    } else if village.food < Decimal::from(30) {
        Color::Yellow
    } else {
        Color::White
    };

    let wood_color = if village.wood < Decimal::from(5) {
        Color::Red
    } else if village.wood < Decimal::from(20) {
        Color::Yellow
    } else {
        Color::White
    };

    let resources = vec![Line::from(vec![
        Span::raw("🌾 "),
        Span::styled(
            format!("{:5.1}", village.food),
            Style::default().fg(food_color),
        ),
        Span::raw("  🪵 "),
        Span::styled(
            format!("{:5.1}", village.wood),
            Style::default().fg(wood_color),
        ),
        Span::raw("  💰 "),
        Span::styled(
            format!("{:3.0}", village.money),
            Style::default().fg(Color::Yellow),
        ),
    ])];
    let resources_para = Paragraph::new(resources);
    f.render_widget(resources_para, inner_chunks[1]);

    // Workers - compact display with visual indicators
    let workers = vec![
        Line::from(Span::styled(
            "Workers:",
            Style::default().add_modifier(Modifier::UNDERLINED),
        )),
        Line::from(vec![
            Span::raw("  🌾"),
            Span::styled(
                format!("{:2}", village.food_workers),
                Style::default().fg(if village.food_workers > 0 {
                    Color::Green
                } else {
                    Color::DarkGray
                }),
            ),
            Span::raw("  🪵"),
            Span::styled(
                format!("{:2}", village.wood_workers),
                Style::default().fg(if village.wood_workers > 0 {
                    Color::Green
                } else {
                    Color::DarkGray
                }),
            ),
            Span::raw("  🔨"),
            Span::styled(
                format!("{:2}", village.construction_workers),
                Style::default().fg(if village.construction_workers > 0 {
                    Color::Green
                } else {
                    Color::DarkGray
                }),
            ),
            Span::raw("  💤"),
            Span::styled(
                format!("{:2}", village.idle_workers),
                Style::default().fg(if village.idle_workers > 0 {
                    Color::Yellow
                } else {
                    Color::DarkGray
                }),
            ),
        ]),
    ];
    let workers_para = Paragraph::new(workers);
    f.render_widget(workers_para, inner_chunks[2]);

    // Production info
    let food_avg_10 = village.food_history.avg_production(10);
    let food_avg_50 = village.food_history.avg_production(50);
    let wood_avg_10 = village.wood_history.avg_production(10);
    let wood_avg_50 = village.wood_history.avg_production(50);

    let food_cons_avg = village.food_history.avg_consumption(10);
    let wood_cons_avg = village.wood_history.avg_consumption(10);

    // Calculate net production
    let food_net = food_avg_10 - food_cons_avg;
    let wood_net = wood_avg_10 - wood_cons_avg;

    let production_info = vec![
        Line::from(Span::styled(
            "Production (last→10→50):",
            Style::default().add_modifier(Modifier::UNDERLINED),
        )),
        Line::from(vec![
            Span::raw("  🌾 "),
            Span::styled(
                format!("{:4.1}", village.food_history.last_production),
                Style::default().fg(Color::White),
            ),
            Span::styled(" → ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:4.1}", food_avg_10),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(" → ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:4.1}", food_avg_50),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(vec![
            Span::raw("  🪵 "),
            Span::styled(
                format!("{:4.1}", village.wood_history.last_production),
                Style::default().fg(Color::White),
            ),
            Span::styled(" → ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:4.1}", wood_avg_10),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(" → ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:4.1}", wood_avg_50),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "Net/tick (10 avg):",
                Style::default().add_modifier(Modifier::UNDERLINED),
            ),
            Span::raw("  "),
            Span::raw("🌾 "),
            Span::styled(
                format!("{:+4.1}", food_net),
                Style::default().fg(if food_net >= Decimal::ZERO {
                    Color::Green
                } else {
                    Color::Red
                }),
            ),
            Span::raw("  🪵 "),
            Span::styled(
                format!("{:+4.1}", wood_net),
                Style::default().fg(if wood_net >= Decimal::ZERO {
                    Color::Green
                } else {
                    Color::Red
                }),
            ),
        ]),
    ];
    let production_para = Paragraph::new(production_info);
    f.render_widget(production_para, inner_chunks[3]);

    // Sparkline trends
    let food_data: Vec<u64> = village
        .food_history
        .production_history
        .iter()
        .map(|d| (d.to_f64().unwrap_or(0.0) * 10.0) as u64)
        .collect();

    if !food_data.is_empty() {
        let trend_block = Block::default().borders(Borders::NONE).title(Span::styled(
            "Trends",
            Style::default().add_modifier(Modifier::UNDERLINED),
        ));

        let inner_trend = trend_block.inner(inner_chunks[4]);
        f.render_widget(trend_block, inner_chunks[4]);

        let sparkline = Sparkline::default()
            .data(&food_data)
            .style(Style::default().fg(Color::Green));
        f.render_widget(sparkline, inner_trend);
    }

    // Show recent events in remaining space if any
    if !village.recent_deaths.is_empty()
        || village.last_food_trade.is_some()
        || village.last_wood_trade.is_some()
    {
        let mut recent_info = vec![];

        // Show recent deaths
        if let Some((_tick, cause)) = village.recent_deaths.last() {
            let death_text = match cause {
                DeathCause::Starvation => "💀 Starved",
                DeathCause::NoShelter => "🥶 No shelter",
            };
            recent_info.push(Line::from(Span::styled(
                death_text,
                Style::default().fg(Color::Red),
            )));
        }

        // Show recent trades
        if let Some((amt, price)) = village.last_food_trade {
            let trade_text = if amt > Decimal::ZERO {
                format!("🌾 Bought {:.1} @ {:.2}", amt, price)
            } else {
                format!("🌾 Sold {:.1} @ {:.2}", -amt, price)
            };
            recent_info.push(Line::from(Span::styled(
                trade_text,
                Style::default().fg(Color::Cyan),
            )));
        }

        if !recent_info.is_empty() {
            let recent_para = Paragraph::new(recent_info);
            f.render_widget(recent_para, inner_chunks[5]);
        }
    }
}
