//! Independent literals pin the shipped scale instead of relying only on self-roundtrips.
use icx_hdc::FractalCoord;

#[test]
fn shipped_profile_a_q15_scale_remains_backward_compatible() {
    for (coordinate, word) in [
        (-1.0, "8001"),
        (-0.5, "c000"),
        (0.0, "0000"),
        (0.5, "4000"),
        (1.0, "7fff"),
    ] {
        let coord = FractalCoord {
            coords: [coordinate; 16],
            depth: 1,
        };
        assert_eq!(coord.encode_fc_hex64(), word.repeat(16));
        let decoded = FractalCoord::decode_fc_hex64(&word.repeat(16), 1).unwrap();
        assert!((decoded.coords[0] - coordinate).abs() <= 1.0 / 32767.0);
    }
    // Historical unversioned `2000` must keep its shipped Q15 meaning (~0.25), never silently
    // acquire the Q1.14 meaning0.5. Q1.14 remains an explicitly separate arithmetic API.
    assert!(
        (FractalCoord::decode_fc_hex64(&"2000".repeat(16), 1)
            .unwrap()
            .coords[0]
            - 0.25)
            .abs()
            < 0.00001
    );
    assert_eq!(
        FractalCoord {
            coords: [0.5; 16],
            depth: 1
        }
        .to_q1_14(),
        [8192; 16]
    );
}
