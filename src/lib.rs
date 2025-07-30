#![cfg_attr(not(feature = "std"), no_std)]
#![doc = include_str!("../README.md")]

//! TrickleZip - A relaxed compression library for embedded devices
//! 
//! Based on DEFLATE algorithm (RFC1951), designed to be CPU-friendly
//! by allowing incremental compression with time limits.

#[cfg(feature = "alloc")]
extern crate alloc;

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
