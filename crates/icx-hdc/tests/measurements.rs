// SPDX-FileCopyrightText: 2026 DoubleGate
// SPDX-License-Identifier: GPL-3.0-or-later AND AGPL-3.0-or-later

//! Measurements published in `docs/numbers.md` (v1.5.0): run with
//! `cargo test --release -p icx-hdc --test measurements -- --ignored --nocapture`.

use icx_core::ArtifactId;
use icx_hdc::HdcAssociativeIndex;

/// A small deterministic generator for the measurement corpora (xorshift64*).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Builds `n` events over a `vocab`-word vocabulary: subject `Bash:run-<a>-<b>`, content of
/// 24 words. Returns the index and each event's words, for building queries.
fn synthetic_corpus(n: usize, vocab: usize, seed: u64) -> (HdcAssociativeIndex, Vec<Vec<usize>>) {
    let mut rng = Rng(seed);
    let mut idx = HdcAssociativeIndex::new();
    let mut words = Vec::with_capacity(n);
    for i in 0..n {
        let w: Vec<usize> = (0..24).map(|_| rng.below(vocab)).collect();
        let content: Vec<String> = w.iter().map(|x| format!("w{x}")).collect();
        idx.observe_event(
            ArtifactId(i as u64 + 1),
            &format!("Bash:run-{}-{}", w[0], w[1]),
            &content.join(" "),
            false,
        );
        words.push(w);
    }
    (idx, words)
}

/// T-71-003 measurement (run with `--ignored --nocapture` in release): stage-1 recall, the
/// share of queries whose target survives the prescreen and postings union, and final
/// recall@1/@5, at 1k, 10k and 100k artifacts. Two query kinds: three of the target's words
/// from a 50,000-word vocabulary (rare tokens, df below the cap) and from a 500-word
/// vocabulary (common tokens, df above the cap, so only the prescreen can keep the target).
#[test]
#[ignore = "measurement; prints a table"]
fn measure_stage1_recall_by_corpus_size() {
    println!(
        "| prescreen | n | vocabulary | queries | max candidates | artifacts with all 3 words (mean) | stage-1 recall | recall@1 | recall@5 | mean query |"
    );
    println!("| :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |");
    for width in [16, 32] {
        for n in [1_000usize, 10_000, 100_000] {
            for vocab in [50_000usize, 500] {
                let (idx, words) = synthetic_corpus(n, vocab, 0x1c5_0071 ^ n as u64);
                let mut rng = Rng(0xfeed ^ n as u64 ^ vocab as u64);
                let queries = 200;
                let (mut s1, mut r1, mut r5, mut ties) = (0, 0, 0, 0usize);
                let mut cap = 0;
                // Only `query_associative` is timed; the inspection of stage 1 and the count of
                // ambiguous artifacts run outside the timer (review of #97).
                let mut timed = std::time::Duration::ZERO;
                for _ in 0..queries {
                    let t = rng.below(n);
                    // Three distinct positions of the target's words (sampled without
                    // replacement, review of #97).
                    let mut pos: Vec<usize> = Vec::with_capacity(3);
                    while pos.len() < 3 {
                        let p = 2 + rng.below(22);
                        if !pos.contains(&p) {
                            pos.push(p);
                        }
                    }
                    let q: Vec<String> = pos.iter().map(|&p| format!("w{}", words[t][p])).collect();
                    let query = q.join(" ");
                    let qw: Vec<usize> = q.iter().map(|w| w[1..].parse().unwrap()).collect();
                    ties += words
                        .iter()
                        .filter(|d| qw.iter().all(|x| d.contains(x)))
                        .count();
                    let (kept, cands) = idx.measure_prescreen(&query, 5, width, t);
                    cap = cap.max(cands);
                    s1 += usize::from(kept);
                    // Timed alone: the ranking, as `query_associative` computes it.
                    let started = std::time::Instant::now();
                    let hits = idx.measure_query(&query, 5, width);
                    timed += started.elapsed();
                    let target = ArtifactId(t as u64 + 1);
                    r1 += usize::from(hits.first().is_some_and(|h| h.artifact_id == target));
                    r5 += usize::from(hits.iter().any(|h| h.artifact_id == target));
                }
                let per = timed.as_secs_f64() * 1e3 / f64::from(queries);
                let pct = |x: usize| 100.0 * x as f64 / f64::from(queries);
                println!(
                    "| {} bits | {n} | {vocab} | {queries} | {cap} | {:.1} | {:.1}% | {:.1}% | {:.1}% | {per:.2} ms |",
                    width * 64,
                    ties as f64 / f64::from(queries),
                    pct(s1),
                    pct(r1),
                    pct(r5)
                );
            }
        }
    }
}

/// ICX-AUD-052 measurement (run with `--ignored --nocapture` in release): over 20,000 events,
/// how many earlier entries the coordinates alone report as sharing the latest history, and
/// how many the confirmed suffix does. Every coordinate-level claim here is false, since no
/// event repeats.
#[test]
#[ignore = "measurement; prints counts"]
fn measure_fractal_suffix_false_positives() {
    for (label, similar) in [("distinct", false), ("similar", true)] {
        let mut idx = HdcAssociativeIndex::new();
        let n = 20_000usize;
        for i in 0..n {
            let (subj, content) = if similar {
                (
                    "Bash:cargo test".to_owned(),
                    format!("test result: ok. {i} passed"),
                )
            } else {
                (
                    format!("cmd-{i}"),
                    format!("payload {} {}", i * 7919, i ^ 0x5a5a),
                )
            };
            idx.observe_event(ArtifactId(i as u64), &subj, &content, false);
        }
        let (coord, exact) = idx.measure_suffix_claims();
        println!("{label}: n={n} coordinate-level claims={coord} confirmed={exact}");
        assert_eq!(exact, 0);
    }
}
