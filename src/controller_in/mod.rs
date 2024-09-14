use std::{time::Duration};

use futures::TryStreamExt;
use futures_util::StreamExt;
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
}

impl XWiiInput {
    pub fn new(address: &Address) -> XWiiInput {
        XWiiInput {
            device: Device::connect(address).unwrap(),
            gamepad: Gamepad::new(),
            // TODO: Make this into a ::new arg.
            channels: Channels::CORE | Channels::NUNCHUK,
            mappings: vec![],
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
                            output_value = output_axis.get_min().clone();
                        } else {
                            output_value = output_axis.get_max().clone();
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
                    let from_min = -128;
                    let from_max = 128;

                    let mut nunchuck_x = Axis::new(x, from_min, from_max);
                    let mut nunchuck_y = Axis::new(y, from_min, from_max);

                    // Specific deadzone to my controller
                    // TODO; make cli that can set these ranges. / dynamically change it.
                    let deadzone_vec_x = vec![-7..7, from_min..-87, 109..from_max];
                    let deadzone_vec_y = vec![-7..7, from_min..-101, 93..from_max];

                    nunchuck_x.set_deadzones(nunchuck_x.make_deadzone(
                        deadzone_vec_x.to_owned(),
                        from_min,
                        from_max,
                    ));
                    nunchuck_y.set_deadzones(nunchuck_y.make_deadzone(
                        deadzone_vec_y.to_owned(),
                        from_min,
                        from_max,
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

        // TODO impl actual mappings;
        self.map_event_to_gamepad(event);
        // map_wii_event_to_xbox_state(event, &mut controller_state);
        return Ok(true);
    }
}

// TODO implement connection and mappings
