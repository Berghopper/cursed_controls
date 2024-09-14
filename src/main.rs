use futures_util::TryStreamExt;
use std::thread::sleep;
use std::time::Duration;
use tokio;
use xwiimote::events::{Event, Key, KeyState, NunchukKey};
use xwiimote::{Address, Channels, Device, Monitor, Result};

// Declare externals
extern "C" {
    fn init_360_gadget(await_endpoint_availability: bool) -> i32;
    fn close_360_gadget(fd: i32);
    fn send_to_ep1(fd: i32, data: *const u8, len: usize) -> bool;
    // fn gadget_example();
}

fn init_360_gadget_c(await_endpoint_availability: bool) -> i32 {
    unsafe { init_360_gadget(await_endpoint_availability) }
}

#[allow(dead_code)]
fn close_360_gadget_c(fd: i32) {
    unsafe { close_360_gadget(fd) }
}

fn send_to_ep1_c(fd: i32, data: *const u8, len: usize) -> bool {
    unsafe { send_to_ep1(fd, data, len) }
}

struct XboxButtonState {
    a: bool,
    b: bool,
    x: bool,
    y: bool,
    lb: bool,
    rb: bool,
    l3: bool,
    r3: bool,
    start: bool,
    options: bool,
    dpad_up: bool,
    dpad_down: bool,
    dpad_left: bool,
    dpad_right: bool,
    xbox: bool,
}

impl XboxButtonState {
    fn new() -> XboxButtonState {
        XboxButtonState {
            a: false,
            b: false,
            x: false,
            y: false,
            lb: false,
            rb: false,
            l3: false,
            r3: false,
            start: false,
            options: false,
            dpad_up: false,
            dpad_down: false,
            dpad_left: false,
            dpad_right: false,
            xbox: false,
        }
    }

    fn get_control_byte_2(&self) -> u8 {
        (self.dpad_up as u8) << 0
            | (self.dpad_down as u8) << 1
            | (self.dpad_left as u8) << 2
            | (self.dpad_right as u8) << 3
            | (self.start as u8) << 4
            | (self.options as u8) << 5
            | (self.l3 as u8) << 6
            | (self.r3 as u8) << 7
    }

    fn get_control_byte_3(&self) -> u8 {
        (self.lb as u8) << 0
            | (self.rb as u8) << 1
            | (self.xbox as u8) << 2
            | (self.a as u8) << 4
            | (self.b as u8) << 5
            | (self.x as u8) << 6
            | (self.y as u8) << 7
    }
}

struct JoystickState {
    // LE values, 0x0000 is left, 0xFFFF is right
    x: i16,
    // LE values, 0x0000 is down, 0xFFFF is up
    y: i16,
}

struct XboxControllerState {
    buttons: XboxButtonState,
    left_trigger: u8,
    right_trigger: u8,
    left_joystick: JoystickState,  // byte 6 - 9
    right_joystick: JoystickState, // byte 10 - 13
}

impl XboxControllerState {
    fn new() -> XboxControllerState {
        XboxControllerState {
            buttons: XboxButtonState::new(),
            left_trigger: 0,
            right_trigger: 0,
            left_joystick: JoystickState { x: 0, y: 0 },
            right_joystick: JoystickState { x: 0, y: 0 },
        }
    }

    fn to_packet(&self) -> [u8; 20] {
        let mut packet = [0u8; 20];
        packet[0] = 0x00; // Report ID (0x00)
        packet[1] = 0x14; // Length (0x14)
        packet[2] = self.buttons.get_control_byte_2();
        packet[3] = self.buttons.get_control_byte_3();
        packet[4] = self.left_trigger;
        packet[5] = self.right_trigger;
        packet[6..8].copy_from_slice(&self.left_joystick.x.to_le_bytes());
        packet[8..10].copy_from_slice(&self.left_joystick.y.to_le_bytes());
        packet[10..12].copy_from_slice(&self.right_joystick.x.to_le_bytes());
        packet[12..14].copy_from_slice(&self.right_joystick.y.to_le_bytes());
        packet
    }
}

#[allow(dead_code)]
fn example_loop() {
    print!("Starting 360 gadget...");

    let fd = init_360_gadget_c(true);
    let mut controller_state = XboxControllerState::new();
    loop {
        sleep(std::time::Duration::from_secs(1));
        controller_state.buttons.a = !controller_state.buttons.a;
        controller_state.buttons.b = !controller_state.buttons.b;
        controller_state.buttons.x = !controller_state.buttons.x;
        controller_state.buttons.y = !controller_state.buttons.y;
        // Set left joystick to north-east
        controller_state.left_joystick.x = 32760;
        controller_state.left_joystick.y = 32760;
        let packet = controller_state.to_packet();
        send_to_ep1_c(fd, packet.as_ptr(), 20);
    }
    // close_360_gadget_c(fd);
}

