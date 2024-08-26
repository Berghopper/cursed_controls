use std::thread::sleep;
// use xwiimote::{Device, Monitor};
// use futures_util::TryStreamExt;

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

fn main() {
    example_loop();
}
