use icx_core::chunker::{ChunkerConfig, chunk_bytes};

#[test]
fn extreme_chunker_sizes_preserve_all_input_without_overflow() {
    let data = vec![0u8; 400];
    for config in [
        ChunkerConfig {
            min_size: usize::MAX,
            avg_size: usize::MAX,
            max_size: usize::MAX,
        },
        ChunkerConfig {
            min_size: 64,
            avg_size: usize::MAX,
            max_size: usize::MAX,
        },
        ChunkerConfig {
            min_size: 0,
            avg_size: 0,
            max_size: 0,
        },
    ] {
        let chunks = chunk_bytes(&data, config);
        let recovered: Vec<_> = chunks
            .iter()
            .flat_map(|&(start, len)| data[start..start + len].iter().copied())
            .collect();
        assert_eq!(recovered, data);
    }
}
