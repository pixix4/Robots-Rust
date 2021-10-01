use driving::DrivingCommand;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use ev3dev_lang_rust::sensors::ColorSensor;
use ev3dev_lang_rust::Ev3Result;
use network::NetworkCommand;
use std::cmp::min;
use std::fs;
use std::time::Duration;

const COLOR_TIMEOUT: Duration = Duration::from_millis(500);

const CONST_PROPORTIONAL: f32 = 0.4;
const CONST_INTEGRAL: f32 = 0.18;
const CONST_DERIVATIVE: f32 = 0.25;
const INTEGRAL_MAXIMUM: f32 = 2.5;
const INTEGRAL_LIMITER: f32 = (INTEGRAL_MAXIMUM - 1.0) / INTEGRAL_MAXIMUM;
const SPEED: f32 = 0.6;
const SPEED_FAST: f32 = SPEED + 0.4;
const SPEED_NORMAL: f32 = SPEED;
const SPEED_SLOW: f32 = SPEED - 0.2;
const COUNTERMEASURE: f32 = 0.5;
const DRIVE_MULTIPLIER: f32 = -1.0;

fn calc_error(
    color_sensor: &mut ColorSensor,
    foreground_color: &(i32, i32, i32),
    background_color: &(i32, i32, i32),
    network: &Sender<NetworkCommand>,
) -> Ev3Result<f32> {
    let sensor = color_sensor.get_rgb()?;
    let red =
        (sensor.0 - foreground_color.0) as f32 / (background_color.0 - foreground_color.0) as f32;
    let green =
        (sensor.1 - foreground_color.1) as f32 / (background_color.1 - foreground_color.1) as f32;
    let blue =
        (sensor.2 - foreground_color.2) as f32 / (background_color.2 - foreground_color.2) as f32;

    network
        .send(NetworkCommand::Color(
            min(255, sensor.0 / 2) as u8,
            min(255, sensor.1 / 2) as u8,
            min(255, sensor.2 / 2) as u8,
        ))
        .unwrap();

    Ok((((red + green + blue) / 1.5).min(2.0).max(0.0) - 1.0) * DRIVE_MULTIPLIER)
}

fn run(
    pid_receiver: &Receiver<PidCommand>,
    driving_sender: &Sender<DrivingCommand>,
    color_sensor: &mut ColorSensor,
    foreground_color: &mut (i32, i32, i32),
    background_color: &mut (i32, i32, i32),
    network: &Sender<NetworkCommand>,
) -> Ev3Result<()> {
    let mut history_error: f32 = 0.0;
    let mut last_error: f32 = 0.0;
    let mut integral: f32 = 0.0;
    let dt: f32 = 1.0;
    let mut lost_line = 0;
    let mut drive_slow = 0;

    loop {
        if let Ok(recv) = pid_receiver.try_recv() {
            match recv {
                PidCommand::Start => {
                    //Do nothing
                }
                PidCommand::Stop => {
                    break;
                }
                PidCommand::SetForeground => {
                    let (r, g, b) = color_sensor.get_rgb()?;
                    foreground_color.0 = r;
                    foreground_color.1 = g;
                    foreground_color.2 = b;
                    save_foreground(foreground_color);
                }
                PidCommand::SetBackground => {
                    let (r, g, b) = color_sensor.get_rgb()?;
                    background_color.0 = r;
                    background_color.1 = g;
                    background_color.2 = b;
                    save_background(background_color);
                }
            }
        }

        let error = calc_error(color_sensor, foreground_color, background_color, network)?;

        integral = (integral + error * dt) * INTEGRAL_LIMITER;
        let derivative = (error - last_error) / dt;

        let output =
            CONST_PROPORTIONAL * error + CONST_INTEGRAL * integral + CONST_DERIVATIVE * derivative;

        if lost_line > 15 {
            println!("search line");
            integral = INTEGRAL_MAXIMUM * DRIVE_MULTIPLIER;
            history_error = 0.0;
            last_error = 0.0;
            lost_line = 0;

            while calc_error(color_sensor, foreground_color, background_color, network)?
                * DRIVE_MULTIPLIER
                > -0.5
            {
                driving_sender
                    .send(DrivingCommand::SetPid(
                        SPEED_SLOW * DRIVE_MULTIPLIER,
                        -SPEED_SLOW * DRIVE_MULTIPLIER,
                    ))
                    .unwrap();
            }

            drive_slow = 20;
            continue;
        }

        if lost_line > 0 {
            if error * DRIVE_MULTIPLIER > 0.5 {
                lost_line += 1;
            } else {
                lost_line = 0;
            }
        }

        if (history_error - error).abs() > 0.7 && error * DRIVE_MULTIPLIER > 0.5 && drive_slow == 0
        {
            lost_line += 1;
        }

        history_error = last_error;
        last_error = error;

        let mut speed = SPEED_NORMAL;
        if drive_slow > 0 {
            drive_slow -= 1;
            speed = SPEED_SLOW;
        } else if integral.abs() < 0.2 && derivative.abs() < 0.2 {
            speed = SPEED_FAST;
        } else if integral.abs() > 1.0 {
            speed = SPEED_SLOW;
        }

        //println!("e: {} | o: {} | p: {} | i: {} | d: {}", error, output, error * CONST_PROPORTIONAL, integral * CONST_INTEGRAL, derivative * CONST_DERIVATIVE);

        driving_sender
            .send(DrivingCommand::SetPid(
                speed + (COUNTERMEASURE * output),
                speed - (COUNTERMEASURE * output),
            ))
            .unwrap();
    }

    driving_sender
        .send(DrivingCommand::SetPid(0.0, 0.0))
        .unwrap();

    Ok(())
}

