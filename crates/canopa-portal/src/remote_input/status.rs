//! Shared finite command acknowledgments for owned input transports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Accepted,
    Invalid,
    Limit,
    Unavailable,
}
impl Status {
    fn code(self) -> u32 {
        match self {
            Self::Accepted => 0,
            Self::Invalid => 1,
            Self::Limit => 2,
            Self::Unavailable => 3,
        }
    }
}
pub fn feedback(sequence: u32, status: Status) -> [u8; 16] {
    let mut data = [0; 16];
    data[..4].copy_from_slice(&sequence.to_le_bytes());
    data[4..8].copy_from_slice(&status.code().to_le_bytes());
    data
}
pub fn status(data: [u8; 16], sequence: u32) -> anyhow::Result<Status> {
    anyhow::ensure!(
        data[..4] == sequence.to_le_bytes() && data[8..] == [0; 8],
        "Invalid pointer acknowledgement"
    );
    match u32::from_le_bytes(data[4..8].try_into()?) {
        0 => Ok(Status::Accepted),
        1 => Ok(Status::Invalid),
        2 => Ok(Status::Limit),
        3 => Ok(Status::Unavailable),
        _ => anyhow::bail!("Invalid pointer status"),
    }
}
