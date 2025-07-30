use crate::{TrickleError, Result};

pub struct BitWriter {
    bit_buffer: u32,
    bit_count: usize,
}

impl BitWriter {
    pub fn new() -> Self {
        Self {
            bit_buffer: 0,
            bit_count: 0,
        }
    }
    
    pub fn write_to_buffer(&mut self, data: &[u8], output: &mut [u8]) -> Result<usize> {
        if data.len() > output.len() {
            return Err(TrickleError::InsufficientOutput);
        }
        
        output[..data.len()].copy_from_slice(data);
        Ok(data.len())
    }
}
