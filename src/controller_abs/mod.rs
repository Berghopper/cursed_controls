use std::{
    ops::{Div, Sub},
    u64,
};

use num_traits::{Bounded, FromPrimitive, NumCast, ToPrimitive};

pub trait NormalizableNumber:
    Bounded + ToPrimitive + FromPrimitive + NumCast + Sub<Output = Self> + Div<Output = Self> + Copy
{
}
impl<T> NormalizableNumber for T where
    T: Bounded + ToPrimitive + FromPrimitive + NumCast + Sub<Output = T> + Div<Output = T> + Copy
{
}

// help, generics :(...
pub fn normalize<From, To>(
    from_value: From,
    from_min: Option<From>,
    from_max: Option<From>,
    to_min: Option<To>,
    to_max: Option<To>,
) -> To
where
    From: NormalizableNumber,
    To: NormalizableNumber,
{
    // Safely convert min and max values to f64
    let from_min_f64 = from_min
        .unwrap_or_else(|| From::min_value())
        .to_f64()
        .unwrap();
    let from_max_f64 = from_max
        .unwrap_or_else(|| From::max_value())
        .to_f64()
        .unwrap();

    let from_range_ = (from_min_f64 - from_max_f64).abs();

    let to_min_f64 = to_min.unwrap_or_else(|| To::min_value()).to_f64().unwrap();
    let to_max_f64 = to_max.unwrap_or_else(|| To::max_value()).to_f64().unwrap();
    let to_range_ = (to_max_f64 - to_min_f64).abs();

    // Normalize the from_value to the [0, 1] range as f64
    let normalized_value = (from_value.to_f64().unwrap() - from_min_f64) / from_range_;

    // Scale the normalized value to the target type's range
    let scaled_value = normalized_value * to_range_ + to_min_f64;

    // Convert the clamped value to the target type
    To::from_f64(scaled_value).unwrap_or_else(|| {
        if scaled_value < to_min_f64 {
            To::min_value()
        } else {
            To::max_value()
        }
    })
}

// Values in axis are all u64, most likely controllers will have smaller sizes, so more easily convertible.
pub struct Axis {
    pub value: u64,
    min: u64,
    max: u64,
    deadzones: Option<Vec<std::ops::Range<u64>>>,
}

impl Axis {
    pub fn new<T>(
        from_value: T,
        min: Option<T>,
        max: Option<T>,
        deadzones: Option<Vec<std::ops::Range<u64>>>,
    ) -> Axis
    where
        T: NormalizableNumber,
    {
        let min_val = normalize(
            min.unwrap_or_else(|| T::min_value()),
            Some(T::min_value()),
            Some(T::max_value()),
            Some(u64::MIN),
            Some(u64::MAX),
        );
        let max_val = normalize(
            max.unwrap_or_else(|| T::max_value()),
            Some(T::min_value()),
            Some(T::max_value()),
            Some(u64::MIN),
            Some(u64::MAX),
        );

        Axis {
            value: normalize(from_value, min, max, Some(min_val), Some(max_val)),
            min: min_val,
            max: max_val,
            deadzones: deadzones,
        }
    }

    pub fn set_values<T>(&mut self, from_value: T, min: Option<T>, max: Option<T>)
    where
        T: NormalizableNumber,
    {
        let min_val = normalize(
            min.unwrap_or_else(|| T::min_value()),
            Some(T::min_value()),
            Some(T::max_value()),
            Some(u64::MIN),
            Some(u64::MAX),
        );
        let max_val = normalize(
            max.unwrap_or_else(|| T::max_value()),
            Some(T::min_value()),
            Some(T::max_value()),
            Some(u64::MIN),
            Some(u64::MAX),
        );
        self.value = normalize(
            from_value,
            min,
            max,
            min.unwrap().to_u64(),
            max.unwrap().to_u64(),
        );
        self.min = min_val;
        self.max = max_val;
    }

    pub fn get_value(&mut self) -> &u64 {
        &self.value
    }

