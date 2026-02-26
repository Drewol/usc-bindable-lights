use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
};

use itertools::Itertools;

use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

#[derive(Debug, Clone, Copy, Default)]
pub struct Rgb<T> {
    pub r: T,
    pub g: T,
    pub b: T,
}

#[derive(
    Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize, strum::EnumIter, Default,
)]
pub enum Channel {
    #[default]
    R,
    G,
    B,
}

#[derive(
    Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize, strum::EnumIter, Default,
)]
pub enum BtLane {
    #[default]
    A,
    B,
    C,
    D,
}

impl Display for BtLane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BtLane::A => f.write_str("A"),
            BtLane::B => f.write_str("B"),
            BtLane::C => f.write_str("C"),
            BtLane::D => f.write_str("D"),
        }
    }
}

#[derive(
    Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize, strum::EnumIter, Default,
)]
pub enum Side {
    #[default]
    Left,
    Right,
}

impl Display for Side {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Side::Left => f.write_str("Left"),
            Side::Right => f.write_str("Right"),
        }
    }
}

impl Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Channel::R => f.write_str("Red"),

            Channel::G => f.write_str("Green"),

            Channel::B => f.write_str("Blue"),
        }
    }
}

impl Channel {
    fn get<T>(&self, rgb: Rgb<T>) -> T {
        match self {
            Channel::R => rgb.r,

            Channel::G => rgb.g,

            Channel::B => rgb.b,
        }
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize, strum::EnumIter)]

pub enum LightingTarget {
    Start,

    Bt(BtLane),

    Fx(Side),

    Top(Side, Channel),

    Middle(Side, Channel),

    Bottom(Side, Channel),
}

#[allow(unused)]
impl LightingTarget {
    pub fn iter() -> impl Iterator<Item = Self> {
        // We can avoid allocating vectors by boxing iterators, but wont be called in a hot path anyway
        <Self as strum::IntoEnumIterator>::iter().flat_map(|x| match x {
            LightingTarget::Start => vec![Self::Start],
            LightingTarget::Bt(..) => BtLane::iter().map(Self::Bt).collect_vec(),
            LightingTarget::Fx(..) => Side::iter().map(Self::Fx).collect_vec(),
            LightingTarget::Top(..) => Side::iter()
                .flat_map(|s| Channel::iter().map(move |c| (s, c)))
                .map(|(s, c)| Self::Top(s, c))
                .collect_vec(),
            LightingTarget::Middle(..) => Side::iter()
                .flat_map(|s| Channel::iter().map(move |c| (s, c)))
                .map(|(s, c)| Self::Middle(s, c))
                .collect_vec(),
            LightingTarget::Bottom(..) => Side::iter()
                .flat_map(|s| Channel::iter().map(move |c| (s, c)))
                .map(|(s, c)| Self::Bottom(s, c))
                .collect_vec(),
        })
    }
}

impl Display for LightingTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LightingTarget::Start => f.write_str("Start"),

            LightingTarget::Bt(bt_lane) => f.write_fmt(format_args!("BT {bt_lane}")),

            LightingTarget::Fx(side) => f.write_fmt(format_args!("FX {side}")),

            LightingTarget::Top(side, channel) => f.write_fmt(format_args!("Top {side} {channel}")),

            LightingTarget::Middle(side, channel) => {
                f.write_fmt(format_args!("Middle {side} {channel}"))
            }

            LightingTarget::Bottom(side, channel) => {
                f.write_fmt(format_args!("Bottom {side} {channel}"))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]

pub struct MappedTarget {
    pub report_id: u32,

    pub start_bit: u32,

    pub target: LightingTarget,
}

pub type LightingDeviceMap = Vec<MappedTarget>;

pub type LightingMap = HashMap<String, LightingDeviceMap>;

pub fn lighting_worker(rx: std::sync::mpsc::Receiver<Option<LightingData>>) {
    let mut config = crate::config::load();

    let Ok(api) = hidlights::HidLights::new() else {
        return;
    };

    let mut devices = api
        .devices()
        .into_iter()
        .filter_map(|dev| {
            config
                .remove(&dev.path().to_string_lossy().into_owned())
                .map(|conf| (dev, conf))
        })
        .filter_map(|(dev, conf)| dev.open().ok().map(|dev| (dev, conf)))
        .filter_map(|(dev, conf)| dev.reports().ok().map(|x| (dev, conf, x)))
        .map(|(dev, conf, reports)| {
            (
                dev,
                conf.into_iter()
                    .map(|x| ((x.report_id, x.start_bit), x.target))
                    .collect::<HashMap<_, _>>(),
                reports
                    .into_iter()
                    .map(|r| (r.id(), r))
                    .collect::<HashMap<_, _>>(),
            )
        })
        .collect_vec();

    let mut used_reports = HashSet::new();

    loop {
        {
            let Ok(Some(lighting_data)) = rx.recv() else {
                return;
            };

            for (dev, conf, reports) in &mut devices {
                used_reports.clear();

                for ((report_id, start_bit), target) in conf {
                    let Some(rep) = reports.get_mut(report_id) else {
                        continue;
                    };

                    let Ok(out) = rep
                        .outputs
                        .binary_search_by_key(start_bit, |x| x.bits().start)
                    else {
                        continue;
                    };

                    rep.outputs[out].real_value = lighting_data.get(*target);

                    used_reports.insert(*report_id);
                }

                for rep in &used_reports {
                    let Some(rep) = reports.get(rep) else {
                        continue;
                    };

                    _ = dev.write_report(rep);
                }
            }
        }
    }
}

#[derive(Default, Clone, Copy)]

pub struct LightingData {
    pub top: [Rgb<f32>; 2],

    pub middle: [Rgb<f32>; 2],

    pub bottom: [Rgb<f32>; 2],

    pub buttons: [bool; 7],
}

impl LightingData {
    fn get(&self, target: LightingTarget) -> f32 {
        match target {
            LightingTarget::Start => self.buttons[6] as u8 as f32,

            LightingTarget::Bt(bt_lane) => self.buttons[bt_lane as usize] as u8 as f32,

            LightingTarget::Fx(side) => self.buttons[4 + side as usize] as u8 as f32,

            LightingTarget::Top(side, channel) => channel.get(self.top[side as usize]),

            LightingTarget::Middle(side, channel) => channel.get(self.middle[side as usize]),

            LightingTarget::Bottom(side, channel) => channel.get(self.bottom[side as usize]),
        }
    }
}
