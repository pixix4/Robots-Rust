extern crate byteorder;
extern crate ev3dev_lang_rust;

use std::sync::mpsc;
use std::sync::mpsc::Sender;

use driving::DrivingCommand;
use pid::PidCommand;

mod driving;
mod network;
mod pid;
mod status;

fn main() {
    let (sender, receiver) = mpsc::channel();

    let network = network::start(Sender::clone(&sender));
    let driving = driving::start();
    let pid = pid::start(Sender::clone(&driving), Sender::clone(&network));

    //pid.send(PidCommand::Start).unwrap();

    loop {
        match receiver.recv().unwrap() {
            RobotCommand::SetTrack(left, right) => {
                driving.send(DrivingCommand::SetTrack(left, right)).unwrap();
            }
            RobotCommand::SetTrim(trim) => {
                driving.send(DrivingCommand::SetTrim(trim)).unwrap();
            }
            RobotCommand::Kick => {
                driving.send(DrivingCommand::Kick).unwrap();
            }
            RobotCommand::SetPid(is_pid) => {
                if is_pid {
                    pid.send(PidCommand::Start).unwrap();
                } else {
                    pid.send(PidCommand::Stop).unwrap();
                }
            }
            RobotCommand::SetForeground => {
                pid.send(PidCommand::SetForeground).unwrap();
            }
            RobotCommand::SetBackground => {
                pid.send(PidCommand::SetBackground).unwrap();
            }
        };
    }
}

pub enum RobotCommand {
    /// Message type: 10
    SetTrack(f32, f32),

    /// Message type: 12
    SetTrim(f32),

    /// Message type: 20
    Kick,

    /// Message type: 30
    SetPid(bool),

    /// Message type: 31
    SetForeground,

    /// Message type: 32
    SetBackground,
}
