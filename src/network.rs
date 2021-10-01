use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use ev3dev_lang_rust::Ev3Result;
use status::ConnectionState;
use status::Status;
use std::io::Cursor;
use std::io::Read;
use std::net::SocketAddr;
use std::net::UdpSocket;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::Duration;
use std::time::SystemTime;
use RobotCommand;

const DISCOVERY_PORT: u16 = 7500;
const PING_TIMEOUT: Duration = Duration::from_millis(100);
const STOP_TIMEOUT: u32 = 300;
const DISCONNECT_TIMEOUT: u32 = 5000;
const BUFFER_SIZE: usize = 64;

/// Start an UDP discovery on given port to find the server's socket address.
fn get_server_address(discover_port: u16) -> Ev3Result<SocketAddr> {
    println!("Start discovery on port {}.", discover_port);

    // Listen to whole network on a random port
    let bind_address = SocketAddr::from(([0, 0, 0, 0], 0));

    // Broadcast to all devices on port 'discovery_port'
    let broadcast_address = SocketAddr::from(([255, 255, 255, 255], discover_port));

    let socket = UdpSocket::bind(bind_address)?;
    // Allow broadcasting
    socket.set_broadcast(true)?;
    // Timeout receive after 5 seconds
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;

    loop {
        let mut send_buffer = [0; 4];
        send_buffer[3] = 1;
        // Send empty discovery packet
        socket.send_to(&send_buffer, broadcast_address)?;

        let mut receive_buffer = [0; 4];
        // Receive server port

        // Check for success
        if let Ok((_, socket)) = socket.recv_from(&mut receive_buffer) {
            let server_ip = socket.ip();

            // Create byte array reader
            let mut rdr = Cursor::new(receive_buffer);
            rdr.set_position(2);
            let server_port = rdr.read_u16::<BigEndian>()?;

            let server_address = SocketAddr::from((server_ip, server_port));

            println!("Found server at: {:?}.", server_address);
            return Ok(server_address);
        }
    }
}

fn parse_message_v1(
    cursor: &mut Cursor<&[u8]>,
    robot_sender: &Sender<RobotCommand>,
    status: &mut Status,
) -> Ev3Result<()> {
    let message_type = cursor.read_u8()?;
    match message_type {
        0 => { // Pong
        }
        10 => {
            // SetTrack
            let left = cursor.read_f32::<BigEndian>()?;
            let right = cursor.read_f32::<BigEndian>()?;
            let _ = robot_sender
                .send(RobotCommand::SetTrack(left, right))
                .unwrap();
        }
        12 => {
            // SetTrim
            let trim = cursor.read_f32::<BigEndian>()?;
            let _ = robot_sender.send(RobotCommand::SetTrim(trim)).unwrap();
        }
        20 => {
            // Kick
            let _ = robot_sender.send(RobotCommand::Kick).unwrap();
        }
        30 => {
            // SetPid
            let pid = cursor.read_u8()?;
            let _ = robot_sender.send(RobotCommand::SetPid(pid != 0)).unwrap();
        }
        31 => {
            // SetForeground
            let _ = robot_sender.send(RobotCommand::SetForeground).unwrap();
        }
        32 => {
            // SetBackground
            let _ = robot_sender.send(RobotCommand::SetBackground).unwrap();
        }
        40 => {
            // SetName
            let mut name = String::new();
            cursor.read_to_string(&mut name)?;
            status.set_name(name)
        }
        41 => {
            // SetLedColor
            let mut color = String::new();
            cursor.read_to_string(&mut color)?;
            status.set_color(color)
        }
        _ => {
            // Nothing to do
        }
    }
    Ok(())
}

fn send(
    socket: &UdpSocket,
    target: &SocketAddr,
    message_version: u8,
    message_type: u8,
    message_content: Vec<u8>,
) -> Ev3Result<usize> {
    let mut bytes = vec![message_version, message_type];
    bytes.extend(message_content);

    Ok(socket.send_to(bytes.as_ref(), target)?)
}

#[allow(clippy::single_match)]
fn perform_networking(
    robot_sender: &Sender<RobotCommand>,
    stop_receiver: &Receiver<NetworkCommand>,
) -> Ev3Result<()> {
    let mut status = Status::new().unwrap();

    // Get server address
    let server_address = get_server_address(DISCOVERY_PORT)?;
    status.set_connection_state(ConnectionState::Connecting);

    // Connect to server
    let bind_address = SocketAddr::from(([0, 0, 0, 0], 0));
    let socket = UdpSocket::bind(bind_address)?;
    socket.set_read_timeout(Some(PING_TIMEOUT))?;

    let mut receive_buffer = [0; BUFFER_SIZE];
    socket.send_to(&[0; 4], &server_address)?;

    let mut last_pong = SystemTime::now();
    status.set_connection_state(ConnectionState::Connected);

    send(
        &socket,
        &server_address,
        1,
        1,
        status.get_version().into_bytes(),
    )?;
    send(
        &socket,
        &server_address,
        1,
        2,
        status.get_name().into_bytes(),
    )?;
    send(
        &socket,
        &server_address,
        1,
        3,
        status.get_color().into_bytes(),
    )?;
    send(
        &socket,
        &server_address,
        1,
        4,
        status.get_available_colors().join(";").into_bytes(),
    )?;

    let mut stopped = false;

    loop {
        // Receive command
        let message = socket.recv_from(&mut receive_buffer);

        match message {
            Ok((size, _)) => {
                last_pong = SystemTime::now();

                if stopped {
                    stopped = false;
                    status.set_connection_state(ConnectionState::Connected);
                }

                // Generate reader
                let mut cursor = Cursor::new(&receive_buffer[..size]);

                let message_version = cursor.read_u8()?;

                match message_version {
                    1 => {
                        let _ = parse_message_v1(&mut cursor, robot_sender, &mut status);
                    }
                    _ => {}
                }
            }
            Err(e) => {
                let duration = last_pong.elapsed().unwrap();
                let elapsed = (duration.as_secs() as u32 * 1000) + duration.subsec_millis();

                if elapsed > DISCONNECT_TIMEOUT {
                    status.set_connection_state(ConnectionState::Disconnected);
                    return Err(e.into());
                } else {
                    if elapsed > STOP_TIMEOUT {
                        robot_sender.send(RobotCommand::SetTrack(0.0, 0.0)).unwrap();
                        status.set_connection_state(ConnectionState::Reconnecting);
                    }
                    socket.send_to(&[0; 4], &server_address)?;

                    stopped = true;
                }
            }
        }

        if let Ok(command) = stop_receiver.try_recv() {
            match command {
                NetworkCommand::Color(r, g, b) => {
                    send(&socket, &server_address, 1, 5, vec![r, g, b])?;

                    let mut wtr = vec![];
                    wtr.write_f32::<BigEndian>(status.get_power()).unwrap();
                    send(&socket, &server_address, 1, 6, wtr)?;
                }
                NetworkCommand::Stop => {
                    return Ok(());
                }
            }
        }
    }
}

pub fn start(robot_sender: Sender<RobotCommand>) -> Sender<NetworkCommand> {
    let (stop_sender, stop_receiver) = mpsc::channel();

    thread::Builder::new()
        .name("Network".to_string())
        .spawn(move || loop {
            match perform_networking(&robot_sender, &stop_receiver) {
                Ok(_) => {
                    break;
                }
                Err(e) => {
                    println!("A network error occurred, retry! {:?}", e);
                }
            }
        })
        .unwrap();

    stop_sender
}

#[allow(dead_code)]
pub enum NetworkCommand {
    Color(u8, u8, u8),
    Stop,
}
