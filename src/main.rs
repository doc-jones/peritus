use chrono::prelude::*;
use crossterm::{
    event::{self, Event as CEvent, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use rand::{distributions::Alphanumeric, prelude::*};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;
use tui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{
        Block, BorderType, Borders, Cell, List, ListItem, ListState, Paragraph, Row, Table, Tabs,
    },
    Terminal,
};

const DB_PATH: &str = "./data/db.json";

#[derive(Error, Debug)]
pub enum Error {
    #[error("error reading the DB file: {0}")]
    ReadDBError(#[from] io::Error),
    #[error("error parsing the DB file: {0}")]
    ParseDBError(#[from] serde_json::Error),
}

enum Event<I> {
    Input(I),
    Tick,
}

#[derive(Serialize, Deserialize, Clone)]
struct Expert {
    id: usize,
    name: String,
    category: String,
    age: usize,
    created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug)]
enum MenuItem {
    Home,
    Experts,
}

impl From<MenuItem> for usize {
    fn from(input: MenuItem) -> usize {
        match input {
            MenuItem::Home => 0,
            MenuItem::Experts => 1,
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode().expect("can run in raw mode");

    let (tx, rx) = mpsc::channel();
    let tick_rate = Duration::from_millis(200);
    thread::spawn(move || {
        let mut last_tick = Instant::now();
        loop {
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if event::poll(timeout).expect("poll works") {
                if let CEvent::Key(key) = event::read().expect("can read events") {
                    tx.send(Event::Input(key)).expect("can send events");
                }
            }

            if last_tick.elapsed() >= tick_rate {
                if let Ok(_) = tx.send(Event::Tick) {
                    last_tick = Instant::now();
                }
            }
        }
    });

    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let menu_titles = vec!["Home", "Experts", "Add", "Delete", "Quit"];
    let mut active_menu_item = MenuItem::Home;
    let mut expert_list_state = ListState::default();
    expert_list_state.select(Some(0));

    loop {
        terminal.draw(|rect| {
            let size = rect.size();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints(
                    [
                        Constraint::Length(3),
                        Constraint::Min(2),
                        Constraint::Length(3),
                    ]
                    .as_ref(),
                )
                .split(size);

            let copyright = Paragraph::new("peritus-CLI 2021 - just kidding - no copyrights here")
                .style(Style::default().fg(Color::LightCyan))
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .style(Style::default().fg(Color::White))
                        .title("Copyright")
                        .border_type(BorderType::Plain),
                );

            let menu = menu_titles
                .iter()
                .map(|t| {
                    let (first, rest) = t.split_at(1);
                    Spans::from(vec![
                        Span::styled(
                            first,
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::UNDERLINED),
                        ),
                        Span::styled(rest, Style::default().fg(Color::White)),
                    ])
                })
                .collect();

            let tabs = Tabs::new(menu)
                .select(active_menu_item.into())
                .block(Block::default().title("Menu").borders(Borders::ALL))
                .style(Style::default().fg(Color::White))
                .highlight_style(Style::default().fg(Color::Yellow))
                .divider(Span::raw("|"));

            rect.render_widget(tabs, chunks[0]);
            match active_menu_item {
                MenuItem::Home => rect.render_widget(render_home(), chunks[1]),
                MenuItem::Experts => {
                    let experts_chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints(
                            [Constraint::Percentage(20), Constraint::Percentage(80)].as_ref(),
                        )
                        .split(chunks[1]);
                    let (left, right) = render_experts(&expert_list_state);
                    rect.render_stateful_widget(left, experts_chunks[0], &mut expert_list_state);
                    rect.render_widget(right, experts_chunks[1]);
                }
            }
            rect.render_widget(copyright, chunks[2]);
        })?;

        match rx.recv()? {
            Event::Input(event) => match event.code {
                KeyCode::Char('q') => {
                    disable_raw_mode()?;
                    terminal.show_cursor()?;
                    break;
                }
                KeyCode::Char('h') => active_menu_item = MenuItem::Home,
                KeyCode::Char('e') => active_menu_item = MenuItem::Experts,
                KeyCode::Char('a') => {
                    add_random_expert_to_db().expect("can add new random expert");
                }
                KeyCode::Char('d') => {
                    remove_expert_at_index(&mut expert_list_state).expect("can remove expert");
                }
                KeyCode::Down => {
                    if let Some(selected) = expert_list_state.selected() {
                        let amount_experts = read_db().expect("can fetch expert list").len();
                        if selected >= amount_experts - 1 {
                            expert_list_state.select(Some(0));
                        } else {
                            expert_list_state.select(Some(selected + 1));
                        }
                    }
                }
                KeyCode::Up => {
                    if let Some(selected) = expert_list_state.selected() {
                        let amount_experts = read_db().expect("can fetch expert list").len();
                        if selected > 0 {
                            expert_list_state.select(Some(selected - 1));
                        } else {
                            expert_list_state.select(Some(amount_experts - 1));
                        }
                    }
                }
                _ => {}
            },
            Event::Tick => {}
        }
    }

    Ok(())
}

fn render_home<'a>() -> Paragraph<'a> {
    let home = Paragraph::new(vec![
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("Welcome")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("to")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::styled(
            "Peritus the Rustc Experts-CLI",
            Style::default().fg(Color::LightCyan),
        )]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("Press 'e' to access experts, 'a' to add random new experts and 'd' to delete the currently selected expert.")]),
    ])
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .title("Home")
            .border_type(BorderType::Plain),
    );
    home
}

