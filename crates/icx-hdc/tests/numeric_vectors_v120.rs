use icx_hdc::{FractalCoord, Hypervector, TitansSurpriseGate};

#[test]
fn splitmix_seed_and_sparse_rotating_sketch_match_independent_vectors() {
    // SplitMix64 seed0 published recurrence; literals independently calculated with integer math.
    assert_eq!(
        Hypervector::from_u64_seed(0).words[..3],
        [
            0xe220_a839_7b1d_cdaf,
            0x6e78_9e6a_a1b9_65f4,
            0x06c4_5d18_8009_454f
        ]
    );
    let mut sparse = Hypervector::zero();
    sparse.words[0] = 1;
    sparse.words[1] = 1;
    sparse.words[4] = 1;
    sparse.words[65] = 2;
    // Channel0:1 XOR rotate(1,4)=17; channel1:rotate(1,1) XOR rotate(2,1)=6.
    assert_eq!(sparse.sketch_256(), [17, 6, 0, 0]);
}

#[test]
fn morton_planes_and_coordinate_digest_have_literal_expected_values() {
    assert_eq!(FractalCoord::default().morton_z64(), 0xffff_0000_0000_0000);
    assert_eq!(
        FractalCoord {
            coords: [-0.5; 16],
            depth: 0
        }
        .morton_z64(),
        0x0000_ffff_0000_0000
    );
    let mut one = FractalCoord {
        coords: [-1.0; 16],
        depth: 0,
    };
    one.coords[0] = 0.5; // Bin12=1100; dimension0 appears in bitplanes2and3.
    assert_eq!(one.morton_z64(), 0x0001_0001_0000_0000);
    let zero = FractalCoord {
        coords: [0.0; 16],
        depth: 7,
    };
    // FNV1a over32zero bytes, XORdepth7, independently calculated modulo2^32.
    assert_eq!(zero.digest_u32(), 0x0b2a_e442);
}

#[test]
fn surprise_momentum_matches_a_hand_calculated_zero_entropy_sequence() {
    let mut gate = TitansSurpriseGate::new(0.5);
    let hv = Hypervector::from_u64_seed(0);
    let first = gate.observe(&hv, "aaaaaaaa", false);
    let second = gate.observe(&hv, "aaaaaaaa", false);
    assert!((first.instantaneous_surprise - 0.65).abs() < 0.00001);
    // Initial momentum0.5: first=0.5*0.5+0.5*0.65, then second=0.5*0.575.
    assert!((first.momentum_surprise - 0.575).abs() < 0.00001);
    assert!((second.momentum_surprise - 0.2875).abs() < 0.00001);
    let mut near = hv;
    near.words[0] ^= 1;
    let third = gate.observe(&near, "aaaaaaaa", false);
    // New fingerprint, one differing bit: similarity=1-1/5120 and novelty=1/5120.
    assert!((third.instantaneous_surprise - 0.65 / 5120.0).abs() < 0.000_001);
}
