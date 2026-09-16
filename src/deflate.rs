extern crate alloc;

use alloc::boxed::Box;
use core::cmp;

use miniz_oxide::{
    deflate::{
        core::{CompressionStrategy, CompressorOxide},
        stream::deflate,
    },
    inflate::stream::{inflate, InflateState as MinizInflateState},
    DataFormat, MZError, MZFlush, MZStatus,
};

use crate::{CompressionConfig, CompressionStats, Result, TrickleError};

pub(crate) struct DeflateState {
    compressor: Box<CompressorOxide>,
    max_input_per_call: usize,
    bytes_processed: usize,
    bytes_output: usize,
    finished: bool,
}

impl DeflateState {
    pub(crate) fn new(config: &CompressionConfig) -> Self {
        let window_size = config.window_size.clamp(512, 32_768).next_power_of_two();
        let window_bits = window_size.trailing_zeros() as u8;

        Self {
            compressor: Box::new(CompressorOxide::with_params(
                DataFormat::Raw,
                config.level.value(),
                CompressionStrategy::Default,
                window_bits,
            )),
            max_input_per_call: config.max_input_per_call.max(1),
            bytes_processed: 0,
            bytes_output: 0,
            finished: false,
        }
    }

    pub(crate) fn compress_chunk(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        finish: bool,
    ) -> Result<(usize, usize, bool)> {
        if self.finished {
            return Ok((0, 0, true));
        }

        let input_len = cmp::min(input.len(), self.max_input_per_call);
        let chunk = &input[..input_len];
        let flush = if finish && input_len == input.len() {
            MZFlush::Finish
        } else {
            MZFlush::None
        };
        let result = deflate(&mut self.compressor, chunk, output, flush);

        self.bytes_processed += result.bytes_consumed;
        self.bytes_output += result.bytes_written;

        match result.status {
            Ok(MZStatus::StreamEnd) => self.finished = true,
            Ok(MZStatus::Ok) => {}
            Ok(MZStatus::NeedDict) => return Err(TrickleError::InvalidData),
            Err(MZError::Buf) if output.is_empty() => return Err(TrickleError::InsufficientOutput),
            Err(MZError::Buf) => return Err(TrickleError::NeedsMoreWork),
            Err(_) => return Err(TrickleError::InvalidData),
        }

        if result.bytes_consumed == 0 && result.bytes_written == 0 && !self.finished {
            if output.is_empty() {
                return Err(TrickleError::InsufficientOutput);
            }
            if input.is_empty() && !finish {
                return Ok((0, 0, false));
            }
            return Err(TrickleError::NeedsMoreWork);
        }

        Ok((result.bytes_consumed, result.bytes_written, self.finished))
    }

    pub(crate) fn stats(&self) -> CompressionStats {
        CompressionStats {
            bytes_processed: self.bytes_processed,
            bytes_output: self.bytes_output,
            compression_ratio: if self.bytes_processed > 0 {
                (self.bytes_output as f32) / (self.bytes_processed as f32)
            } else {
                0.0
            },
        }
    }
}

pub(crate) struct InflateState {
    inflater: Box<MinizInflateState>,
    finished: bool,
}

impl Default for InflateState {
    fn default() -> Self {
        Self::new()
    }
}

impl InflateState {
    pub(crate) fn new() -> Self {
        Self {
            inflater: MinizInflateState::new_boxed(DataFormat::Raw),
            finished: false,
        }
    }

    pub(crate) fn decompress_chunk(
        &mut self,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<(usize, usize, bool)> {
        if self.finished {
            return Ok((0, 0, true));
        }

        let result = inflate(&mut self.inflater, input, output, MZFlush::None);
        match result.status {
            Ok(MZStatus::StreamEnd) => self.finished = true,
            Ok(MZStatus::Ok) => {}
            Ok(MZStatus::NeedDict) => return Err(TrickleError::InvalidData),
            Err(MZError::Buf) if result.bytes_consumed != 0 || result.bytes_written != 0 => {}
            Err(MZError::Buf) if output.is_empty() => return Err(TrickleError::InsufficientOutput),
            Err(MZError::Buf) => return Err(TrickleError::InsufficientInput),
            Err(_) => return Err(TrickleError::InvalidData),
        }

        Ok((result.bytes_consumed, result.bytes_written, self.finished))
    }
}