fn render_experts<'a>(expert_list_state: &ListState) -> (List<'a>, Table<'a>) {
    let experts = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White))
        .title("Experts")
        .border_type(BorderType::Plain);

    let expert_list = read_db().expect("can fetch expert list");
    let items: Vec<_> = expert_list
        .iter()
        .map(|expert| {
            ListItem::new(Spans::from(vec![Span::styled(
                expert.name.clone(),
                Style::default(),
            )]))
        })
        .collect();

    let selected_expert = expert_list
        .get(
            expert_list_state
                .selected()
                .expect("there is always a selected expert"),
        )
        .expect("exists")
        .clone();

    let list = List::new(items).block(experts).highlight_style(
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    );

    let expert_detail = Table::new(vec![Row::new(vec![
        Cell::from(Span::raw(selected_expert.id.to_string())),
        Cell::from(Span::raw(selected_expert.name)),
        Cell::from(Span::raw(selected_expert.category)),
        Cell::from(Span::raw(selected_expert.age.to_string())),
        Cell::from(Span::raw(selected_expert.created_at.to_string())),
    ])])
    .header(Row::new(vec![
        Cell::from(Span::styled(
            "ID",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Name",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Category",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Age",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Created At",
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .title("Detail")
            .border_type(BorderType::Plain),
    )
    .widths(&[
        Constraint::Percentage(5),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(5),
        Constraint::Percentage(20),
    ]);

    (list, expert_detail)
}

fn read_db() -> Result<Vec<Expert>, Error> {
    let db_content = fs::read_to_string(DB_PATH)?;
    let parsed: Vec<Expert> = serde_json::from_str(&db_content)?;
    Ok(parsed)
}

fn add_random_expert_to_db() -> Result<Vec<Expert>, Error> {
    let mut rng = rand::thread_rng();
    let db_content = fs::read_to_string(DB_PATH)?;
    let mut parsed: Vec<Expert> = serde_json::from_str(&db_content)?;
    let areadirs = match rng.gen_range(0..1) {
        0 => "areas",
        _ => "directories",
    };

    let random_expert = Expert {
        id: rng.gen_range(0..=9999999),
        name: rng.sample_iter(Alphanumeric).take(10).map(char::from).collect(),
        category: areadirs.to_owned(),
        age: 6,
        created_at: Utc::now(),
    };

    parsed.push(random_expert);
    fs::write(DB_PATH, &serde_json::to_vec(&parsed)?)?;
    Ok(parsed)
}

fn remove_expert_at_index(expert_list_state: &mut ListState) -> Result<(), Error> {
    if let Some(selected) = expert_list_state.selected() {
        let db_content = fs::read_to_string(DB_PATH)?;
        let mut parsed: Vec<Expert> = serde_json::from_str(&db_content)?;
        parsed.remove(selected);
        fs::write(DB_PATH, &serde_json::to_vec(&parsed)?)?;
        expert_list_state.select(Some(selected - 1));
    }
    Ok(())
}
