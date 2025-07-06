#[derive(Default)]
pub struct U16Reader {
    buffer: Vec<u8>,
}

impl U16Reader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bytes_needed(&self) -> usize {
        2 - self.buffer.len()
    }

    pub fn write(&mut self, mut data: &[u8]) {
        if data.len() > self.bytes_needed() {
            data = &data[..self.bytes_needed()];
        }

        self.buffer.extend_from_slice(data);
    }

    pub fn finish(&self) -> Option<u16> {
        let Ok(&buffer) = self.buffer.as_slice().try_into() else {
            return None;
        };

        Some(u16::from_be_bytes(buffer))
    }
}
