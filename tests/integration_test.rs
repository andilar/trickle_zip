use tricklezip::{
    compress, decompress, CompressionConfig, CompressionLevel, TrickleCompressor,
    TrickleDecompressor, TrickleError,
};

const KNOWN_DEFLATE: &[u8] = &[
    243, 72, 205, 201, 201, 215, 81, 40, 74, 44, 87, 112, 113, 117, 243, 113, 12, 113, 85, 4, 0,
];
const KNOWN_PLAINTEXT: &[u8] = b"Hello, raw DEFLATE!";

#[test]
fn compresses_and_round_trips_repetitive_data() {
    let input = vec![b'a'; 8 * 1024];
    let mut compressed = vec![0; input.len() + 256];
    let compressed_len = compress(&input, &mut compressed).unwrap();

    assert!(compressed_len < input.len());
    assert_ne!(&compressed[..compressed_len], input.as_slice());

    let mut restored = vec![0; input.len()];
    let restored_len = decompress(&compressed[..compressed_len], &mut restored).unwrap();
    assert_eq!(restored_len, input.len());
    assert_eq!(restored, input);
}

#[test]
fn decompresses_a_known_rfc1951_stream() {
    let mut output = [0; 64];
    let written = decompress(KNOWN_DEFLATE, &mut output).unwrap();

    assert_eq!(&output[..written], KNOWN_PLAINTEXT);
}

#[test]
fn compression_is_bounded_per_call() {
    let config = CompressionConfig {
        level: CompressionLevel::FAST,
        window_size: 16_384,
        max_input_per_call: 64,
    };
    let mut compressor = TrickleCompressor::with_config(config);
    let input = vec![b'x'; 1024];
    let mut output = vec![0; 2048];

    let (consumed, _, finished) = compressor
        .compress_trickle(&input, &mut output, true)
        .unwrap();

    assert!(consumed <= 64);
    assert!(!finished);
}

#[test]
fn incremental_stream_round_trips() {
    let input = b"A streaming DEFLATE payload. ".repeat(300);
    let config = CompressionConfig {
        max_input_per_call: 97,
        ..CompressionConfig::default()
    };
    let mut compressor = TrickleCompressor::with_config(config);
    let mut compressed = vec![0; input.len() + 256];
    let mut input_offset = 0;
    let mut output_offset = 0;

    loop {
        let (consumed, written, finished) = compressor
            .compress_trickle(
                &input[input_offset..],
                &mut compressed[output_offset..],
                true,
            )
            .unwrap();
        input_offset += consumed;
        output_offset += written;
        if finished {
            break;
        }
    }

    let mut decompressor = TrickleDecompressor::new();
    let mut restored = vec![0; input.len()];
    let mut compressed_offset = 0;
    let mut restored_offset = 0;

    loop {
        let output_end = (restored_offset + 31).min(restored.len());
        let (consumed, written, finished) = decompressor
            .decompress_trickle(
                &compressed[compressed_offset..output_offset],
                &mut restored[restored_offset..output_end],
            )
            .unwrap();
        compressed_offset += consumed;
        restored_offset += written;
        if finished {
            break;
        }
    }

    assert_eq!(compressed_offset, output_offset);
    assert_eq!(restored_offset, input.len());
    assert_eq!(restored, input);
}

#[test]
fn reports_output_and_input_errors() {
    let input = b"some input that cannot fit into a one-byte output";
    assert_eq!(
        compress(input, &mut [0; 1]),
        Err(TrickleError::InsufficientOutput)
    );

    let truncated = &KNOWN_DEFLATE[..KNOWN_DEFLATE.len() - 1];
    assert_eq!(
        decompress(truncated, &mut [0; 64]),
        Err(TrickleError::InsufficientInput)
    );
}

#[test]
fn tracks_compression_statistics() {
    let input = vec![b'z'; 1024];
    let mut output = vec![0; 2048];
    let mut compressor = TrickleCompressor::new();

    let (_, _, finished) = compressor
        .compress_trickle(&input, &mut output, true)
        .unwrap();

    assert!(finished);
    let stats = compressor.stats();
    assert_eq!(stats.bytes_processed, input.len());
    assert!(stats.bytes_output > 0);
    assert!(stats.compression_ratio < 1.0);
}

#[cfg(feature = "std")]
#[test]
fn timed_compression_completes_or_returns_progress() {
    use std::time::Duration;

    let config = CompressionConfig {
        max_input_per_call: 32,
        ..CompressionConfig::default()
    };
    let mut compressor = TrickleCompressor::with_config(config);
    let input = vec![b't'; 16 * 1024];
    let mut output = vec![0; input.len() + 256];

    let (consumed, written, _) = compressor
        .compress_timed(&input, &mut output, true, Duration::from_millis(1))
        .unwrap();

    assert!(consumed > 0);
    assert!(written <= output.len());
}

#[cfg(feature = "std")]
#[test]
fn zero_time_limit_times_out_without_mutating_state() {
    use std::time::Duration;

    let mut compressor = TrickleCompressor::new();
    let result = compressor.compress_timed(b"input", &mut [0; 64], true, Duration::from_millis(0));

    assert_eq!(result, Err(TrickleError::TimeoutExceeded));
    assert_eq!(compressor.stats().bytes_processed, 0);
}
