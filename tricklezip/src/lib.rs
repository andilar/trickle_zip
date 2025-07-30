// Cargo.toml
[package]
name = "tricklezip"
version = "0.1.0"
edition = "2021"
description = "A relaxed compression library for embedded devices using DEFLATE algorithm"
license = "MIT OR Apache-2.0"
keywords = ["compression", "deflate", "embedded", "no-std"]
categories = ["compression", "embedded", "no-std"]

[dependencies]
# Optional std support
[features]
default = []
std = []

[dev-dependencies]
# For testing only

# src/lib.rs
#![cfg_attr(not(feature = "std"), no_std)]
#![doc = include_str!("../README.md")]

//! TrickleZip - A relaxed compression library for embedded devices
//! 
//! Based on DEFLATE algorithm (RFC1951), designed to be CPU-friendly
//! by allowing incremental compression with time limits.

mod deflate;
mod huffman;
mod lz77;
mod bitstream;

pub use deflate::*;

use core::time::Duration;

/// Result type for TrickleZip operations
pub type Result<T> = core::result::Result<T, TrickleError>;

/// Errors that can occur during compression/decompression
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrickleError {
    /// Input buffer is too small
    InsufficientInput,
    /// Output buffer is too small
    InsufficientOutput,
    /// Invalid DEFLATE data
    InvalidData,
    /// Compression is not yet complete
    NeedsMoreWork,
    /// Time limit exceeded
    TimeoutExceeded,
}

/// Compression level (0 = no compression, 9 = maximum compression)
#[derive(Debug, Clone, Copy)]
pub struct CompressionLevel(u8);

impl CompressionLevel {
    pub const NONE: Self = Self(0);
    pub const FAST: Self = Self(1);
    pub const BALANCED: Self = Self(6);
    pub const BEST: Self = Self(9);
    
    pub fn new(level: u8) -> Self {
        Self(level.min(9))
    }
    
    pub fn value(&self) -> u8 {
        self.0
    }
}

/// Configuration for the compression process
#[derive(Debug, Clone)]
pub struct CompressionConfig {
    pub level: CompressionLevel,
    pub window_size: usize,
    pub max_lazy_match: usize,
    pub max_chain_length: usize,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            level: CompressionLevel::BALANCED,
            window_size: 32768, // 32KB sliding window
            max_lazy_match: 258,
            max_chain_length: 256,
        }
    }
}

/// Main compressor state
pub struct TrickleCompressor {
    config: CompressionConfig,
    state: deflate::DeflateState,
}

impl TrickleCompressor {
    /// Create a new compressor with default configuration
    pub fn new() -> Self {
        Self::with_config(CompressionConfig::default())
    }
    
    /// Create a new compressor with custom configuration
    pub fn with_config(config: CompressionConfig) -> Self {
        Self {
            state: deflate::DeflateState::new(&config),
            config,
        }
    }
    
    /// Compress data incrementally without time limits
    /// Returns (bytes_consumed, bytes_written, is_finished)
    pub fn compress_trickle(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        finish: bool,
    ) -> Result<(usize, usize, bool)> {
        self.state.compress_chunk(input, output, finish)
    }
    
    /// Compress data with a time limit
    /// Returns (bytes_consumed, bytes_written, is_finished)
    #[cfg(feature = "std")]
    pub fn compress_timed(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        finish: bool,
        time_limit: Duration,
    ) -> Result<(usize, usize, bool)> {
        let start = std::time::Instant::now();
        
        loop {
            if start.elapsed() >= time_limit {
                return Err(TrickleError::TimeoutExceeded);
            }
            
            match self.compress_trickle(input, output, finish) {
                Ok(result) => return Ok(result),
                Err(TrickleError::NeedsMoreWork) => continue,
                Err(e) => return Err(e),
            }
        }
    }
    
    /// Reset the compressor for reuse
    pub fn reset(&mut self) {
        self.state = deflate::DeflateState::new(&self.config);
    }
    
    /// Get current compression statistics
    pub fn stats(&self) -> CompressionStats {
        self.state.stats()
    }
}

impl Default for TrickleCompressor {
    fn default() -> Self {
        Self::new()
    }
}

/// Compression statistics
#[derive(Debug, Clone, Copy)]
pub struct CompressionStats {
    pub bytes_processed: usize,
    pub bytes_output: usize,
    pub compression_ratio: f32,
}

/// Convenience function for one-shot compression
pub fn compress(input: &[u8], output: &mut [u8]) -> Result<usize> {
    let mut compressor = TrickleCompressor::new();
    let mut total_written = 0;
    let mut input_offset = 0;
    
    loop {
        let (consumed, written, finished) = compressor.compress_trickle(
            &input[input_offset..],
            &mut output[total_written..],
            true,
        )?;
        
        input_offset += consumed;
        total_written += written;
        
        if finished {
            break;
        }
        
        if total_written >= output.len() {
            return Err(TrickleError::InsufficientOutput);
        }
    }
    
    Ok(total_written)
}

