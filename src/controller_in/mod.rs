use std::{collections::HashMap, time::Duration};

use futures::TryStreamExt;
use futures_util::StreamExt;
use xwiimote::{events::{Event, Key, KeyState}, Address, Channels, Device, Monitor};

use crate::controller_abs::{
        ControllerInput, ControllerMapping, Gamepad, InputType, OutputMapping
    };
use futures::executor::block_on;

// TODO: use actix?

struct XWiiInput {
    device: Device,
    gamepad: Gamepad,
    channels: Channels,
    mappings: Vec<ControllerMapping<Event>>
}

impl XWiiInput {
    pub fn new(address: &Address) -> XWiiInput {
        XWiiInput {
            device: Device::connect(address).unwrap(),
            gamepad: Gamepad::new(),
            // TODO: Make this into a ::new arg.
            channels: Channels::CORE | Channels::NUNCHUK,
            mappings: vec![]
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

    fn map_event(&mut self, event: Event, to_mapping: OutputMapping) {
        self.mappings.push(
            ControllerMapping {
                input: event,
                output: to_mapping.clone()
            }
        );   
    }

    async fn map_next_event(&mut self, eventType: InputType, to_mapping: OutputMapping) {
        // FIXME; Break after timeout or something similar.
        loop {
            match self.next_event().await {
                Ok(event) => {
                    match event {
                        Event::Key(key, key_state) => {
                            let button_down = !matches!(key_state, KeyState::Up);
                            if !button_down {
                                continue;
                            }
                            self.map_event(Event::Key(key, KeyState::Up), to_mapping);
                        }
                        _ => {
                            // not supported, ignore.
                            // TODO: break somehow
                        }
                    }

                },
                _ => {
                    // Do nothing.
                }
    
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
        let mut event_stream = self.device.events().unwrap();

        let maybe_event = tokio::select! {
            res = event_stream.try_next() => match res {
                Ok(event) => event,
                Err(_) => return Err("Error reading events.")
            },
            // TODO: Make this a setting somehow?
            _ = tokio::time::sleep(Duration::from_millis(5)) => {
                return Ok(false);
            },
        };

        let (event, _time) = match maybe_event {
            Some(event) => event,
            None => {
                return Ok(false);
            }
        };

        // TODO impl actual mappings;
        // map_wii_event_to_xbox_state(event, &mut controller_state);
        return Ok(true);
    }

     
}

// TODO implement connection and mappings
