use controller_abs::Axis;
use futures_util::TryStreamExt;
use std::thread::sleep;
use std::time::Duration;
use tokio;
use xwiimote::events::{Event, Key, KeyState, NunchukKey};
use xwiimote::{Address, Channels, Device, Monitor, Result};

#[allow(dead_code)]
mod controller_abs;
#[allow(dead_code)]
mod controller_out;
#[allow(dead_code)]
use controller_out::x360::XboxControllerState;

// Declare externals
extern "C" {
    fn init_360_gadget(await_endpoint_availability: bool, n_interfaces: i32) -> i32;
    fn close_360_gadget(fd: i32);
    fn send_to_ep(fd: i32, n: i32, data: *const u8, len: usize) -> bool;
}

fn init_360_gadget_c(await_endpoint_availability: bool, n_interfaces: i32) -> i32 {
    unsafe { init_360_gadget(await_endpoint_availability, n_interfaces) }
}

#[allow(dead_code)]
fn close_360_gadget_c(fd: i32) {
    unsafe { close_360_gadget(fd) }
}

fn send_to_ep_c(fd: i32, n: i32, data: *const u8, len: usize) -> bool {
    unsafe { send_to_ep(fd, n, data, len) }
}

#[allow(dead_code)]
fn example_loop() {
    print!("Starting 360 gadget...");

    let fd = init_360_gadget_c(true, 1);
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
        send_to_ep_c(fd, 1, packet.as_ptr(), 20);
    }
    // close_360_gadget_c(fd);
}

// Wii Remote stuff
async fn connect(addresses: Vec<Address>) -> Result<()> {
    let mut devices = vec![];
    for addr in &addresses {
        let mut device = Device::connect(addr)?;
        let name = device.kind()?;

        device.open(Channels::CORE, true)?;
        device.open(Channels::NUNCHUK, true)?;
        println!("Device connected: {name}");
        devices.push(device);
    }

    handle(devices).await?;
    // println!("Device disconnected: {name}");
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
            // Specific limits to my controller
            // TODO; make cli that can set this..?
            let x_min = -88;
            let x_max = 110;
            let y_min = -102;
            let y_max = 94;
            let mut nunchuck_x = Axis::new(x, x_min, x_max);
            let mut nunchuck_y = Axis::new(y, y_min, y_max);

            let deadzone_vec_x = vec![-10..10];
            let deadzone_vec_y = vec![-10..10];

            nunchuck_x.set_deadzones(nunchuck_x.make_deadzone(
                deadzone_vec_x.to_owned(),
                x_min,
                x_max,
            ));
            nunchuck_y.set_deadzones(nunchuck_y.make_deadzone(
                deadzone_vec_y.to_owned(),
                y_min,
                y_max,
            ));

            xbox_state.left_joystick.x.value = nunchuck_x.convert_into(true);
            xbox_state.left_joystick.y.value = nunchuck_y.convert_into(true);
        }
        _ => {}
    }
}

async fn handle(devices: Vec<Device>) -> Result<()> {
    // Start xbox gadget
    // 1 controller
    let fd = init_360_gadget_c(true, devices.len().try_into().unwrap());
    // Wait 1 sec because C lib spam.
    tokio::time::sleep(Duration::from_secs(1)).await;

    let mut controller_states: Vec<XboxControllerState> = vec![];

    for _i in 0..devices.len() {
        controller_states.push(XboxControllerState::new());
    }
    loop {
        for i in 0..devices.len() {
            let device = &devices[i];
            let controller_state = &mut controller_states[i];
            let mut event_stream = device.events()?;

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
                    // close gadget and exit
                    close_360_gadget_c(fd);
                    break;
                }
            };

            map_wii_event_to_xbox_state(event, controller_state);
            // After sending state, sleep 1ms.
            tokio::time::sleep(Duration::from_micros(900)).await;
            // emit to gadget
            let success = send_to_ep_c(
                fd,
                i.try_into().unwrap(),
                controller_state.to_packet().as_ptr(),
                20,
            );
            if !success {
                // Probably crashed?
                break;
            }
        }
    }
    return Ok(());
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Create a monitor to enumerate connected Wii Remotes
    let mut monitor = Monitor::enumerate().unwrap();
    let mut addresses = Vec::new();
    loop {
        let opt_addr = monitor.try_next().await.unwrap();
        if opt_addr.is_none() {
            break;
        } else {
            addresses.push(opt_addr.unwrap());
        }
    }
    println!("{}", addresses.len());

    connect(addresses).await?;
    Ok(())
}
