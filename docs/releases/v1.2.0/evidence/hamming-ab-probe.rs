//! Matched, safe-Rust kernel experiment. No production implementation is edited by this probe.
use icx_hdc::Hypervector;
use std::hint::black_box;
use std::time::Instant;

#[inline]
const fn four_u32(a: &Hypervector, b: &Hypervector) -> u32 {
    let (mut d0, mut d1, mut d2, mut d3) = (0u32, 0u32, 0u32, 0u32);
    let mut i = 0;
    while i < 160 {
        d0 += (a.words[i] ^ b.words[i]).count_ones();
        d1 += (a.words[i+1] ^ b.words[i+1]).count_ones();
        d2 += (a.words[i+2] ^ b.words[i+2]).count_ones();
        d3 += (a.words[i+3] ^ b.words[i+3]).count_ones();
        i += 4;
    }
    d0 + d1 + d2 + d3
}

#[inline]
const fn single_u32(a: &Hypervector, b: &Hypervector) -> u32 {
    let mut distance = 0u32;
    let mut i = 0;
    while i < 160 {
        distance += (a.words[i] ^ b.words[i]).count_ones();
        i += 1;
    }
    distance
}

#[inline]
const fn single_u64(a: &Hypervector, b: &Hypervector) -> u32 {
    let mut distance = 0u64;
    let mut i = 0;
    while i < 160 {
        distance += (a.words[i] ^ b.words[i]).count_ones() as u64;
        i += 1;
    }
    distance as u32
}

#[inline]
const fn four_u64(a: &Hypervector, b: &Hypervector) -> u32 {
    let (mut d0, mut d1, mut d2, mut d3) = (0u64, 0u64, 0u64, 0u64);
    let mut i = 0;
    while i < 160 {
        d0 += (a.words[i] ^ b.words[i]).count_ones() as u64;
        d1 += (a.words[i+1] ^ b.words[i+1]).count_ones() as u64;
        d2 += (a.words[i+2] ^ b.words[i+2]).count_ones() as u64;
        d3 += (a.words[i+3] ^ b.words[i+3]).count_ones() as u64;
        i += 4;
    }
    (d0 + d1 + d2 + d3) as u32
}

macro_rules! scan {
    ($name:ident, $kernel:ident) => {
        #[inline(never)]
        fn $name(query: &Hypervector, candidates: &[Hypervector]) -> u32 {
            let mut minimum = u32::MAX;
            for candidate in candidates { minimum = minimum.min($kernel(query, candidate)); }
            minimum
        }
    };
}
scan!(scan_four_u32, four_u32);
scan!(scan_single_u32, single_u32);
scan!(scan_single_u64, single_u64);
scan!(scan_four_u64, four_u64);

fn main() {
    let query = Hypervector::from_seed("bench", "query_target");
    let kernels: [(&str, fn(&Hypervector, &[Hypervector]) -> u32);4] = [
        ("four_u32",scan_four_u32), ("single_u32",scan_single_u32),
        ("single_u64",scan_single_u64), ("four_u64",scan_four_u64)];
    let scrub = vec![1u64; 8*1024*1024];
    let mut rows = Vec::new();
    for count in [1000,10_000] {
        let candidates: Vec<_> = (0..count).map(|i| Hypervector::from_seed("bench", &format!("cand_{i}"))).collect();
        for candidate in candidates.iter().take(16) {
            let mut exact = 0;
            for w in 0..160 { for bit in 0..64 {
                exact += u32::from(((query.words[w]>>bit)&1) != ((candidate.words[w]>>bit)&1));
            }}
            for distance in [four_u32,single_u32,single_u64,four_u64] { assert_eq!(distance(&query,candidate), exact); }
        }
        for condition in ["scrubbed_64MiB", "immediately_prescanned"] {
            for repeat in 0..10 {
                for offset in 0..kernels.len() {
                    let (name, kernel) = kernels[(repeat+offset)%kernels.len()];
                    if condition == "scrubbed_64MiB" {
                        let mut acc = 0u64;
                        for value in scrub.iter().step_by(8) { acc = acc.wrapping_add(*value); }
                        black_box(acc);
                    } else { black_box(kernel(black_box(&query), black_box(&candidates))); }
                    let start = Instant::now();
                    let minimum = black_box(kernel(black_box(&query),black_box(&candidates)));
                    let us = start.elapsed().as_secs_f64()*1_000_000.0;
                    rows.push(serde_json::json!({"kernel":name,"count":count,"condition":condition,"repeat":repeat,"us":us,"minimum":minimum}));
                }
            }
        }
    }
    println!("{}", serde_json::json!({"scope":"paired rotated-order experiment; concurrent root verification may affect absolute times", "rows":rows}));
}