    pub fn get_min(&mut self) -> &u64 {
        &self.min
    }

    pub fn get_max(&mut self) -> &u64 {
        &self.max
    }

    pub fn set_deadzones(&mut self, deadzones: Option<Vec<std::ops::Range<u64>>>) {
        self.deadzones = deadzones;
    }

    pub fn get_deadzones(&mut self) -> &Option<Vec<std::ops::Range<u64>>> {
        return &self.deadzones;
    }

    pub fn make_deadzone<T>(
        &self,
        input: Vec<std::ops::Range<T>>,
        min: T,
        max: T,
    ) -> Vec<std::ops::Range<u64>>
    where
        T: NormalizableNumber,
    {
        input
            .into_iter()
            .map(|range| {
                let start_normalized = normalize::<T, u64>(
                    range.start,
                    Some(min),
                    Some(max),
                    Some(self.min),
                    Some(self.max),
                );
                let end_normalized = normalize::<T, u64>(
                    range.end,
                    Some(min),
                    Some(max),
                    Some(self.min),
                    Some(self.max),
                );
                std::ops::Range {
                    start: start_normalized,
                    end: end_normalized,
                }
            })
            .collect()
    }

    pub fn convert_into<T>(&self, use_deadzones: Option<bool>) -> T
    where
        T: NormalizableNumber,
    {
        // Normalization step, usually between two different Axis systems
        // Apply deadzones if needed
        if use_deadzones.unwrap_or(true) {
            if let Some(deadzones) = &self.deadzones {
                for deadzone in deadzones {
                    if deadzone.contains(&self.value) {
                        let norm_range =
                            (self.min.to_f64().unwrap() - self.max.to_f64().unwrap()).abs();
                        let normalized_ratio = (self.value.to_f64().unwrap()
                            - self.min.to_f64().unwrap())
                            / norm_range;
                        let deadzone_start_ratio = (deadzone.start.to_f64().unwrap()
                            - self.min.to_f64().unwrap())
                            / norm_range;
                        let deadzone_end_ratio = (deadzone.end.to_f64().unwrap()
                            - self.min.to_f64().unwrap())
                            / norm_range;

                        println!("dedrat: {}", normalized_ratio);
                        if (deadzone_start_ratio > 0.3 && deadzone_end_ratio < 0.7) {
                            // 'middle' deadzone?
                            let middle_value_f64 = self.min.to_f64().unwrap()
                                + (self.max.to_f64().unwrap() - self.min.to_f64().unwrap()) / 2.0;
                            println!("middl; {}", middle_value_f64);
                            return T::from_f64(middle_value_f64).unwrap();
                        } else if (normalized_ratio < 0.3) {
                            // Min
                            return T::min_value();
                        } else {
                            // Max
                            return T::max_value();
                        }
                    }
                }
            }
        }

        return normalize(self.value, Some(self.min), Some(self.max), None, None);
    }
}

#[derive(Clone)]
pub struct BitPackedButton {
    // Button with it's corresponding address
    name: Option<String>,
    pub value: bool,
    addr: u8,
}

impl BitPackedButton {
    pub fn new<N: Into<Option<String>>>(name: N, addr: u8) -> BitPackedButton {
        BitPackedButton {
            name: name.into(),
            value: false,
            addr,
        }
    }
}

pub struct BitPackedButtons {
    pub buttons: Vec<BitPackedButton>,
}

impl BitPackedButtons {
    pub fn get_by_name(self: &Self, name: &String) -> Option<&BitPackedButton> {
        self.buttons
            .iter()
            .find(|button| button.name.as_ref() == Some(name))
    }

    pub fn to_bytes_repr(self: &Self) -> u8 {
        let mut buttons_sorted = self.buttons.to_vec();
        buttons_sorted.sort_by_key(|button| button.addr);
        return buttons_sorted
            .iter()
            .map(|button| (button.value as u8) << button.addr)
            .fold(0, |acc, bit| acc | bit);
    }
}
