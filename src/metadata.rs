// TODO: Add fingerprint to Metadata
#[derive(Debug, Copy, Clone)]
pub struct Metadata {
    data_bytes: u64
}

impl Metadata {
    pub fn new(data_bytes: u64) -> Metadata {
        Metadata {
            data_bytes: data_bytes
        }
    }

    pub fn data_bytes(&self) -> u64 {
        self.data_bytes
    }
}