// Wii Remote stuff
async fn connect(address: &Address) -> Result<()> {
    let mut device = Device::connect(address)?;
    let name = device.kind()?;

    device.open(Channels::CORE, true)?;
    device.open(Channels::NUNCHUK, true)?;
    println!("Device connected: {name}");

    handle(&mut device).await?;
    println!("Device disconnected: {name}");
    Ok(())
}

fn nunchuck_to_xbox_joystick(nunchuck_axis: i32) -> i16 {
    // Depending on the nunchuck, values range from about ~ -110 - 90 or -90 - 110
    // So the working range is -90 - 90, with a +- deadzone of 10 around 0.
    if nunchuck_axis >= -10 && nunchuck_axis <= 10 {
        return 0; // Deadzone mapping
    } else if nunchuck_axis < -10 {
        // Map from [-90, -10] to [-32768, 0]
        let mapped_value = ((nunchuck_axis + 10) as f32 / -80.0) * i16::MIN as f32;
        return mapped_value as i16;
    } else {
        // Map from [10, 90] to [0, 32767]
        let mapped_value = ((nunchuck_axis - 10) as f32 / 80.0) * i16::MAX as f32;
        return mapped_value as i16;
    }
}

fn map_wii_event_to_xbox_state(event: Event, xbox_state: &mut XboxControllerState) {
    // Example mapping for rocket league
    match event {
        Event::Key(key, key_state) => {
            let button_state = !matches!(key_state, KeyState::Up);
            match key {
                // Jump
                Key::A => xbox_state.buttons.a = button_state,
                Key::B => {
                    // Throttle
                    xbox_state.right_trigger = if button_state {
                        u8::max_value()
                    } else {
                        u8::min_value()
                    };
                }
                Key::Plus => xbox_state.buttons.start = button_state,
                Key::Minus => xbox_state.buttons.options = button_state,

                // boost
                Key::Down => xbox_state.buttons.b = button_state,

                // DPAD
                Key::Up => xbox_state.buttons.dpad_up = button_state,
                Key::Left => xbox_state.buttons.dpad_left = button_state,
                Key::Right => xbox_state.buttons.dpad_right = button_state,
                Key::Two => xbox_state.buttons.dpad_down = button_state,
                // Ball cam
                Key::One => xbox_state.buttons.y = button_state,
                _ => {}
            }
        }
        Event::NunchukKey(nunchuk_key, key_state) => {
            let button_state = !matches!(key_state, KeyState::Up);
            match nunchuk_key {
                NunchukKey::Z => {
                    // Brake
                    xbox_state.left_trigger = if button_state {
                        u8::max_value()
                    } else {
                        u8::min_value()
                    };
                }
                // Handbreak
                NunchukKey::C => xbox_state.buttons.x = button_state,
            }
        }
        Event::NunchukMove {
            x,
            y,
            x_acceleration: _,
            y_acceleration: _,
        } => {
            xbox_state.left_joystick.x = nunchuck_to_xbox_joystick(x);
            xbox_state.left_joystick.y = nunchuck_to_xbox_joystick(y);
        }
        _ => {}
    }
}

async fn handle(device: &mut Device) -> Result<()> {
    let mut event_stream = device.events()?;
    // let mut display = LightsDisplay::new(device);

    // Start xbox gadget
    let fd = init_360_gadget_c(true);
    let mut controller_state = XboxControllerState::new();

    let mut gadget_open = true;
    loop {
        // Wait for the next event, which is either an event
        // emitted by the device or a display update request.
        let maybe_event = tokio::select! {
            res = event_stream.try_next() => res?,
            _ = tokio::time::sleep(Duration::from_millis(1)) => {
                continue;
            },
        };

        let (event, _time) = match maybe_event {
            Some(event) => event,
            None => {
                // connection closed
                // close gadget
                close_360_gadget_c(fd);
                gadget_open = false;
                return Ok(());
            }
        };

        if !gadget_open {
            // Exit.
            break;
        }

        map_wii_event_to_xbox_state(event, &mut controller_state);
        // emit to gadget
        let success = send_to_ep1_c(fd, controller_state.to_packet().as_ptr(), 20);
        if !success {
            // Probably crashed?
            break;
        }
    }
    return Ok(());
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Create a monitor to enumerate connected Wii Remotes
    let mut monitor = Monitor::enumerate().unwrap();
    let address = monitor.try_next().await.unwrap().unwrap();
    connect(&address).await?;

    Ok(())
}
