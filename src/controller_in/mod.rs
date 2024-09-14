use std::time::Duration;

use futures::TryStreamExt;
use futures_util::StreamExt;
use num_traits::ToPrimitive;
use xwiimote::{
    events::{Event, KeyState},
    Address, Channels, Device, Monitor,
};

use crate::controller_abs::{
    Axis, ControllerInput, ControllerMapping, Gamepad, GamepadAxis, OutputMapping,
};
use futures::executor::block_on;

// TODO: use actix?

struct XWiiEvent(Event);

impl XWiiEvent {
    // Constructor to wrap an Event into MyEvent
    fn new(event: xwiimote::events::Event) -> Self {
        XWiiEvent(event)
    }
}

impl PartialEq for XWiiEvent {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Event::Key(key1, _), Event::Key(key2, _)) => {
                std::mem::discriminant(key1) == std::mem::discriminant(key2)
            }
            (Event::NunchukKey(key1, _), Event::NunchukKey(key2, _)) => {
                std::mem::discriminant(key1) == std::mem::discriminant(key2)
            }
            (Event::NunchukMove { .. }, Event::NunchukMove { .. }) => true,
            // FIXME: Add others...
            _ => false,
        }
    }
}

pub struct XWiiInput {
    device: Device,
    gamepad: Gamepad,
    channels: Channels,
    mappings: Vec<ControllerMapping<Event>>,
    nunchuck_x_min: i32,
    nunchuck_x_max: i32,
    nunchuck_y_min: i32,
    nunchuck_y_max: i32,
    deadzone_percentage: f64,
}

impl XWiiInput {
    pub fn new(address: &Address) -> XWiiInput {
        XWiiInput {
            device: Device::connect(address).unwrap(),
            gamepad: Gamepad::new(),
            // TODO: Make this into a ::new arg.
            channels: Channels::CORE | Channels::NUNCHUK,
            mappings: vec![],
            nunchuck_x_min: 0,
            nunchuck_x_max: 0,
            nunchuck_y_min: 0,
            nunchuck_y_max: 0,
            deadzone_percentage: 0.05, // 5%
        }
    }

    async fn next_event(&mut self) -> Result<Event, &'static str> {
        let mut event_stream = self.device.events().unwrap();

        let maybe_event = tokio::select! {
            res = event_stream.try_next() => match res {
                Ok(event) => event,
                Err(_) => return Err("Error reading events")
            },
            _ = tokio::time::sleep(Duration::from_millis(5)) => {
                return Err("Error, no event");
            },
        };

