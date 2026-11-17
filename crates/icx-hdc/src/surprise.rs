//! Titans Surprise-Gated Momentum Tracker (`arXiv:2501.00663`).
//!
//! Combines HDC associative novelty, Shannon byte entropy, diagnostic anomaly detection, and
//! exponential momentum smoothing to classify events into virtual memory retention tiers.

use crate::hv::{HdcBundler, Hypervector};
use serde::{Deserialize, Serialize};

/// Virtual memory retention tier assigned by the `TitansSurpriseGate`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetentionTier {
    /// `S_t >= 0.70`: Keep high-resolution AHTL + full diagnostic clauses in L1.
    PinHighResolution,
    /// `0.35 <= S_t < 0.70`: Fold into standard AHTL Holographic Stub + `#icx:<hash>`.
    FoldAhtlStub,
    /// `S_t < 0.35`: Compress to a minimal 1-clause micro-beacon pointing to Cold CAS.
    ColdBeacon,
}

/// Result of scoring an incoming event through the `TitansSurpriseGate`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SurpriseVerdict {
    pub instantaneous_surprise: f32,
    pub momentum_surprise: f32,
    pub tier: RetentionTier,
}

/// Online Titans Surprise-Gated Momentum Tracker (`arXiv:2501.00663`).
#[derive(Clone, Debug)]
pub struct TitansSurpriseGate {
    bundler: HdcBundler,
    momentum: f32,
    eta: f32,
    events_seen: u64,
}

impl Default for TitansSurpriseGate {
    fn default() -> Self {
        Self::new(0.82)
    }
}

impl TitansSurpriseGate {
    /// Creates a new `TitansSurpriseGate` with momentum decay factor `eta` (typically `0.82`).
    pub fn new(eta: f32) -> Self {
        Self {
            bundler: HdcBundler::new(),
            momentum: 0.5,
            eta: eta.clamp(0.1, 0.98),
            events_seen: 0,
        }
    }

    /// Computes normalized Shannon byte entropy in `[0.0, 1.0]`.
    pub fn shannon_entropy_norm(bytes: &[u8]) -> f32 {
        if bytes.is_empty() {
            return 0.0;
        }
        let mut counts = [0u32; 256];
        let sample = if bytes.len() > 4096 {
            &bytes[..4096]
        } else {
            bytes
        };
        for &b in sample {
            counts[b as usize] += 1;
        }
        let len = sample.len() as f32;
        let mut entropy = 0.0f32;
        for &c in &counts {
            if c > 0 {
                let p = (c as f32) / len;
                entropy -= p * p.log2();
            }
        }
        (entropy / 8.0).clamp(0.0, 1.0)
    }

    /// Detects whether text contains high-surprise diagnostic markers (errors, panics, failures).
    pub fn has_diagnostic_anomaly(text: &str) -> bool {
        let needles = [
            "error[E",
            "panicked at",
            "FAILED",
            "fatal:",
            "Segmentation fault",
            "IntegrityViolation",
            "thread '",
            "Exception:",
            "Traceback (most recent call last)",
        ];
        needles.iter().any(|n| text.contains(n))
    }

    /// Scores an event, updates associative memory and momentum $S_t$, and returns its `SurpriseVerdict`.
    pub fn observe(
        &mut self,
        hv: &Hypervector,
        raw_text: &str,
        force_anomaly: bool,
    ) -> SurpriseVerdict {
        let novelty = if self.events_seen == 0 {
            1.0
        } else {
            (1.0 - self.bundler.finalize().similarity(hv)).clamp(0.0, 1.0)
        };

        let entropy = Self::shannon_entropy_norm(raw_text.as_bytes());
        let is_anomaly = force_anomaly || Self::has_diagnostic_anomaly(raw_text);
        let anomaly_boost = if is_anomaly { 0.45 } else { 0.0 };

        let instantaneous = (0.65 * novelty + 0.35 * entropy + anomaly_boost).clamp(0.0, 1.0);
        self.momentum = if is_anomaly {
            instantaneous.max(self.eta * self.momentum + (1.0 - self.eta) * instantaneous)
        } else {
            (self.eta * self.momentum + (1.0 - self.eta) * instantaneous).clamp(0.0, 1.0)
        };

        let weight = ((self.momentum * 32.0).round() as i16).clamp(1, 64);
        self.bundler.add_weighted(hv, weight);
        self.events_seen += 1;

        let effective = instantaneous.max(self.momentum);
        let tier = if effective >= 0.70 {
            RetentionTier::PinHighResolution
        } else if effective >= 0.35 {
            RetentionTier::FoldAhtlStub
        } else {
            RetentionTier::ColdBeacon
        };

        SurpriseVerdict {
            instantaneous_surprise: instantaneous,
            momentum_surprise: self.momentum,
            tier,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surprise_gate_spikes_on_panic_and_maintains_momentum_for_followup_fix() {
        let mut gate = TitansSurpriseGate::default();
        let routine_hv = Hypervector::from_seed("log", "routine_build_ok");

        for _ in 0..10 {
            let _ = gate.observe(&routine_hv, "test foo ... ok\ntest bar ... ok", false);
        }

        let panic_hv = Hypervector::from_seed("err", "panic_oob_412");
        let v_panic = gate.observe(
            &panic_hv,
            "thread 'main' panicked at crates/core/src/lib.rs:412:9: index out of bounds",
            false,
        );
        assert_eq!(v_panic.tier, RetentionTier::PinHighResolution);
        assert!(v_panic.instantaneous_surprise >= 0.85);

        // Immediately following turn (fixing the bug) inherits elevated momentum
        let fix_hv = Hypervector::from_seed("edit", "fix_bounds_check_412");
        let v_fix = gate.observe(&fix_hv, "if idx < slice.len() { slice[idx] }", false);
        assert_eq!(
            v_fix.tier,
            RetentionTier::PinHighResolution,
            "momentum must keep the immediate post-panic fix in PinHighResolution"
        );
    }
}
