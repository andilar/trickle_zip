use crate::{ TrickleError, Result };

extern crate alloc;
use alloc::vec::Vec;

pub struct BitWriter {
    bit_buffer: u32,
    bit_count: usize,
    output: Vec<u8>,
}

impl BitWriter {
    pub fn new() -> Self {
        Self {
            bit_buffer: 0,
            bit_count: 0,
            output: Vec::new(),
        }
    }

    /// Write bits to the buffer
    pub fn write_bits(&mut self, value: u32, num_bits: usize) -> Result<()> {
        if num_bits > 32 {
            return Err(TrickleError::InvalidData);
        }

        // Add bits to buffer
        self.bit_buffer |= (value & ((1 << num_bits) - 1)) << self.bit_count;
        self.bit_count += num_bits;

        // Flush complete bytes
        while self.bit_count >= 8 {
            self.output.push((self.bit_buffer & 0xff) as u8);
            self.bit_buffer >>= 8;
            self.bit_count -= 8;
        }

        Ok(())
    }

    /// Write a single byte as 8 bits
    pub fn write_byte(&mut self, byte: u8) -> Result<()> {
        self.write_bits(byte as u32, 8)
    }

    /// Flush any remaining bits (pad with zeros if needed)
    pub fn flush(&mut self) -> Result<()> {
        if self.bit_count > 0 {
            self.output.push((self.bit_buffer & 0xff) as u8);
            self.bit_buffer = 0;
            self.bit_count = 0;
        }
        Ok(())
    }

    /// Write the accumulated bits to output buffer
    pub fn write_to_buffer(&mut self, data: &[u8], output: &mut [u8]) -> Result<usize> {
        // First write the input data as bytes
        for &byte in data {
            self.write_byte(byte)?;
        }

        // Flush any remaining bits
        self.flush()?;

        // Copy to output buffer
        if self.output.len() > output.len() {
            return Err(TrickleError::InsufficientOutput);
        }

        let bytes_written = self.output.len();
        output[..bytes_written].copy_from_slice(&self.output);

        // Clear internal buffer for next use
        self.output.clear();

        Ok(bytes_written)
    }

    /// Get current buffer state (for debugging)
    pub fn buffer_info(&self) -> (u32, usize) {
        (self.bit_buffer, self.bit_count)
    }
}
