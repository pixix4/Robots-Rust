use std::io::Result;
use std::path::Path;
use std::fs;

use ev3dev_lang_rust::led::{self, Led};
use ev3dev_lang_rust::power_supply::PowerSupply;

const COLOR_LIME: &'static str = "lime";
const COLOR_YELLOW: &'static str = "yellow";
const COLOR_AMBER: &'static str = "amber";
const COLOR_ORANGE: &'static str = "orange";
const COLOR_RED: &'static str = "red";
const COLOR_OFF: &'static str = "black";

pub struct Status {
    led: Led,
    power: PowerSupply,
    connection: ConnectionState,
}

impl Status {
    pub fn new() -> Result<Status> {
        let mut status = Status {
            led: Led::new().unwrap(),
            power: PowerSupply::new().unwrap(),
            connection: ConnectionState::DISCONNECTED,
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
        fs::read_to_string("name").unwrap_or_else(|_| { String::new() })
    }
    pub fn set_name(&self, name: String) {
        fs::write("name", name).unwrap();
    }

    pub fn get_color(&self) -> String {
        fs::read_to_string("color").unwrap_or_else(|_| { String::new() })
    }
    pub fn set_color(&mut self, color: String) {
        fs::write("color", color).unwrap();
        self.load_color()
    }

    pub fn get_power(&mut self) -> f32 {
        let now = self.power.get_voltage_now().unwrap();
        let max = self.power.get_voltage_max_design().unwrap();
        let min = self.power.get_voltage_min_design().unwrap();

        return ((now - min) as f32 / (max - min) as f32).max(0.0).min(1.0);
    }

    pub fn set_connection_state(&mut self, connected: ConnectionState) {
        self.connection = connected;
        self.load_color()
    }

    fn load_color(&mut self) {
        let main_color = match self.get_color().as_ref() {
            COLOR_LIME => {
                led::COLOR_GREEN
            }
            COLOR_YELLOW => {
                led::COLOR_YELLOW
            }
            COLOR_AMBER => {
                led::COLOR_AMBER
            }
            COLOR_ORANGE => {
                led::COLOR_ORANGE
            }
            COLOR_RED => {
                led::COLOR_RED
            }
            _ => {
                led::COLOR_OFF
            }
        };


        let status_color = match self.connection {
            ConnectionState::DISCONNECTED => {
                led::COLOR_RED
            }
            ConnectionState::CONNECTING => {
                led::COLOR_AMBER
            }
            ConnectionState::CONNECTED => {
                led::COLOR_GREEN
            }
            ConnectionState::RECONNECTING => {
                led::COLOR_YELLOW
            }
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
            String::from(COLOR_RED)
        ]
    }

    pub fn get_version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }
}

pub enum ConnectionState {
    DISCONNECTED,
    CONNECTED,
    CONNECTING,
    RECONNECTING,
}