        let (event, _time) = match maybe_event {
            Some(event) => event,
            None => {
                return Err("Erorr, no event");
            }
        };
        return Ok(event);
    }

    pub fn map_event(&mut self, event: Event, to_mapping: OutputMapping) {
        self.mappings.push(ControllerMapping {
            input: event,
            output: to_mapping.clone(),
        });
    }

    fn map_event_to_gamepad(&mut self, event: Event) {
        macro_rules! button_to_gamepad {
            ($self:expr, $controller_mapping_output:expr, $key_state:expr) => {
                let button_down = !matches!($key_state, KeyState::Up);
                match ($controller_mapping_output) {
                    OutputMapping::Axis(gamepad_axis) => {
                        let output_axis = $self.gamepad.get_axis_ref(gamepad_axis.to_owned());
                        let output_value;
                        if button_down {
                            output_value = output_axis.get_max().clone();
                        } else {
                            output_value = output_axis.get_min().clone();
                        };
                        output_axis.value = output_value;
                    }
                    OutputMapping::Button(gamepad_button) => {
                        self.gamepad
                            .set_button(gamepad_button.to_owned(), button_down);
                    }
                }
            };
        }

        for controller_mapping in &self.mappings {
            if XWiiEvent::new(controller_mapping.input) != XWiiEvent::new(event) {
                continue;
            }
            // If we have found our input key, we still need to do some basic matching to ensure correct mapping.
            // E.g. button -> Axis is a little weird.
            match event {
                Event::Key(_key, key_state) => {
                    button_to_gamepad!(self, &controller_mapping.output, key_state);
                }
                Event::NunchukKey(_key, key_state) => {
                    button_to_gamepad!(self, &controller_mapping.output, key_state);
                }
                Event::NunchukMove {
                    x,
                    y,
                    x_acceleration: _,
                    y_acceleration: _,
                } => {
                    // println!("nunchuck X {}", x);
                    // println!("nunchuck Y {}", y);

                    if x < self.nunchuck_x_min {
                        self.nunchuck_x_min = x;
                    }
                    if x > self.nunchuck_x_max {
                        self.nunchuck_x_max = x;
                    }

                    if y < self.nunchuck_y_min {
                        self.nunchuck_y_min = y;
                    }
                    if y > self.nunchuck_y_max {
                        self.nunchuck_y_max = y;
                    }

                    let mut nunchuck_x = Axis::new(x, self.nunchuck_x_min, self.nunchuck_x_max);
                    // println!("nunchuck X {}", nunchuck_x.get_normalized_value());
                    let mut nunchuck_y = Axis::new(y, self.nunchuck_y_min, self.nunchuck_y_max);
                    // println!("nunchuck Y {}", nunchuck_y.get_normalized_value());

                    let deadzone_range_x = (self.deadzone_percentage
                        * (self.nunchuck_x_min - self.nunchuck_x_max)
                            .abs()
                            .to_f64()
                            .unwrap())
                    .to_i32()
                    .unwrap();
                    let deadzone_range_y = (self.deadzone_percentage
                        * (self.nunchuck_y_min - self.nunchuck_y_max)
                            .abs()
                            .to_f64()
                            .unwrap())
                    .to_i32()
                    .unwrap();

                    nunchuck_x.set_deadzones(nunchuck_x.make_deadzone(
                        vec![-deadzone_range_x..deadzone_range_x].to_owned(),
                        self.nunchuck_x_min,
                        self.nunchuck_x_max,
                    ));
                    nunchuck_y.set_deadzones(nunchuck_y.make_deadzone(
                        vec![-deadzone_range_y..deadzone_range_y].to_owned(),
                        self.nunchuck_y_min,
                        self.nunchuck_y_max,
                    ));

                    match &controller_mapping.output {
                        OutputMapping::Axis(gamepad_axis) => {
                            let output_axis = self.gamepad.get_axis_ref(gamepad_axis.to_owned());
                            match gamepad_axis {
                                GamepadAxis::LeftJoystickX | GamepadAxis::RightJoystickX => {
                                    output_axis.value = nunchuck_x.convert_into(true)
                                }
                                GamepadAxis::LeftJoystickY | GamepadAxis::RightJoystickY => {
                                    output_axis.value = nunchuck_y.convert_into(true)
                                }
                                _ => {
                                    // Triggers?... could maybe?
                                }
                            }
                        }
                        OutputMapping::Button(_gamepad_button) => {
                            // not sure yet...
                        }
                    }
                }

                _ => {}
            }
        }
    }
}

impl ControllerInput for XWiiInput {
    type ControllerType = XWiiInput;

    fn to_gamepad<'a>(&'a mut self) -> &'a Gamepad {
        return &self.gamepad;
    }

    fn discover_all() -> Vec<Self::ControllerType> {
        let monitor = Monitor::enumerate().unwrap();

        let addresses: Vec<_> = block_on(async { monitor.collect().await });

        let mut inps: Vec<Self::ControllerType> = vec![];
        for address in addresses {
            inps.push(Self::ControllerType::new(&address.unwrap()));
        }

        return inps;
    }

    fn prep_for_input_events(&mut self) {
        // TODO: better decice handling with disconnects etc.
        self.device
            .open(Channels::from_bits(self.channels.bits()).unwrap(), true)
            .unwrap();
        println!("XWiiInput connected: {}", self.device.kind().unwrap());
    }

    async fn get_next_inputs(&mut self) -> Result<bool, &'static str> {
        let maybe_event = {
            let event_stream = &mut self.device.events().unwrap();
            tokio::select! {
                res = event_stream.try_next() => match res {
                    Ok(event) => event,
                    Err(_) => return Err("Error reading events.")
                },
                // TODO: Make this a setting somehow?
                _ = tokio::time::sleep(Duration::from_millis(5)) => {
                    return Ok(false);
                },
            }
        };

        let (event, _time) = match maybe_event {
            Some(event) => event,
            None => {
                return Ok(false);
            }
        };

        self.map_event_to_gamepad(event);
        return Ok(true);
    }
}
