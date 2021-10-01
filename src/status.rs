use std::fs;
use std::path::Path;

use ev3dev_lang_rust::{Ev3Result, Led, PowerSupply};

const COLOR_LIME: &str = "lime";
const COLOR_YELLOW: &str = "yellow";
const COLOR_AMBER: &str = "amber";
const COLOR_ORANGE: &str = "orange";
const COLOR_RED: &str = "red";
const COLOR_OFF: &str = "black";

pub struct Status {
    led: Led,
    power: PowerSupply,
    connection: ConnectionState,
}

impl Status {
    pub fn new() -> Ev3Result<Status> {
        let mut status = Status {
            led: Led::new().unwrap(),
            power: PowerSupply::new().unwrap(),
            connection: ConnectionState::Disconnected,
        };

        if !Path::new("name").exists() {
            status.set_name(String::from("EV3"));
        }

        if !Path::new("color").exists() {
            status.set_color(String::from(COLOR_OFF))
        }

        status.load_color();

        Ok(status)
    }

    pub fn get_name(&self) -> String {
        fs::read_to_string("name").unwrap_or_else(|_| String::new())
    }
    pub fn set_name(&self, name: String) {
        fs::write("name", name).unwrap();
    }

    pub fn get_color(&self) -> String {
        fs::read_to_string("color").unwrap_or_else(|_| String::new())
    }
    pub fn set_color(&mut self, color: String) {
        fs::write("color", color).unwrap();
        self.load_color()
    }

    pub fn get_power(&mut self) -> f32 {
        let now = self.power.get_voltage_now().unwrap();
        let max = self.power.get_voltage_max_design().unwrap();
        let min = self.power.get_voltage_min_design().unwrap();

        ((now - min) as f32 / (max - min) as f32).max(0.0).min(1.0)
    }

    pub fn set_connection_state(&mut self, connected: ConnectionState) {
        self.connection = connected;
        self.load_color()
    }

    fn load_color(&mut self) {
        let main_color = match self.get_color().as_ref() {
            COLOR_LIME => Led::COLOR_GREEN,
            COLOR_YELLOW => Led::COLOR_YELLOW,
            COLOR_AMBER => Led::COLOR_AMBER,
            COLOR_ORANGE => Led::COLOR_ORANGE,
            COLOR_RED => Led::COLOR_RED,
            _ => Led::COLOR_OFF,
        };

        let status_color = match self.connection {
            ConnectionState::Disconnected => Led::COLOR_RED,
            ConnectionState::Connecting => Led::COLOR_AMBER,
            ConnectionState::Connected => Led::COLOR_GREEN,
            ConnectionState::Reconnecting => Led::COLOR_YELLOW,
        };

        self.led.set_left_color(main_color).unwrap();
        self.led.set_right_color(status_color).unwrap();
    }

    pub fn get_available_colors(&self) -> Vec<String> {
        vec![
            String::from(COLOR_LIME),
            String::from(COLOR_YELLOW),
            String::from(COLOR_AMBER),
            String::from(COLOR_ORANGE),
            String::from(COLOR_RED),
        ]
    }

    pub fn get_version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }
}

pub enum ConnectionState {
    Disconnected,
    Connected,
    Connecting,
    Reconnecting,
}
