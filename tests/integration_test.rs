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

    #[cfg(feature = "std")]
    #[test]
    fn test_timed_compression() {
        use std::time::Duration;
        
        let mut compressor = TrickleCompressor::new();
        let input = b"Test data for timed compression";
        let mut output = vec![0u8; input.len() * 2];
        let time_limit = Duration::from_millis(100);
        
        let result = compressor.compress_timed(input, &mut output, true, time_limit);
        assert!(result.is_ok());
        
        let (consumed, written, finished) = result.unwrap();
        assert_eq!(consumed, input.len());
        assert!(written > 0);
        assert!(finished);
    }
}
