#![cfg_attr(not(feature = "std"), no_std)]
#![doc = include_str!("../README.md")]

//! TrickleZip - A relaxed compression library for embedded devices
//!
//! Based on DEFLATE algorithm (RFC1951), designed to be CPU-friendly
//! by allowing incremental compression with time limits.

extern crate alloc;

mod deflate;

#[cfg(feature = "std")]
use std::time::Duration;

/// Result type for TrickleZip operations
pub type Result<T> = core::result::Result<T, TrickleError>;

/// Errors that can occur during compression/decompression
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrickleError {
    /// More compressed input is required to complete decompression.
    InsufficientInput,
    /// The supplied output buffer has no space for further progress.
    InsufficientOutput,
    /// The input is not a valid raw DEFLATE stream.
    InvalidData,
    /// The operation could not make progress in the current call.
    NeedsMoreWork,
    /// The time limit elapsed before any work could be completed.
    TimeoutExceeded,
}

impl core::fmt::Display for TrickleError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TrickleError::InsufficientInput => write!(f, "Input buffer is too small"),
            TrickleError::InsufficientOutput => write!(f, "Output buffer is too small"),
            TrickleError::InvalidData => write!(f, "Invalid DEFLATE data"),
            TrickleError::NeedsMoreWork => write!(f, "Compression is not yet complete"),
            TrickleError::TimeoutExceeded => write!(f, "Time limit exceeded"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TrickleError {}

/// Compression level (0 = no compression, 9 = maximum compression)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompressionLevel(u8);

impl CompressionLevel {
    /// Store blocks without compression.
    pub const NONE: Self = Self(0);
    /// Optimize for compression speed.
    pub const FAST: Self = Self(1);
    /// Balance speed and compressed size.
    pub const BALANCED: Self = Self(6);
    /// Optimize for compressed size.
    pub const BEST: Self = Self(9);

    /// Creates a compression level, clamped to the supported range of 0 through 9.
    pub const fn new(level: u8) -> Self {
        if level > 9 {
            Self(9)
        } else {
            Self(level)
        }
    }

    /// Returns the numeric compression level.
    pub const fn value(self) -> u8 {
        self.0
    }
}

/// Configuration for the compression process
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompressionConfig {
    /// Compression level from no compression through best compression.
    pub level: CompressionLevel,
    /// Requested DEFLATE window size in bytes.
    ///
    /// Values are rounded up to a power of two and constrained to 512 through 32,768.
    pub window_size: usize,
    /// Maximum number of input bytes processed by one `compress_trickle` call.
    /// Smaller values provide finer cooperative scheduling at some throughput cost.
    pub max_input_per_call: usize,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            level: CompressionLevel::BALANCED,
            window_size: 32768, // 32KB sliding window
            max_input_per_call: 4096,
        }
    }
}

/// Stateful raw-DEFLATE compressor.
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

    /// Compresses at most one configured work unit.
    ///
    /// Returns `(bytes_consumed, bytes_written, is_finished)`. Set `finish` when
    /// the supplied input contains the end of the stream. Continue passing any
    /// unconsumed input until `is_finished` is true.
    pub fn compress_trickle(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        finish: bool,
    ) -> Result<(usize, usize, bool)> {
        self.state.compress_chunk(input, output, finish)
    }

    /// Compresses input within a cooperative time budget.
    ///
    /// The deadline is checked between configured work units. When it expires
    /// after progress, that progress is returned with `is_finished = false`.
    #[cfg(feature = "std")]
    pub fn compress_timed(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        finish: bool,
        time_limit: Duration,
    ) -> Result<(usize, usize, bool)> {
        let start = std::time::Instant::now();
        let mut total_consumed = 0;
        let mut total_written = 0;

        loop {
            if start.elapsed() >= time_limit {
                return if total_consumed == 0 && total_written == 0 {
                    Err(TrickleError::TimeoutExceeded)
                } else {
                    Ok((total_consumed, total_written, false))
                };
            }

            let (consumed, written, finished) = self.compress_trickle(
                &input[total_consumed..],
                &mut output[total_written..],
                finish,
            )?;
            total_consumed += consumed;
            total_written += written;

            if finished || (total_consumed == input.len() && !finish) {
                return Ok((total_consumed, total_written, finished));
            }
            if total_written == output.len() {
                return Ok((total_consumed, total_written, false));
            }
            if consumed == 0 && written == 0 {
                return Err(TrickleError::NeedsMoreWork);
            }
        }
    }

    /// Resets the compressor for a new raw-DEFLATE stream.
    pub fn reset(&mut self) {
        self.state = deflate::DeflateState::new(&self.config);
    }

    /// Returns statistics for the current stream.
    pub fn stats(&self) -> CompressionStats {
        self.state.stats()
    }
}

impl Default for TrickleCompressor {
    fn default() -> Self {
        Self::new()
    }
}

/// Compression statistics for the current stream.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompressionStats {
    /// Number of uncompressed input bytes consumed.
    pub bytes_processed: usize,
    /// Number of compressed output bytes produced.
    pub bytes_output: usize,
    /// Compressed bytes divided by processed bytes, or zero before input.
    pub compression_ratio: f32,
}

/// Compresses an entire raw-DEFLATE stream into a caller-provided buffer.
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

        if total_written >= output.len() && !finished {
            return Err(TrickleError::InsufficientOutput);
        }
    }

    Ok(total_written)
}

/// Decompresses an entire raw-DEFLATE stream into a caller-provided buffer.
pub fn decompress(input: &[u8], output: &mut [u8]) -> Result<usize> {
    let mut decompressor = TrickleDecompressor::new();
    let mut total_written = 0;
    let mut input_offset = 0;

    loop {
        let (consumed, written, finished) = decompressor
            .decompress_trickle(&input[input_offset..], &mut output[total_written..])?;

        input_offset += consumed;
        total_written += written;

        if finished {
            break;
        }

        if consumed == 0 && written == 0 {
            return Err(TrickleError::InsufficientInput);
        }

        if total_written >= output.len() && !finished {
            return Err(TrickleError::InsufficientOutput);
        }
    }

    Ok(total_written)
}

/// Stateful raw-DEFLATE decompressor.
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

    /// Decompresses data incrementally.
    ///
    /// Returns `(bytes_consumed, bytes_written, is_finished)`.
    pub fn decompress_trickle(
        &mut self,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<(usize, usize, bool)> {
        self.state.decompress_chunk(input, output)
    }

    /// Resets the decompressor for a new raw-DEFLATE stream.
    pub fn reset(&mut self) {
        self.state = deflate::InflateState::new();
    }
}

impl Default for TrickleDecompressor {
    fn default() -> Self {
        Self::new()
    }
}
