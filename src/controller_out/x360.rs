use crate::controller_abs::{Axis, BitPackedButton, BitPackedButtons};
use std::{u8, vec};

pub struct XboxButtonState {
    pub a: BitPackedButton,
    pub b: BitPackedButton,
    pub x: BitPackedButton,
    pub y: BitPackedButton,
    pub lb: BitPackedButton,
    pub rb: BitPackedButton,
    pub l3: BitPackedButton,
    pub r3: BitPackedButton,
    pub start: BitPackedButton,
    pub options: BitPackedButton,
    pub dpad_up: BitPackedButton,
    pub dpad_down: BitPackedButton,
    pub dpad_left: BitPackedButton,
    pub dpad_right: BitPackedButton,
    pub xbox: BitPackedButton,
}

impl XboxButtonState {
    pub fn new() -> XboxButtonState {
        XboxButtonState {
            a: BitPackedButton::new("A".to_string(), 0x04),
            b: BitPackedButton::new("B".to_string(), 0x05),
            x: BitPackedButton::new("X".to_string(), 0x06),
            y: BitPackedButton::new("Y".to_string(), 0x07),
            lb: BitPackedButton::new("LB".to_string(), 0x00),
            rb: BitPackedButton::new("RB".to_string(), 0x01),
            // Joystick buttons
            l3: BitPackedButton::new("L3".to_string(), 0x06),
            r3: BitPackedButton::new("R3".to_string(), 0x07),

            start: BitPackedButton::new("START".to_string(), 0x04),
            options: BitPackedButton::new("OPTIONS".to_string(), 0x05),
            xbox: BitPackedButton::new("XBOX".to_string(), 0x02),

            // Dpad
            dpad_up: BitPackedButton::new("DPAD_UP".to_string(), 0x00),
            dpad_down: BitPackedButton::new("DPAD_DOWN".to_string(), 0x01),
            dpad_left: BitPackedButton::new("DPAD_LEFT".to_string(), 0x02),
            dpad_right: BitPackedButton::new("DPAD_RIGHT".to_string(), 0x03),
        }
    }

    pub fn get_control_byte_2(&self) -> u8 {
        BitPackedButtons {
            buttons: vec![
                self.dpad_up.clone(),
                self.dpad_down.clone(),
                self.dpad_left.clone(),
                self.dpad_right.clone(),
                self.start.clone(),
                self.options.clone(),
                self.l3.clone(),
                self.r3.clone(),
            ],
        }
        .to_bytes_repr()
    }

    pub fn get_control_byte_3(&self) -> u8 {
        BitPackedButtons {
            buttons: vec![
                self.lb.clone(),
                self.rb.clone(),
                self.xbox.clone(),
                self.a.clone(),
                self.b.clone(),
                self.x.clone(),
                self.y.clone(),
            ],
        }
        .to_bytes_repr()
    }
}

pub struct JoystickState {
    // LE values, 0x0000 is left, 0xFFFF is right
    pub x: Axis,
    // LE values, 0x0000 is down, 0xFFFF is up
    pub y: Axis,
}

pub struct XboxControllerState {
    pub buttons: XboxButtonState,
    pub left_trigger: Axis,
    pub right_trigger: Axis,
    pub left_joystick: JoystickState,  // byte 6 - 9
    pub right_joystick: JoystickState, // byte 10 - 13
}

impl XboxControllerState {
    pub fn new() -> XboxControllerState {
        XboxControllerState {
            buttons: XboxButtonState::new(),
            left_trigger: Axis::new(u8::MIN, Some(u8::MIN), Some(u8::MAX), None),
            right_trigger: Axis::new(u8::MIN, Some(u8::MIN), Some(u8::MAX), None),
            left_joystick: JoystickState {
                x: Axis::new(0, Some(i16::MIN), Some(i16::MAX), None),
                y: Axis::new(0, Some(i16::MIN), Some(i16::MAX), None),
            },
            right_joystick: JoystickState {
                x: Axis::new(0, Some(i16::MIN), Some(i16::MAX), None),
                y: Axis::new(0, Some(i16::MIN), Some(i16::MAX), None),
            },
        }
    }

    pub fn to_packet(&self) -> [u8; 20] {
        let mut packet = [0u8; 20];
        packet[0] = 0x00; // Report ID (0x00)
        packet[1] = 0x14; // Length (0x14)
        packet[2] = self.buttons.get_control_byte_2();
        packet[3] = self.buttons.get_control_byte_3();
        packet[4] = self.left_trigger.convert_into(Some(false));
        packet[5] = self.right_trigger.convert_into(Some(false));
        packet[6..8].copy_from_slice(
            &self
                .left_joystick
                .x
                .convert_into::<i16>(Some(false))
                .to_le_bytes(),
        );
        packet[8..10].copy_from_slice(
            &self
                .left_joystick
                .y
                .convert_into::<i16>(Some(false))
                .to_le_bytes(),
        );
        packet[10..12].copy_from_slice(
            &self
                .right_joystick
                .x
                .convert_into::<i16>(Some(false))
                .to_le_bytes(),
        );
        packet[12..14].copy_from_slice(
            &self
                .right_joystick
                .y
                .convert_into::<i16>(Some(false))
                .to_le_bytes(),
        );
        packet
    }
}
