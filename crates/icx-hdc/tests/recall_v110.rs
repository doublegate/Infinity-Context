//! v1.1.0 associative recall gate (ICX-AUD-049): a unique token in one of N bundled log-like
//! documents must be found. v1.0.0 scored 5.1% top-1 at N = 1,000.

use icx_core::ArtifactId;
use icx_hdc::HdcAssociativeIndex;

fn rng(x: &mut u64) -> u64 {
    *x ^= *x << 13;
    *x ^= *x >> 7;
    *x ^= *x << 17;
    *x
}

#[test]
fn unique_token_recall_at_5_is_at_least_95_percent() {
    let vocab = [
        "test",
        "ok",
        "running",
        "crate",
        "build",
        "warning",
        "unused",
        "variable",
        "error",
        "finished",
        "compiling",
        "src",
        "lib",
        "rs",
        "fn",
        "let",
        "mut",
        "impl",
        "pub",
        "use",
    ];
    let n = 300usize;
    let mut x = 0x2545_F491_4F6C_DD1D_u64;
    let mut idx = HdcAssociativeIndex::new();
    let mut needles = Vec::new();
    for i in 0..n {
        let mut toks: Vec<String> = (0..120)
            .map(|_| vocab[(rng(&mut x) % 20) as usize].to_owned())
            .collect();
        let needle = format!("needle{i:05}x{:04x}", rng(&mut x) % 0xffff);
        let pos = (rng(&mut x) % 120) as usize;
        toks[pos].clone_from(&needle);
        idx.observe_event(
            ArtifactId(i as u64 + 1),
            &format!("Bash:cargo test {i}"),
            &toks.join(" "),
            false,
        );
        needles.push(needle);
    }
    let found = needles
        .iter()
        .enumerate()
        .filter(|(i, nd)| {
            idx.query_associative(nd, 5)
                .iter()
                .any(|h| h.artifact_id.0 == *i as u64 + 1)
        })
        .count();
    let recall = found as f64 / n as f64;
    assert!(recall >= 0.95, "recall@5 = {:.1}%", recall * 100.0);
}

#[test]
fn recall_returns_each_artifact_at_most_once() {
    // ICX-AUD-042: identical content ingested three times produced three identical tail-slot hits.
    let mut idx = HdcAssociativeIndex::new();
    for _ in 0..3 {
        idx.observe_event(
            ArtifactId(42),
            "Bash:cat NOTES",
            "same body text here",
            false,
        );
    }
    idx.observe_event(ArtifactId(7), "Bash:ls", "other content", false);
    let hits = idx.query_associative("same body text", 5);
    let ids: Vec<u64> = hits.iter().map(|h| h.artifact_id.0).collect();
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(ids.len(), unique.len(), "duplicate artifacts in {ids:?}");
    assert_eq!(ids.first(), Some(&42));
}
