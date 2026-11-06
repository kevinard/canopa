//! Private pointer commands share the compositor's finite 32-byte framing.
//! Absolute selectors on the worker socket are PipeWire nodes, not stream IDs.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    Barrier,
    Relative {
        x: f64,
        y: f64,
    },
    Absolute {
        node: u32,
        x: f64,
        y: f64,
    },
    Button {
        code: u32,
        pressed: bool,
    },
    Scroll {
        x: Option<f64>,
        y: Option<f64>,
        finger: bool,
    },
    Wheel {
        x: Option<i32>,
        y: Option<i32>,
    },
}
pub use super::super::status::{Status, feedback, status};
impl Command {
    pub fn valid(self) -> bool {
        let amount = |v: f64| v.is_finite() && v.abs() <= 1_000_000.;
        match self {
            Self::Barrier => true,
            Self::Relative { x, y } => amount(x) && amount(y),
            Self::Absolute { node, x, y } => {
                node != 0 && node != u32::MAX && amount(x) && amount(y) && x >= 0. && y >= 0.
            }
            Self::Button { code, .. } => (1..=0xffff).contains(&code),
            Self::Scroll { x, y, .. } => {
                (x.is_some() || y.is_some()) && x.is_none_or(amount) && y.is_none_or(amount)
            }
            Self::Wheel { x, y } => {
                (x.is_some() || y.is_some())
                    && [x, y]
                        .into_iter()
                        .flatten()
                        .all(|v| v != 0 && (-3840..=3840).contains(&v))
            }
        }
    }
    pub fn packet(self, sequence: u32, absolute_stream: Option<u64>) -> [u8; 32] {
        let (kind, a, b, c) = match self {
            Self::Barrier => (0, 0, 0, 0),
            Self::Relative { x, y } => (1, x.to_bits(), y.to_bits(), 0),
            Self::Button { code, pressed } => (2, u64::from(code), u64::from(pressed), 0),
            Self::Scroll { x, y, finger } => (
                3,
                x.unwrap_or(0.).to_bits(),
                y.unwrap_or(0.).to_bits(),
                u64::from(x.is_some()) | (u64::from(y.is_some()) << 1) | (u64::from(finger) << 2),
            ),
            Self::Wheel { x, y } => (
                4,
                x.map_or(0, |v| i64::from(v) as u64),
                y.map_or(0, |v| i64::from(v) as u64),
                u64::from(x.is_some()) | (u64::from(y.is_some()) << 1),
            ),
            Self::Absolute { node, x, y } => (
                5,
                x.to_bits(),
                y.to_bits(),
                absolute_stream.unwrap_or(u64::from(node)),
            ),
        };
        let mut data = [0; 32];
        data[..4].copy_from_slice(&sequence.to_le_bytes());
        data[4..8].copy_from_slice(&u32::to_le_bytes(kind));
        for (i, value) in [a, b, c].into_iter().enumerate() {
            data[8 + i * 8..16 + i * 8].copy_from_slice(&value.to_le_bytes());
        }
        data
    }
    pub fn decode(data: &[u8; 32], expected: u32) -> anyhow::Result<Self> {
        anyhow::ensure!(
            u32::from_le_bytes(data[..4].try_into()?) == expected,
            "Invalid pointer sequence"
        );
        let kind = u32::from_le_bytes(data[4..8].try_into()?);
        let words: [u64; 3] = std::array::from_fn(|i| {
            u64::from_le_bytes(data[8 + i * 8..16 + i * 8].try_into().unwrap())
        });
        let [a, b, c] = words;
        let command = match kind {
            0 if words == [0; 3] => Self::Barrier,
            1 if c == 0 => Self::Relative {
                x: f64::from_bits(a),
                y: f64::from_bits(b),
            },
            2 if c == 0 && a <= 0xffff && b <= 1 => Self::Button {
                code: a as u32,
                pressed: b == 1,
            },
            3 if (1..=7).contains(&c)
                && c & 3 != 0
                && (c & 1 != 0 || a == 0)
                && (c & 2 != 0 || b == 0) =>
            {
                Self::Scroll {
                    x: (c & 1 != 0).then_some(f64::from_bits(a)),
                    y: (c & 2 != 0).then_some(f64::from_bits(b)),
                    finger: c & 4 != 0,
                }
            }
            4 if (1..=3).contains(&c) && (c & 1 != 0 || a == 0) && (c & 2 != 0 || b == 0) => {
                Self::Wheel {
                    x: if c & 1 != 0 {
                        Some(i32::try_from(a as i64)?)
                    } else {
                        None
                    },
                    y: if c & 2 != 0 {
                        Some(i32::try_from(b as i64)?)
                    } else {
                        None
                    },
                }
            }
            5 => Self::Absolute {
                node: u32::try_from(c)?,
                x: f64::from_bits(a),
                y: f64::from_bits(b),
            },
            _ => anyhow::bail!("Invalid pointer command"),
        };
        anyhow::ensure!(command.valid(), "Invalid pointer command values");
        Ok(command)
    }
}
