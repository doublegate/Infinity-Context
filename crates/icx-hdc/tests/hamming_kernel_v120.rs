//! Independent bit-by-bit distance oracle protects kernel optimization exactness.
use icx_hdc::Hypervector;

// Compiling this initializer is part of the public const-evaluation contract.
const CONST_OPPOSITE_DISTANCE: u32 =
    Hypervector { words: [0; 160] }.hamming_distance_simd(&Hypervector {
        words: [u64::MAX; 160],
    });

fn bit_oracle(left: &Hypervector, right: &Hypervector) -> u32 {
    let mut distance = 0;
    for word in 0..160 {
        for bit in 0..64 {
            distance +=
                u32::from(((left.words[word] >> bit) & 1) != ((right.words[word] >> bit) & 1));
        }
    }
    distance
}

#[test]
fn distance_kernel_matches_individual_bits_for_dense_sparse_and_asymmetric_vectors() {
    assert_eq!(CONST_OPPOSITE_DISTANCE, 10_240);
    let zero = Hypervector::zero();
    let ones = Hypervector {
        words: [u64::MAX; 160],
    };
    let alternating = Hypervector {
        words: std::array::from_fn(|i| {
            if i % 2 == 0 {
                0x5555_5555_5555_5555
            } else {
                0xaaaa_aaaa_aaaa_aaaa
            }
        }),
    };
    let mut tail = zero;
    tail.words[159] = 1u64 << 63;
    assert_eq!(zero.hamming_distance_simd(&ones), 10_240);
    assert_eq!(zero.hamming_distance_simd(&tail), 1);
    let mut vectors = vec![zero, ones, alternating, tail];
    for seed in 0..12 {
        vectors.push(Hypervector::from_u64_seed(seed * 0x10203));
    }
    for left in &vectors {
        for right in &vectors {
            assert_eq!(left.hamming_distance_simd(right), bit_oracle(left, right));
        }
    }
}
