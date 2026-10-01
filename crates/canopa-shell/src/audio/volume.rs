//! Channel-preserving level policy shared by default-output and device controls.
use super::pulse::volume::{ChannelVolumes, Volume};

pub(super) enum Change {
    Adjust { step: u8, up: bool },
    Set { percent: u8 },
}

pub(super) fn apply(
    mut channels: ChannelVolumes,
    change: Change,
) -> Result<ChannelVolumes, String> {
    if !channels.is_valid() {
        return Err("Invalid audio channel volumes.".into());
    }
    let maximum = match change {
        Change::Set { percent } => {
            if percent > 100 {
                return Err("Volume must be between 0% and 100%.".into());
            }
            u32::from(percent) * Volume::NORMAL.0 / 100
        }
        Change::Adjust { step, up } => {
            // An increase never lowers externally amplified or unbalanced audio.
            if up && channels.max() >= Volume::NORMAL {
                return Ok(channels);
            }
            let delta = u32::from(step.min(100)) * Volume::NORMAL.0 / 100;
            if up {
                channels.max().0.saturating_add(delta)
            } else {
                channels.max().0.saturating_sub(delta)
            }
            .min(Volume::NORMAL.0)
        }
    };
    channels
        .scale(Volume(maximum))
        .ok_or("Invalid audio channel volumes.")?;
    Ok(channels)
}
