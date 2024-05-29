#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use csv::ReaderBuilder;
use serde::{Deserialize, Deserializer};
use serde::de::Error as SerdeError;
use eframe::{egui, App, Frame};
use std::error::Error;
use std::time::{Duration, Instant};
use chrono::{DateTime, Utc, TimeZone};

#[derive(Debug, Deserialize)]
struct LedCoordinate {
    x_led: f64,
    y_led: f64,
}

#[derive(Debug)]
struct RunRace {
    timestamp: DateTime<Utc>,
    x_data: f64,
    y_data: f64,
    time_delta: u64, // New field to hold the time delta
}

// Custom deserialization for RunRace to handle DateTime
impl<'de> Deserialize<'de> for RunRace {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RunRaceHelper {
            timestamp: String,
            x_data: f64,
            y_data: f64,
            time_delta: u64, // Deserialize time_delta from CSV
        }

        let helper = RunRaceHelper::deserialize(deserializer)?;
        let timestamp = Utc.datetime_from_str(&helper.timestamp, "%+")
            .map_err(SerdeError::custom)?;

        Ok(RunRace {
            timestamp,
            x_data: helper.x_data,
            y_data: helper.y_data,
            time_delta: helper.time_delta,
        })
    }
}

struct PlotApp {
    coordinates: Vec<LedCoordinate>,
    run_race_data: Vec<RunRace>,
    start_time: Instant,
    start_datetime: DateTime<Utc>,
    current_index: usize,
    race_started: bool,
    next_update_time: DateTime<Utc>, // New field to hold the next update time
}

impl PlotApp {
    fn new(coordinates: Vec<LedCoordinate>, run_race_data: Vec<RunRace>) -> Self {
        let mut app = Self {
            coordinates,
            run_race_data,
            start_time: Instant::now(),
            start_datetime: Utc::now(),
            current_index: 0,
            race_started: false,
            next_update_time: Utc::now(), // Initialize next_update_time
        };
        app.calculate_next_update_time(); // Calculate initial next_update_time
        app
    }

    fn reset(&mut self) {
        self.start_time = Instant::now();
        self.start_datetime = Utc::now();
        self.current_index = 0;
        self.race_started = false;
        self.calculate_next_update_time(); // Calculate next_update_time after reset
    }

    fn calculate_next_update_time(&mut self) {
        if let Some(run_data) = self.run_race_data.get(self.current_index) {
            self.next_update_time = Utc::now() + Duration::from_millis(run_data.time_delta);
        }
    }
}

impl App for PlotApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("my_layer")));

        let (min_x, max_x) = self.coordinates.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), coord| {
            (min.min(coord.x_led), max.max(coord.x_led))
        });
        let (min_y, max_y) = self.coordinates.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), coord| {
            (min.min(coord.y_led), max.max(coord.y_led))
        });

        let width = max_x - min_x;
        let height = max_y - min_y;

        if self.race_started {
            let current_time = Utc::now();

            if let Some(run_data) = self.run_race_data.get(self.current_index) {
                if current_time >= self.next_update_time {
                    self.current_index += 1;
                    self.calculate_next_update_time(); // Calculate next update time for the next data point
                }
            }
        }



        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Add the timestamp field in the center of the menu bar
                ui.separator(); // Align items to center
                if let Some(run_data) = self.run_race_data.get(self.current_index) {
                    let timestamp_str = run_data.timestamp.format("%H:%M:%S%.3f").to_string();
                    ui.label(timestamp_str);
                }
                ui.separator(); // Align items to center
        
                if ui.button("START").clicked() {
                    self.race_started = true;
                    self.start_time = Instant::now();
                    self.start_datetime = Utc::now();
                    self.current_index = 0;
                    self.calculate_next_update_time(); // Calculate next update time when race starts
                }
                if ui.button("STOP").clicked() {
                    self.reset();
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            for coord in &self.coordinates {
                // Draw LEDs based on coordinates
                let norm_x = ((coord.x_led - min_x) / width) as f32 * ui.available_width();
                let norm_y = ui.available_height() - (((coord.y_led - min_y) / height) as f32 * ui.available_height());
                let mut color = egui::Color32::BLACK;

                for i in 0..self.current_index {
                    if let Some(run_data) = self.run_race_data.get(i) {
                        if run_data.x_data == coord.x_led && run_data.y_data == coord.y_led {
                            color = egui::Color32::GREEN;
                        } else {
                            color = egui::Color32::BLACK;
                        }
                    }
                }

                painter.rect_filled(
                    egui::Rect::from_min_size(
                        egui::pos2(norm_x, norm_y),
                        egui::vec2(20.0, 20.0),
                    ),
                    egui::Rounding::same(0.0),
                    color,
                );
            }
        });

        // Request a repaint to ensure continuous updates
        ctx.request_repaint();
    }
}

fn main() -> eframe::Result<()> {
    let coordinates = read_coordinates("led_coords.csv").expect("Error reading CSV");
    let run_race_data = read_race_data("modified_race_data.csv").expect("Error reading CSV");

    let app = PlotApp::new(coordinates, run_race_data);

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "F1-LED-CIRCUIT SIMULATION",
        native_options,
        Box::new(|_cc| Box::new(app)),
    )
}

fn read_coordinates(file_path: &str) -> Result<Vec<LedCoordinate>, Box<dyn Error>> {
    let mut rdr = ReaderBuilder::new().from_path(file_path)?;
    let mut coordinates = Vec::new();
    for result in rdr.deserialize() {
        let record: LedCoordinate = result?;
        coordinates.push(record);
    }
    Ok(coordinates)
}

fn read_race_data(file_path: &str) -> Result<Vec<RunRace>, Box<dyn Error>> {
    let mut rdr = ReaderBuilder::new().from_path(file_path)?;
    let mut run_race_data = Vec::new();
    for result in rdr.deserialize() {
        let record: RunRace = result?;
        run_race_data.push(record);
    }
    Ok(run_race_data)
}
