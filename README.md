# TrickleZip

A relaxed compression library for embedded devices, which does not use up all your CPU time at once.

## Features

- 🚀 **CPU-friendly**: Incremental compression that won't block your system
- ⏱️ **Time-controlled**: Set time limits for compression operations
- 📦 **DEFLATE-based**: Uses the proven DEFLATE algorithm (RFC1951)
- 🔧 **Embedded-ready**: `no_std` support for resource-constrained environments
- 😌 **Relaxed approach**: Designed to be gentle on your system resources

## Usage

### Basic Compression

```rust
use tricklezip::{TrickleCompressor, compress};

fn example() -> Result<(), Box<dyn std::error::Error>> {
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
    
    Ok(())
}
```

### Time-Limited Compression

```rust
use std::time::Duration;
use tricklezip::TrickleCompressor;

fn timed_example() -> Result<(), Box<dyn std::error::Error>> {
    let mut compressor = TrickleCompressor::new();
    let input = b"test data";
    let mut output = vec![0u8; 1024];
    let time_limit = Duration::from_millis(10); // 10ms max

    match compressor.compress_timed(input, &mut output, true, time_limit) {
        Ok((consumed, written, finished)) => {
            // Compression completed within time limit
        }
        Err(tricklezip::TrickleError::TimeoutExceeded) => {
            // Time limit exceeded, continue later
        }
        Err(e) => {
            // Handle other errors
        }
    }
    
    Ok(())
}
```

### Custom Configuration

```rust
use tricklezip::{TrickleCompressor, CompressionConfig, CompressionLevel};

fn config_example() {
    let config = CompressionConfig {
        level: CompressionLevel::FAST,
        window_size: 16384,  // 16KB window
        max_lazy_match: 128,
        max_chain_length: 128,
    };

    let mut compressor = TrickleCompressor::with_config(config);
}
```

## License

Licensed under Apache License, Version 2.0
