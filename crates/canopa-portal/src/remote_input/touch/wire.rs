//! Worker touch coordinates select approved PipeWire nodes; only the compositor
//! adapter translates these selectors into private capture stream identities.
pub use super::super::status::{Status, feedback, status};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    Frame,
    Down {
        contact: u32,
        node: u32,
        x: f64,
        y: f64,
    },
    Motion {
        contact: u32,
        node: u32,
        x: f64,
        y: f64,
    },
    Up {
        contact: u32,
    },
}
impl Command {
    pub fn valid(self) -> bool {
        match self {
            Self::Frame | Self::Up { .. } => true,
            Self::Down { node, x, y, .. } | Self::Motion { node, x, y, .. } => {
                node != 0
                    && node != u32::MAX
                    && [x, y]
                        .into_iter()
                        .all(|v| v.is_finite() && (0.0..=1_000_000.0).contains(&v))
            }
        }
    }
    pub fn packet(self, sequence: u32, stream: Option<u64>) -> [u8; 40] {
        let (kind, contact, node, x, y) = match self {
            Self::Frame => (0, 0, 0, 0., 0.),
            Self::Down {
                contact,
                node,
                x,
                y,
            } => (1, contact, node, x, y),
            Self::Motion {
                contact,
                node,
                x,
                y,
            } => (2, contact, node, x, y),
            Self::Up { contact } => (3, contact, 0, 0., 0.),
        };
        let mut bytes = [0; 40];
        bytes[..4].copy_from_slice(&sequence.to_le_bytes());
        bytes[4..8].copy_from_slice(&u32::to_le_bytes(kind));
        bytes[8..12].copy_from_slice(&contact.to_le_bytes());
        bytes[16..24].copy_from_slice(&stream.unwrap_or(u64::from(node)).to_le_bytes());
        bytes[24..32].copy_from_slice(&f64::to_le_bytes(x));
        bytes[32..40].copy_from_slice(&f64::to_le_bytes(y));
        bytes
    }
    pub fn decode(bytes: &[u8; 40], expected: u32) -> anyhow::Result<Self> {
        let word = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
        let wide = |offset| u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
        anyhow::ensure!(
            word(0) == expected && word(12) == 0,
            "Invalid touch sequence/reserved word"
        );
        let (kind, contact, node, a, b) = (word(4), word(8), wide(16), wide(24), wide(32));
        let command = match (kind, contact, node, a, b) {
            (0, 0, 0, 0, 0) => Self::Frame,
            (3, contact, 0, 0, 0) => Self::Up { contact },
            (1, contact, node, a, b) => Self::Down {
                contact,
                node: u32::try_from(node)?,
                x: f64::from_bits(a),
                y: f64::from_bits(b),
            },
            (2, contact, node, a, b) => Self::Motion {
                contact,
                node: u32::try_from(node)?,
                x: f64::from_bits(a),
                y: f64::from_bits(b),
            },
            _ => anyhow::bail!("Invalid touch command"),
        };
        anyhow::ensure!(command.valid(), "Invalid touch coordinates/node");
        Ok(command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_contacts_are_independent_of_nodes_and_private_streams() {
        for command in [
            Command::Frame,
            Command::Up { contact: u32::MAX },
            Command::Down {
                contact: 0,
                node: 9,
                x: 10.25,
                y: 20.5,
            },
            Command::Motion {
                contact: u32::MAX,
                node: 17,
                x: 0.,
                y: 1.,
            },
        ] {
            assert_eq!(
                Command::decode(&command.packet(3, None), 3).unwrap(),
                command
            );
        }
        let command = Command::Down {
            contact: 7,
            node: 9,
            x: 1.,
            y: 2.,
        };
        let packet = command.packet(1, Some(u64::MAX));
        assert_eq!(
            u64::from_le_bytes(packet[16..24].try_into().unwrap()),
            u64::MAX
        );
        assert!(Command::decode(&packet, 1).is_err());
    }
    #[test]
    fn invalid_payloads_are_rejected_before_a_contact_transition() {
        for (node, x, y) in [
            (0, 0., 0.),
            (u32::MAX, 0., 0.),
            (1, -1., 0.),
            (1, 0., f64::NAN),
            (1, f64::INFINITY, 0.),
            (1, 0., 1_000_001.),
        ] {
            assert!(
                !Command::Down {
                    contact: 0,
                    node,
                    x,
                    y
                }
                .valid()
            );
        }
        let mut p = Command::Frame.packet(1, None);
        p[8] = 1;
        assert!(Command::decode(&p, 1).is_err());
        let mut p = Command::Up { contact: 1 }.packet(1, None);
        p[12] = 1;
        assert!(Command::decode(&p, 1).is_err());
        assert!(Command::decode(&Command::Frame.packet(1, None), 2).is_err());
    }
}
