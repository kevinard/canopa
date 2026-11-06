//! Typed public input, independent of the compositor transport and EI framing.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Input {
    Keyboard(KeyboardInput),
    Pointer(PointerInput),
    Touch(TouchInput),
}
impl Input {
    pub(in crate::screencast) fn device(self) -> u32 {
        match self {
            Self::Keyboard(_) => 1,
            Self::Pointer(_) => 2,
            Self::Touch(_) => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TouchInput {
    Down {
        stream: u32,
        slot: u32,
        x: f64,
        y: f64,
    },
    Motion {
        stream: u32,
        slot: u32,
        x: f64,
        y: f64,
    },
    Up {
        slot: u32,
    },
}
impl TouchInput {
    fn checked(self) -> fdo::Result<Self> {
        let position = match self {
            Self::Down { stream, x, y, .. } | Self::Motion { stream, x, y, .. } => {
                Some((stream, x, y))
            }
            Self::Up { .. } => None,
        };
        if position.is_none_or(|(stream, x, y)| {
            stream != 0
                && stream != u32::MAX
                && [x, y]
                    .into_iter()
                    .all(|v| v.is_finite() && (0.0..=1_000_000.0).contains(&v))
        }) {
            Ok(self)
        } else {
            Err(fdo::Error::InvalidArgs("Invalid touch input".into()))
        }
    }
    pub(super) fn down(stream: u32, slot: u32, x: f64, y: f64) -> fdo::Result<Self> {
        Self::Down { stream, slot, x, y }.checked()
    }
    pub(super) fn motion(stream: u32, slot: u32, x: f64, y: f64) -> fdo::Result<Self> {
        Self::Motion { stream, slot, x, y }.checked()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointerInput {
    Relative { x: f64, y: f64 },
    Absolute { stream: u32, x: f64, y: f64 },
    Button { code: u32, pressed: bool },
    Axis { x: f64, y: f64, finish: bool },
    Discrete { axis: u32, steps: i32 },
}
impl PointerInput {
    fn checked(self) -> fdo::Result<Self> {
        let amount = |value: f64| value.is_finite() && value.abs() <= 1_000_000.;
        let valid = match self {
            Self::Relative { x, y } | Self::Axis { x, y, .. } => amount(x) && amount(y),
            Self::Absolute { stream, x, y } => {
                stream != 0 && stream != u32::MAX && amount(x) && amount(y) && x >= 0. && y >= 0.
            }
            Self::Button { code, .. } => (1..=0xffff).contains(&code),
            Self::Discrete { axis, steps } => axis <= 1 && (-32..=32).contains(&steps),
        };
        if valid {
            Ok(self)
        } else {
            Err(fdo::Error::InvalidArgs("Invalid pointer input".into()))
        }
    }
    pub(super) fn motion(x: f64, y: f64) -> fdo::Result<Self> {
        Self::Relative { x, y }.checked()
    }
    pub(super) fn absolute(stream: u32, x: f64, y: f64) -> fdo::Result<Self> {
        Self::Absolute { stream, x, y }.checked()
    }
    pub(super) fn button(code: i32, state: u32) -> fdo::Result<Self> {
        if code <= 0 || state > 1 {
            return Err(fdo::Error::InvalidArgs(
                "Invalid pointer button/state".into(),
            ));
        }
        Self::Button {
            code: code as u32,
            pressed: state == 1,
        }
        .checked()
    }
    pub(super) fn axis(x: f64, y: f64, mut options: Options) -> fdo::Result<Self> {
        Self::Axis {
            x,
            y,
            finish: option(&mut options, "finish")?.unwrap_or(false),
        }
        .checked()
    }
    pub(super) fn discrete(axis: u32, steps: i32) -> fdo::Result<Self> {
        Self::Discrete { axis, steps }.checked()
    }
}
