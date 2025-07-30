# tricklezip
A relaxed compression library for embedded devices, which does not use up all your CPU time at once.

It is based on the DEFLATE algorithm as described in [RFC1951](https://www.ietf.org/rfc/rfc1951.txt).

You may either run the compression just trickly, or in a given time. It is meant to be a relaxed compression library, so chill!

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
