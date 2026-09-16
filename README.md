# TrickleZip

TrickleZip is a `no_std`-compatible raw DEFLATE (RFC 1951) library for systems
that need to spread compression work across multiple calls.

## Features

- Produces and consumes interoperable raw DEFLATE streams
- Limits the input processed by each compression call
- Supports best-effort time budgets when the `std` feature is enabled
- Uses `no_std + alloc` by default for embedded environments
- Supports compression levels 0 through 9 and DEFLATE windows up to 32 KiB

TrickleZip emits raw DEFLATE data. It does not add zlib or gzip headers.

## One-shot compression

```rust
use tricklezip::{compress, decompress, TrickleError};

fn example() -> Result<(), TrickleError> {
    let input = b"Hello, world! Hello, world! Hello, world!";
    let mut compressed = vec![0; input.len() + 128];
    let compressed_len = compress(input, &mut compressed)?;

    let mut restored = vec![0; input.len()];
    let restored_len = decompress(&compressed[..compressed_len], &mut restored)?;
    assert_eq!(&restored[..restored_len], input);
    Ok(())
}
```

The caller owns the output buffers. `compress` returns
`TrickleError::InsufficientOutput` when the compressed stream does not fit.

## Incremental compression

```rust
use tricklezip::{CompressionConfig, TrickleCompressor, TrickleError};

fn incremental(input: &[u8], output: &mut [u8]) -> Result<usize, TrickleError> {
    let config = CompressionConfig {
        max_input_per_call: 256,
        ..CompressionConfig::default()
    };
    let mut compressor = TrickleCompressor::with_config(config);
    let mut input_offset = 0;
    let mut output_offset = 0;

    loop {
        let (consumed, written, finished) = compressor.compress_trickle(
            &input[input_offset..],
            &mut output[output_offset..],
            true,
        )?;
        input_offset += consumed;
        output_offset += written;

        if finished {
            return Ok(output_offset);
        }

        // Yield to other work here.
    }
}
```

`finish` means that the supplied input is the end of the stream. A call may
consume only part of that input; continue passing the unconsumed suffix until
`finished` is true.

## Time-limited compression

Enable the `std` feature to use `compress_timed`:

```toml
[dependencies]
tricklezip = { version = "0.1", features = ["std"] }
```

```rust,ignore
use std::time::Duration;
use tricklezip::{TrickleCompressor, TrickleError};

fn timed(input: &[u8], output: &mut [u8]) -> Result<(usize, usize, bool), TrickleError> {
    let mut compressor = TrickleCompressor::new();
    compressor.compress_timed(input, output, true, Duration::from_millis(10))
}
```

The time budget is cooperative: it is checked between work units controlled by
`CompressionConfig::max_input_per_call`. If the deadline is reached after work
has been completed, the function returns that progress with `finished = false`.
A deadline reached before any work returns `TrickleError::TimeoutExceeded`.

## Configuration

```rust
use tricklezip::{CompressionConfig, CompressionLevel, TrickleCompressor};

let config = CompressionConfig {
    level: CompressionLevel::FAST,
    window_size: 16_384,
    max_input_per_call: 512,
};

let compressor = TrickleCompressor::with_config(config);
```

Window sizes are rounded up to a power of two and constrained to 512 bytes
through 32 KiB. A zero work limit is normalized to one byte.

## Minimum supported Rust version

TrickleZip supports Rust 1.85 and newer.

## License

Licensed under the Apache License, Version 2.0.