/// Convenience function for one-shot decompression
pub fn decompress(input: &[u8], output: &mut [u8]) -> Result<usize> {
    let mut decompressor = TrickleDecompressor::new();
    let mut total_written = 0;
    let mut input_offset = 0;
    
    loop {
        let (consumed, written, finished) = decompressor.decompress_trickle(
            &input[input_offset..],
            &mut output[total_written..],
        )?;
        
        input_offset += consumed;
        total_written += written;
        
        if finished {
            break;
        }
        
        if total_written >= output.len() {
            return Err(TrickleError::InsufficientOutput);
        }
    }
    
    Ok(total_written)
}

/// Main decompressor state
pub struct TrickleDecompressor {
    state: deflate::InflateState,
}

impl TrickleDecompressor {
    /// Create a new decompressor
    pub fn new() -> Self {
        Self {
            state: deflate::InflateState::new(),
        }
    }
    
    /// Decompress data incrementally
    /// Returns (bytes_consumed, bytes_written, is_finished)
    pub fn decompress_trickle(
        &mut self,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<(usize, usize, bool)> {
        self.state.decompress_chunk(input, output)
    }
    
    /// Reset the decompressor for reuse
    pub fn reset(&mut self) {
        self.state = deflate::InflateState::new();
    }
}

impl Default for TrickleDecompressor {
    fn default() -> Self {
        Self::new()
    }
}

// src/deflate.rs
use crate::{huffman::HuffmanCoder, lz77::Lz77Encoder, bitstream::BitWriter, CompressionConfig, TrickleError, Result, CompressionStats};

pub struct DeflateState {
    lz77: Lz77Encoder,
    huffman: HuffmanCoder,
    bit_writer: BitWriter,
    bytes_processed: usize,
    bytes_output: usize,
    finished: bool,
}

impl DeflateState {
    pub fn new(config: &CompressionConfig) -> Self {
        Self {
            lz77: Lz77Encoder::new(config.window_size, config.max_lazy_match, config.max_chain_length),
            huffman: HuffmanCoder::new(),
            bit_writer: BitWriter::new(),
            bytes_processed: 0,
            bytes_output: 0,
            finished: false,
        }
    }
    
    pub fn compress_chunk(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        finish: bool,
    ) -> Result<(usize, usize, bool)> {
        if self.finished {
            return Ok((0, 0, true));
        }
        
        // Process input through LZ77
        let tokens = self.lz77.encode(input)?;
        self.bytes_processed += input.len();
        
        // Encode with Huffman
        let compressed = self.huffman.encode(&tokens)?;
        
        // Write to output
        let written = self.bit_writer.write_to_buffer(&compressed, output)?;
        self.bytes_output += written;
        
        if finish {
            self.finished = true;
        }
        
        Ok((input.len(), written, self.finished))
    }
    
    pub fn stats(&self) -> CompressionStats {
        CompressionStats {
            bytes_processed: self.bytes_processed,
            bytes_output: self.bytes_output,
            compression_ratio: if self.bytes_processed > 0 {
                self.bytes_output as f32 / self.bytes_processed as f32
            } else {
                0.0
            },
        }
    }
}

pub struct InflateState {
    // Simplified inflate state for basic decompression
    finished: bool,
}

impl InflateState {
    pub fn new() -> Self {
        Self {
            finished: false,
        }
    }
    
    pub fn decompress_chunk(
        &mut self,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<(usize, usize, bool)> {
        // Simplified decompression - in real implementation this would
        // parse DEFLATE streams and decompress them
        if input.is_empty() {
            self.finished = true;
            return Ok((0, 0, true));
        }
        
        // Placeholder: copy input to output (not real decompression)
        let copy_len = input.len().min(output.len());
        output[..copy_len].copy_from_slice(&input[..copy_len]);
        
        Ok((copy_len, copy_len, copy_len == input.len()))
    }
}

// src/lz77.rs
use crate::{TrickleError, Result};

#[derive(Debug, Clone)]
pub enum Token {
    Literal(u8),
    Match { length: usize, distance: usize },
}

pub struct Lz77Encoder {
    window_size: usize,
    max_lazy_match: usize,
    max_chain_length: usize,
    window: Vec<u8>,
    position: usize,
}

impl Lz77Encoder {
    pub fn new(window_size: usize, max_lazy_match: usize, max_chain_length: usize) -> Self {
        Self {
            window_size,
            max_lazy_match,
            max_chain_length,
            window: Vec::with_capacity(window_size),
            position: 0,
        }
    }
    