fn perform_pid(
    pid_receiver: &Receiver<PidCommand>,
    driving_sender: &Sender<DrivingCommand>,
    network: &Sender<NetworkCommand>,
) -> Ev3Result<()> {
    let mut color_sensor = ColorSensor::find().unwrap();
    color_sensor.set_mode_rgb_raw()?;

    let mut foreground_color = get_saved_foreground();
    let mut background_color = get_saved_background();

    //println!("Current color: {:?}", color_sensor.get_rgb());
    //println!("Foreground color: {:?}",foreground_color);
    //println!("Background color: {:?}", background_color);

    loop {
        if let Ok(command) = pid_receiver.recv_timeout(COLOR_TIMEOUT) {
            match command {
                PidCommand::Start => {
                    run(
                        pid_receiver,
                        driving_sender,
                        &mut color_sensor,
                        &mut foreground_color,
                        &mut background_color,
                        network,
                    )?;
                }
                PidCommand::Stop => {
                    // Do nothing
                }
                PidCommand::SetForeground => {
                    foreground_color = color_sensor.get_rgb()?;
                    save_foreground(&foreground_color);
                }
                PidCommand::SetBackground => {
                    background_color = color_sensor.get_rgb()?;
                    save_background(&background_color);
                }
            }
        }

        let sensor = color_sensor.get_rgb()?;
        network
            .send(NetworkCommand::Color(
                min(255, sensor.0 / 2) as u8,
                min(255, sensor.1 / 2) as u8,
                min(255, sensor.2 / 2) as u8,
            ))
            .unwrap();
    }
}

pub fn start(
    driving: Sender<DrivingCommand>,
    network: Sender<NetworkCommand>,
) -> Sender<PidCommand> {
    let (pid_sender, pid_receiver) = mpsc::channel();

    thread::Builder::new()
        .name("PID".to_string())
        .spawn(move || loop {
            match perform_pid(&pid_receiver, &driving, &network) {
                Ok(_) => {
                    break;
                }
                _ => {
                    println!("A pid error occurred, retry!");
                }
            }
        })
        .unwrap();

    pid_sender
}

fn get_saved_color(name: &str, default: i32) -> (i32, i32, i32) {
    let file = String::from(
        fs::read_to_string(name)
            .unwrap_or_else(|_| String::from("0;0;0"))
            .trim(),
    );
    let vec: Vec<&str> = file.split(';').collect::<Vec<&str>>();

    let r = vec[0].parse::<i32>().unwrap_or(default);
    let g = vec[1].parse::<i32>().unwrap_or(default);
    let b = vec[2].parse::<i32>().unwrap_or(default);

    (r, g, b)
}
fn save_color(name: &str, foreground: &(i32, i32, i32)) {
    let mut color = String::new();
    color.push_str(foreground.0.to_string().as_ref());
    color.push(';');
    color.push_str(foreground.1.to_string().as_ref());
    color.push(';');
    color.push_str(foreground.2.to_string().as_ref());

    fs::write(name, color).unwrap();
}

fn get_saved_foreground() -> (i32, i32, i32) {
    get_saved_color("foreground", 20)
}
fn save_foreground(color: &(i32, i32, i32)) {
    save_color("foreground", color)
}
fn get_saved_background() -> (i32, i32, i32) {
    get_saved_color("background", 200)
}
fn save_background(color: &(i32, i32, i32)) {
    save_color("background", color)
}

pub enum PidCommand {
    Start,
    Stop,
    SetForeground,
    SetBackground,
}
