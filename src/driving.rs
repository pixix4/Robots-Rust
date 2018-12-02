use std::io::Result;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use ev3dev_lang_rust::core::MotorPort;
use ev3dev_lang_rust::tacho_motor::{self, LargeMotor, MediumMotor, TachoMotor};

const MAX_SPEED: u8 = 100;
const PID_SPEED: f32 = 0.5;
const RECEIVE_TIMEOUT: Duration = Duration::from_millis(100);

use std::time::Duration;
use std::time::SystemTime;

fn perform_drive(driving_receiver: &Receiver<DrivingCommand>) -> Result<()> {
    let mut pid_left_speed: f32 = 0.0;
    let mut pid_right_speed: f32 = 0.0;

    let mut left_speed: f32 = 0.0;
    let mut right_speed: f32 = 0.0;

    let speed: f32 = MAX_SPEED as f32;

    let mut kick: Option<SystemTime> = None;

    let mut right_motor = LargeMotor::new(MotorPort::OutA).unwrap();
    let mut left_motor = LargeMotor::new(MotorPort::OutB).unwrap();
    let mut kicker = MediumMotor::new(MotorPort::OutC).unwrap();

    //Calibrate kicker
    kicker.set_stop_action(String::from(tacho_motor::STOP_ACTION_BRAKE))?;
    kicker.set_speed_sp(-100)?;
    kicker.run_timed(Some(2000))?;
    thread::sleep(Duration::from_millis(2000));
    kicker.stop()?;
    thread::sleep(Duration::from_millis(500));
    kicker.set_position(0)?;
    kicker.set_speed_sp(850)?;

    //Stop running motors
    left_motor.set_duty_cycle_sp(0)?;
    right_motor.set_duty_cycle_sp(0)?;
    left_motor.run_direct()?;
    right_motor.run_direct()?;

    /*
            devices.extra_motor.speed_sp = 850
            devices.extra_motor.position_sp = 150
            devices.extra_motor.run_to_abs_pos()
            devices.extra_motor.wait_while("running")
            devices.extra_motor.position_sp = 0
            devices.extra_motor.run_to_abs_pos()
    */

    loop {
        if let Ok(driving) = driving_receiver.recv_timeout(RECEIVE_TIMEOUT) {
            let mut drive_change = false;

            match driving {
                DrivingCommand::SetTrack(left, right) => {
                    left_speed = left;
                    right_speed = right;
                    drive_change = true;
                }
                DrivingCommand::SetPid(left, right) => {
                    pid_left_speed = left * PID_SPEED;
                    pid_right_speed = right * PID_SPEED;
                    drive_change = true;
                }
                DrivingCommand::SetTrim(_) => {}
                DrivingCommand::Kick => {
                    if kick.is_none() {
                        kick = Some(SystemTime::now());
                        kicker.run_to_abs_pos(Some(150))?;
                    }
                }
                DrivingCommand::Stop => {
                    return Ok(());
                }
            }

            if drive_change {
                let left = (pid_left_speed + left_speed).max(-1.0).min(1.0);
                let right = (pid_right_speed + right_speed).max(-1.0).min(1.0);

                left_motor.set_duty_cycle_sp((left * speed) as isize)?;
                right_motor.set_duty_cycle_sp((right * speed) as isize)?;
            }
        }

        if let Some(time) = kick {
            let duration = time.elapsed().unwrap();
            let elapsed = (duration.as_secs() as u32 * 1000) + duration.subsec_millis();
            if elapsed > 200 {
                kick = None;
                kicker.run_to_abs_pos(Some(0))?;
            }
        }
    }
}

pub fn start() -> Sender<DrivingCommand> {
    let (driving_sender, driving_receiver) = mpsc::channel();

    thread::Builder::new()
        .name("Driving".to_string())
        .spawn(move || loop {
            match perform_drive(&driving_receiver) {
                Ok(_) => {
                    break;
                }
                _ => {
                    println!("A drive error occurred, retry!");
                }
            }
        })
        .unwrap();

    return driving_sender;
}

pub enum DrivingCommand {
    SetTrack(f32, f32),
    SetPid(f32, f32),
    SetTrim(f32),
    Kick,
    Stop,
}
