use futures_util::TryStreamExt;
use num_traits::cast::FromPrimitive;
use std::thread::sleep;
use std::time::Duration;
use tokio;
use tokio::time::MissedTickBehavior;
use xwiimote::events::{Event, Key};
use xwiimote::{Address, Channels, Device, Led, Monitor, Result};

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

/// The metrics that can be displayed in a [`LightsDisplay`].
#[derive(Debug, Copy, Clone)]
enum LightsMetric {
    /// Display the battery level.
    Battery,
    /// Display the connection strength level.
    Connection,
}

/// The set of lights in a Wii Remote, used as a display.
struct LightsDisplay<'d> {
    /// The device whose lights are being controlled.
    device: &'d Device,
    /// The metric to display.
    metric: LightsMetric,
    /// An interval that ticks whenever the display needs to be updated.
    interval: tokio::time::Interval,
}

impl<'d> LightsDisplay<'d> {
    /// Creates a wrapper for the display of a Wii Remote.
    pub fn new(device: &'d Device) -> Self {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        Self {
            device,
            // The connection strength is probably high immediately
            // after pairing; display the battery level by default.
            metric: LightsMetric::Battery,
            interval,
        }
    }

    /// Completes when the device display should be updated.
    pub async fn tick(&mut self) -> tokio::time::Instant {
        print!("tick");
        self.interval.tick().await
    }

    /// Updates the device lights according to the current metric.
    pub async fn update(&self) -> Result<()> {
        let level = match self.metric {
            LightsMetric::Battery => self.device.battery()?,
            LightsMetric::Connection => {
                // Technically RSSI is a measure of the received intensity
                // rather than connection quality. This is good enough for
                // the Wii Remote. The scale goes from -80 to 0, where 0
                // represents the greatest signal strength.
                let rssi = 0i8; // todo
                !((rssi as i16 * 100 / -80) as u8)
            }
        };

        // `level` is a value from 0 to 100 (inclusive).
        let last_ix = 1 + level / 30; // 1..=4
        for ix in 1..=4 {
            let light = Led::from_u8(ix).unwrap();
            self.device.set_led(light, ix <= last_ix)?;
        }
        Ok(())
    }

    /// Updates the displayed metric.
    pub async fn set_metric(&mut self, metric: LightsMetric) -> Result<()> {
        self.metric = metric;
        self.update().await
    }
}

async fn handle(device: &mut Device) -> Result<()> {
    let mut event_stream = device.events()?;
    let mut display = LightsDisplay::new(device);

    loop {
        // Wait for the next event, which is either an event
        // emitted by the device or a display update request.
        let maybe_event = tokio::select! {
            res = event_stream.try_next() => res?,
            // _ = display.tick() => {
            //     display.update().await?;
            //     continue;
            // },
            _ = tokio::time::sleep(Duration::from_millis(1)) => {
                display.update().await?;
                continue;
            },
        };

        let (event, _time) = match maybe_event {
            Some(event) => event,
            None => return Ok(()), // connection closed
        };

        if let Event::Key(key, state) = event {
            match key {
                Key::One => display.set_metric(LightsMetric::Battery).await,
                Key::Two => display.set_metric(LightsMetric::Connection).await,
                // If the remote key is mapped to a regular keyboard key,
                // send a press or release event via the `uinput` API.
                _ => {
                    println!("Key {:?} {:?}", key, state);
                    continue;
                }
            };
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Create a monitor to enumerate connected Wii Remotes
    let mut monitor = Monitor::enumerate().unwrap();
    let address = monitor.try_next().await.unwrap().unwrap();
    connect(&address).await?;

    Ok(())
}

// fn main() {
//     example_loop();
// }