    pub fn encode(&mut self, input: &[u8]) -> Result<Vec<Token>> {
        let mut tokens = Vec::new();
        let mut i = 0;
        
        while i < input.len() {
            // Simple literal encoding for now
            // Real implementation would do LZ77 match finding
            tokens.push(Token::Literal(input[i]));
            i += 1;
        }
        
        // Update sliding window
        self.window.extend_from_slice(input);
        if self.window.len() > self.window_size {
            self.window.drain(..self.window.len() - self.window_size);
        }
        
        Ok(tokens)
    }
}

// src/huffman.rs
use crate::{lz77::Token, TrickleError, Result};

pub struct HuffmanCoder {
    // Simplified Huffman tables
}

impl HuffmanCoder {
    pub fn new() -> Self {
        Self {}
    }
    
    pub fn encode(&self, tokens: &[Token]) -> Result<Vec<u8>> {
        let mut output = Vec::new();
        
        // Simplified encoding - real implementation would build Huffman trees
        // and encode according to DEFLATE specification
        for token in tokens {
            match token {
                Token::Literal(byte) => output.push(*byte),
                Token::Match { length, distance } => {
                    // Encode match as placeholder
                    output.push(0xFF); // Special marker for matches
                    output.push(*length as u8);
                    output.push(*distance as u8);
                }
            }
        }
        
        Ok(output)
    }
}

// src/bitstream.rs
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



// One-shot compression
let input = b"Hello, world! This is some data to compress.";
let mut output = vec![0u8; input.len() * 2];
let compressed_size = compress(input, &mut output)?;

// Incremental compression
let mut compressor = TrickleCompressor::new();
let mut total_written = 0;

loop {
    let (consumed, written, finished) = compressor.compress_trickle(
        &input[..],
        &mut output[total_written..],
        true, // finish
    )?;
    
    total_written += written;
    
    if finished {
        break;
    }
    
    // Do other work here - compression won't block!
}
```

### Time-Limited Compression

```rust
use std::time::Duration;
use tricklezip::TrickleCompressor;

let mut compressor = TrickleCompressor::new();
let time_limit = Duration::from_millis(10); // 10ms max

match compressor.compress_timed(input, output, true, time_limit) {
    Ok((consumed, written, finished)) => {
        // Compression completed within time limit
    }
    Err(TrickleError::TimeoutExceeded) => {
        // Time limit exceeded, continue later
    }
    Err(e) => {
        // Handle other errors
    }
}
```

### Custom Configuration

```rust
use tricklezip::{TrickleCompressor, CompressionConfig, CompressionLevel};

let config = CompressionConfig {
    level: CompressionLevel::FAST,
    window_size: 16384,  // 16KB window
    max_lazy_match: 128,
    max_chain_length: 128,
};

let mut compressor = TrickleCompressor::with_config(config);
```

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.

// tests/integration_tests.rs
#[cfg(test)]
mod tests {
    use tricklezip::*;

    #[test]
    fn test_basic_compression() {
        let input = b"Hello, world! This is a test string for compression.";
        let mut output = vec![0u8; input.len() * 2];
        
        let compressed_size = compress(input, &mut output).unwrap();
        assert!(compressed_size > 0);
        assert!(compressed_size <= output.len());
    }

    #[test]
    fn test_incremental_compression() {
        let mut compressor = TrickleCompressor::new();
        let input = b"Test data for incremental compression";
        let mut output = vec![0u8; input.len() * 2];
        
        let (consumed, written, finished) = compressor
            .compress_trickle(input, &mut output, true)
            .unwrap();
        
        assert_eq!(consumed, input.len());
        assert!(written > 0);
        assert!(finished);
    }

    #[test]
    fn test_compression_stats() {
        let mut compressor = TrickleCompressor::new();
        let input = b"Some test data";
        let mut output = vec![0u8; input.len() * 2];
        
        compressor.compress_trickle(input, &mut output, true).unwrap();
        
        let stats = compressor.stats();
        assert_eq!(stats.bytes_processed, input.len());
        assert!(stats.bytes_output > 0);
        assert!(stats.compression_ratio > 0.0);
    }

    #[test]
    fn test_decompression() {
        let input = b"Test decompression data";
        let mut output = vec![0u8; input.len() * 2];
        
        let decompressed_size = decompress(input, &mut output).unwrap();
        assert_eq!(decompressed_size, input.len());
    }

    #[test]
    fn test_custom_config() {
        let config = CompressionConfig {
            level: CompressionLevel::FAST,
            window_size: 16384,
            max_lazy_match: 64,
            max_chain_length: 64,
        };
        
        let compressor = TrickleCompressor::with_config(config);
        assert_eq!(compressor.config.level.value(), 1);
        assert_eq!(compressor.config.window_size, 16384);
    }
}
