//! Parallel exact Hamming scan (v1.2.0 release review): results must equal the sequential scan
//! for every input size, including the sizes on both sides of the threading threshold.

use icx_hdc::Hypervector;
use icx_hdc::hv::{PARALLEL_SCAN_MIN, hamming_distances, nearest};

fn corpus(n: usize) -> Vec<Hypervector> {
    (0..n)
        .map(|i| Hypervector::from_seed("parallel-scan", &format!("node_{i}")))
        .collect()
}

#[test]
fn parallel_distances_equal_sequential_distances_at_every_size() {
    let query = Hypervector::from_seed("parallel-scan", "query");
    for n in [
        0,
        1,
        7,
        PARALLEL_SCAN_MIN - 1,
        PARALLEL_SCAN_MIN,
        PARALLEL_SCAN_MIN + 1,
        10_000,
    ] {
        let nodes = corpus(n);
        let sequential: Vec<u32> = nodes.iter().map(|c| query.hamming_distance(c)).collect();
        assert_eq!(hamming_distances(&query, &nodes), sequential, "n={n}");
    }
}

#[test]
fn nearest_returns_the_first_minimum_like_a_sequential_scan() {
    let query = Hypervector::from_seed("parallel-scan", "query");
    let mut nodes = corpus(10_000);
    assert_eq!(nearest(&query, &[]), None);
    // Plant two exact matches; the earlier index must win, as in a sequential `min` scan.
    nodes[7_321] = query;
    nodes[9_999] = query;
    assert_eq!(nearest(&query, &nodes), Some((7_321, 0)));

    let nodes = corpus(PARALLEL_SCAN_MIN * 3);
    let expected = nodes
        .iter()
        .enumerate()
        .map(|(i, c)| (query.hamming_distance(c), i))
        .min()
        .map(|(d, i)| (i, d));
    assert_eq!(nearest(&query, &nodes), expected);
}
