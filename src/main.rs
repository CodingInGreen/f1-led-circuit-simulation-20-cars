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
    date: DateTime<Utc>,
    x_led: f64,
    y_led: f64,
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
            date: String,
            x_led: f64,
            y_led: f64,
            time_delta: Option<u64>, // Deserialize time_delta from CSV, allowing for missing values
        }

        let helper = RunRaceHelper::deserialize(deserializer)?;
        let date = Utc.datetime_from_str(&helper.date, "%+")
            .map_err(SerdeError::custom)?;

        Ok(RunRace {
            date,
            x_led: helper.x_led,
            y_led: helper.y_led,
            time_delta: helper.time_delta.unwrap_or(0), // Default to 0 if missing
        })
    }
}

struct PlotApp {
    coordinates: Vec<LedCoordinate>,
    run_race_data: Vec<Vec<RunRace>>, // Changed to a vector of vectors to hold multiple datasets
    start_time: Instant,
    start_datetime: DateTime<Utc>,
    current_index: usize,
    race_started: bool,
    next_update_time: DateTime<Utc>, // New field to hold the next update time
    colors: Vec<egui::Color32>, // Colors for each dataset
}

impl PlotApp {
    fn new(coordinates: Vec<LedCoordinate>, run_race_data: Vec<Vec<RunRace>>, colors: Vec<egui::Color32>) -> Self {
        let mut app = Self {
            coordinates,
            run_race_data,
            start_time: Instant::now(),
            start_datetime: Utc::now(),
            current_index: 0,
            race_started: false,
            next_update_time: Utc::now(), // Initialize next_update_time
            colors,
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
        if let Some(run_data) = self.run_race_data.get(0).and_then(|data| data.get(self.current_index)) {
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

            if let Some(run_data) = self.run_race_data.get(0).and_then(|data| data.get(self.current_index)) {
                if current_time >= self.next_update_time {
                    self.current_index += 1;
                    self.calculate_next_update_time(); // Calculate next update time for the next data point
                }
            }
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Add the date field in the center of the menu bar
                ui.separator(); // Align items to center
                if let Some(run_data) = self.run_race_data.get(0).and_then(|data| data.get(self.current_index)) {
                    let date_str = run_data.date.format("%H:%M:%S%.3f").to_string();
                    ui.label(date_str);
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
            // First, draw all LEDs as black
            for coord in &self.coordinates {
                let norm_x = ((coord.x_led - min_x) / width) as f32 * ui.available_width();
                let norm_y = ui.available_height() - (((coord.y_led - min_y) / height) as f32 * ui.available_height());

                painter.rect_filled(
                    egui::Rect::from_min_size(
                        egui::pos2(norm_x, norm_y),
                        egui::vec2(20.0, 20.0),
                    ),
                    egui::Rounding::same(0.0),
                    egui::Color32::BLACK,
                );
            }

            // Then, update LEDs with car colors if there's a match
            for coord in &self.coordinates {
                let norm_x = ((coord.x_led - min_x) / width) as f32 * ui.available_width();
                let norm_y = ui.available_height() - (((coord.y_led - min_y) / height) as f32 * ui.available_height());

                for (dataset_idx, dataset) in self.run_race_data.iter().enumerate() {
                    let color = self.colors[dataset_idx];

                    for i in 0..self.current_index {
                        if let Some(run_data) = dataset.get(i) {
                            println!("Checking car {} at ({}, {}) against LED ({}, {})",
                                     dataset_idx, run_data.x_led, run_data.y_led, coord.x_led, coord.y_led); // Debug print
                            if run_data.x_led == coord.x_led && run_data.y_led == coord.y_led {
                                println!("Match found: Drawing color {:?} for car {} at coordinate ({}, {})",
                                         color, dataset_idx, coord.x_led, coord.y_led); // Debug print
                                painter.rect_filled(
                                     egui::Rect::from_min_size(
                                        egui::pos2(norm_x, norm_y),
                                        egui::vec2(20.0, 20.0),
                                    ),
                                    egui::Rounding::same(0.0),
                                    color,
                                );
                                break; // Exit the loop as we found a match
                            }
                        }
                    }
                }
            }
        });


        // Request a repaint to ensure continuous updates
        ctx.request_repaint();
    }
}

fn main() -> eframe::Result<()> {
    let coordinates = read_coordinates("led_coords.csv").expect("Error reading CSV");

    // Specify file paths for multiple datasets
    let dataset_paths = vec![
        "time_delta_albon_start.csv",
        "time_delta_alonso_start.csv",
        "time_delta_bottas_start.csv",
        "time_delta_gasley_start.csv",
        "time_delta_guanyu_start.csv",
        "time_delta_hamilton_start.csv",
        "time_delta_hulkenberg_start.csv",
        "time_delta_lawson_start.csv",
        "time_delta_leclerc_start.csv",
        "time_delta_magnussen_start.csv",
        "time_delta_norris_start.csv",
        "time_delta_ocon_start.csv",
        "time_delta_perez_start.csv",
        "time_delta_piastri_start.csv",
        "time_delta_russell_start.csv",
        "time_delta_sainz_start.csv",
        "time_delta_sargeant_start.csv",
        "time_delta_stroll_start.csv",
        "time_delta_tsunoda_start.csv",
        "time_delta_verstappen_start.csv",
    ];


    // Read multiple datasets
    let mut run_race_data = Vec::new();
    for file_path in dataset_paths {
        let data = read_race_data(file_path).expect("Error reading CSV");
        run_race_data.push(data);
    }

    // Debug print to check data
    for (i, data) in run_race_data.iter().enumerate() {
        println!("Dataset {}: {} records", i, data.len());
        for record in data.iter().take(5) { // Print the first 5 records of each dataset
            println!("{:?}", record);
        }
    }


    // Define colors for each dataset
    let colors = vec![
        egui::Color32::from_rgb(255, 0, 0),    // Red
        egui::Color32::from_rgb(0, 255, 0),    // Green
        egui::Color32::from_rgb(0, 0, 255),    // Blue
        egui::Color32::from_rgb(255, 255, 0),  // Yellow
        egui::Color32::from_rgb(255, 0, 255),  // Magenta
        egui::Color32::from_rgb(0, 255, 255),  // Cyan
        egui::Color32::from_rgb(128, 0, 0),    // Maroon
        egui::Color32::from_rgb(0, 128, 0),    // Dark Green
        egui::Color32::from_rgb(0, 0, 128),    // Navy
        egui::Color32::from_rgb(128, 128, 0),  // Olive
        egui::Color32::from_rgb(128, 0, 128),  // Purple
        egui::Color32::from_rgb(128, 0, 128),  // Purple
        egui::Color32::from_rgb(0, 128, 128),  // Teal
        egui::Color32::from_rgb(192, 192, 192), // Silver
        egui::Color32::from_rgb(128, 128, 128), // Gray
        egui::Color32::from_rgb(255, 165, 0),  // Orange
        egui::Color32::from_rgb(255, 20, 147), // Deep Pink
        egui::Color32::from_rgb(75, 0, 130),   // Indigo
        egui::Color32::from_rgb(255, 215, 0),  // Gold
        egui::Color32::from_rgb(0, 191, 255),  // Deep Sky Blue
        egui::Color32::from_rgb(255, 105, 180) // Hot Pink
    ];

    let app = PlotApp::new(coordinates, run_race_data, colors);

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
    let mut is_first_row = true;

    for result in rdr.deserialize() {
        if is_first_row {
            is_first_row = false;
            continue; // Skip the first row
        }
        let record: RunRace = result?;
        run_race_data.push(record);
    }
    Ok(run_race_data)
}