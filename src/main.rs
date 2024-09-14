use controller_abs::Axis;
use futures_util::TryStreamExt;
use std::thread::sleep;
use std::time::Duration;
use tokio;
use xwiimote::events::{Event, Key, KeyState, NunchukKey};
use xwiimote::{Address, Channels, Device, Monitor, Result};

mod controller_abs;
mod controller_out;
use controller_out::x360::XboxControllerState;

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

#[allow(dead_code)]
fn example_loop() {
    print!("Starting 360 gadget...");

    let fd = init_360_gadget_c(true);
    let mut controller_state = XboxControllerState::new();
    loop {
        sleep(std::time::Duration::from_secs(1));
        controller_state.buttons.a.value = !controller_state.buttons.a.value;
        controller_state.buttons.b.value = !controller_state.buttons.b.value;
        controller_state.buttons.x.value = !controller_state.buttons.x.value;
        controller_state.buttons.y.value = !controller_state.buttons.y.value;
        // Set left joystick to north-east
        controller_state.left_joystick.x.value = 32760;
        controller_state.left_joystick.y.value = 32760;
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

fn map_wii_event_to_xbox_state(event: Event, xbox_state: &mut XboxControllerState) {
    // Example mapping for rocket league
    match event {
        Event::Key(key, key_state) => {
            let button_state = !matches!(key_state, KeyState::Up);
            match key {
                // Jump
                Key::A => xbox_state.buttons.a.value = button_state,
                Key::B => {
                    // Throttle
                    xbox_state.right_trigger.value = if button_state {
                        u64::max_value()
                    } else {
                        u64::min_value()
                    };
                }
                Key::Plus => xbox_state.buttons.start.value = button_state,
                Key::Minus => xbox_state.buttons.options.value = button_state,

                // boost
                Key::Down => xbox_state.buttons.b.value = button_state,

                // DPAD
                Key::Up => xbox_state.buttons.dpad_up.value = button_state,
                Key::Left => xbox_state.buttons.dpad_left.value = button_state,
                Key::Right => xbox_state.buttons.dpad_right.value = button_state,
                Key::Two => xbox_state.buttons.dpad_down.value = button_state,
                // Ball cam
                Key::One => xbox_state.buttons.y.value = button_state,
                _ => {}
            }
        }
        Event::NunchukKey(nunchuk_key, key_state) => {
            let button_state = !matches!(key_state, KeyState::Up);
            match nunchuk_key {
                NunchukKey::Z => {
                    // Brake
                    xbox_state.left_trigger.value = if button_state {
                        u64::max_value()
                    } else {
                        u64::min_value()
                    };
                }
                // Handbreak
                NunchukKey::C => xbox_state.buttons.x.value = button_state,
            }
        }
        Event::NunchukMove {
            x,
            y,
            x_acceleration: _,
            y_acceleration: _,
        } => {
            let mut nunchuck_x = Axis::new(x, Some(-128), Some(128), None);
            let mut nunchuck_y = Axis::new(y, Some(-128), Some(128), None);

            let deadzone_vec = vec![-8..8, -128..-70, 70..128];
            nunchuck_x.set_deadzones(Some(nunchuck_x.make_deadzone(
                deadzone_vec.to_owned(),
                -128,
                128,
            )));
            nunchuck_y.set_deadzones(Some(nunchuck_y.make_deadzone(
                deadzone_vec.to_owned(),
                -128,
                128,
            )));

            xbox_state.left_joystick.x.value = nunchuck_x.convert_into(Some(true));
            xbox_state.left_joystick.y.value = nunchuck_y.convert_into(Some(true));
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
            _ = tokio::time::sleep(Duration::from_millis(5)) => { // TODO: Make this a setting somehow
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
        // After sending state, sleep 1ms.
        tokio::time::sleep(Duration::from_micros(900)).await;
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
