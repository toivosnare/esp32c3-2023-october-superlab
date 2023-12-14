use std::collections::VecDeque;
use std::io::{self, Stdout};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use rumqttc::{Client, Event, Incoming, MqttOptions, QoS};
use serde::Deserialize;

use termion::event::Key;
use termion::input::TermRead;
use termion::raw::{IntoRawMode, RawTerminal};

use tui::backend::TermionBackend;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Modifier, Style};
use tui::symbols::Marker;
use tui::text::{Span, Spans};
use tui::widgets::{Axis, Block, Borders, Chart, Dataset, Paragraph};
use tui::Terminal;

const UI_REFRESH_DELAY: Duration = Duration::from_millis(25);
const DATA_CAPACITY: usize = 100;

#[derive(Deserialize, Debug)]
pub enum Sensor {
    Rear,
    Top,
    FrontLeft,
    FrontRight,
}

#[derive(Deserialize, Debug)]
pub struct Message<'a> {
    pub client_id: &'a str,
    pub sensor: Sensor,
    pub temperature: f32,
    pub humidity: f32,
}

#[derive(Default)]
struct Data {
    capacity: usize,
    temperature: VecDeque<f32>,
    humidity: VecDeque<f32>,
    average_temperature: f32,
    average_humidity: f32,
    data_points_collected: usize,
}

impl Data {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            ..Default::default()
        }
    }

    fn push(&mut self, temperature: f32, humidity: f32) {
        self.temperature.push_front(temperature);
        self.temperature.truncate(self.capacity);
        self.humidity.push_front(humidity);
        self.humidity.truncate(self.capacity);

        self.data_points_collected += 1;
        self.average_temperature -= self.average_temperature / self.data_points_collected as f32;
        self.average_temperature += temperature / self.data_points_collected as f32;
        self.average_humidity -= self.average_humidity / self.data_points_collected as f32;
        self.average_humidity += humidity / self.data_points_collected as f32;
    }
}

fn main() -> Result<(), io::Error> {
    let running = Arc::new(AtomicBool::new(true));
    let run_render_loop = running.clone();
    thread::spawn(move || {
        for key in io::stdin().keys() {
            if let Ok(Key::Ctrl('c')) = key {
                running.store(false, Ordering::SeqCst);
                break;
            }
        }
    });

    let mut mqtt_options = MqttOptions::new("server", "localhost", 1883);
    mqtt_options.set_keep_alive(Duration::from_secs(5));
    let (mut client, mut connection) = Client::new(mqtt_options, 10);
    client.subscribe("measurements", QoS::AtMostOnce).unwrap();

    let stdout = io::stdout().into_raw_mode()?;
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear().unwrap();
    terminal.hide_cursor().unwrap();

    let mut data = Data::new(DATA_CAPACITY);

    for notification in connection.iter() {
        if !run_render_loop.load(Ordering::SeqCst) {
            break;
        }
        thread::sleep(UI_REFRESH_DELAY);

        let event = match notification {
            Ok(e) => e,
            Err(_) => continue,
        };
        let publish = match event {
            Event::Incoming(Incoming::Publish(p)) => p,
            _ => continue,
        };
        let message: Message = serde_json::from_slice(&publish.payload[..]).unwrap();
        data.push(message.temperature, message.humidity);
        // println!("{:?}", message);
        render(&mut terminal, &data);
    }

    let _ = terminal.clear();
    let _ = terminal.show_cursor();
    Ok(())
}

fn render(terminal: &mut Terminal<TermionBackend<RawTerminal<Stdout>>>, data: &Data) {
    terminal
        .draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
                .split(f.size());

            let bold_style = Style::default().add_modifier(Modifier::BOLD);
            let status_text = Spans::from(vec![
                Span::raw("Average temperature: "),
                Span::styled(format!("{:.2}", data.average_temperature), bold_style),
                Span::raw(" °C, "),
                Span::raw("Average humidity: "),
                Span::styled(format!("{:.2}", data.average_humidity), bold_style),
                Span::raw(" %"),
            ]);
            let status_widget = Paragraph::new(status_text);
            f.render_widget(status_widget, chunks[0]);

            let temperature_data: Vec<_> = data
                .temperature
                .iter()
                .rev()
                .enumerate()
                .map(|(i, x): (usize, &f32)| (i as f64, *x as f64))
                .collect();
            let humidity_data: Vec<_> = data
                .humidity
                .iter()
                .rev()
                .enumerate()
                .map(|(i, x): (usize, &f32)| (i as f64, *x as f64))
                .collect();
            let datasets = vec![
                Dataset::default()
                    .name("Temperature [°C]")
                    .marker(Marker::Dot)
                    .style(Style::default().fg(Color::Red))
                    .data(temperature_data.as_slice()),
                Dataset::default()
                    .name("Humidity [%]")
                    .marker(Marker::Dot)
                    .style(Style::default().fg(Color::Blue))
                    .data(humidity_data.as_slice()),
            ];
            let chart = Chart::new(datasets)
                .block(Block::default().title("Graph").borders(Borders::ALL))
                .x_axis(
                    Axis::default()
                        .style(Style::default().fg(Color::White))
                        .bounds([0.0, DATA_CAPACITY as f64]),
                )
                .y_axis(
                    Axis::default()
                        .style(Style::default().fg(Color::White))
                        .bounds([0.0, 50.0])
                        .labels(
                            ["0", "10", "20", "30", "40", "50"]
                                .iter()
                                .cloned()
                                .map(Span::from)
                                .collect(),
                        ),
                );
            f.render_widget(chart, chunks[1]);
        })
        .unwrap();
}
