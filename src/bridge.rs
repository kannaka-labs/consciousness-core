//! Market-mediated coupling bridge.
//!
//! Introduces external signal modulation to Kuramoto coupling:
//!
//! ```text
//! K(t) = K_base × P_market(t)
//! ```
//!
//! Where P_market is an external signal (e.g. market sentiment, coherence proxy)
//! that modulates the base coupling strength. This allows consciousness
//! synchronization to be influenced by environmental signals.
//!
//! Supports multiple coupling modes:
//! - **Static**: K(t) = K_base (constant)
//! - **MarketMediated**: K(t) = K_base × P(t)
//! - **Adaptive**: K adjusts toward a target coherence level

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

/// Coupling mode for the bridge.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CouplingMode {
    /// Constant coupling: K(t) = K_base
    Static,
    /// Market-mediated: K(t) = K_base × P_market
    MarketMediated,
    /// Adaptive: K adjusts toward target coherence
    Adaptive,
}

/// Configuration for the coupling bridge.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BridgeConfig {
    /// Base coupling strength K_base
    pub k_base: f32,
    /// Adaptive rate (how fast K adjusts)
    pub adaptive_rate: f32,
    /// Target coherence for adaptive mode
    pub target_coherence: f32,
    /// Min/max bounds for effective coupling
    pub k_min: f32,
    pub k_max: f32,
    /// Maximum number of signal samples kept for `mean_signal` diagnostics.
    /// Older entries are discarded ring-buffer-style. The docstring on
    /// `signal_history` advertised a windowed history but the previous
    /// implementation appended forever — long-running bridges leaked
    /// memory until process restart (#14).
    pub max_signal_history: usize,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            k_base: 0.5,
            adaptive_rate: 0.01,
            target_coherence: 0.8,
            k_min: 0.1,
            k_max: 5.0,
            // 1024 samples is enough for any sensible mean-signal window
            // and bounds worst-case memory at ~4 KiB per bridge.
            max_signal_history: 1024,
        }
    }
}

/// The coupling bridge — modulates Kuramoto coupling with external signals.
pub struct CouplingBridge {
    pub config: BridgeConfig,
    pub mode: CouplingMode,
    /// Current effective coupling strength
    pub k_effective: f32,
    /// History of market signals for diagnostics
    signal_history: Vec<f32>,
}

/// Clamp `k` into the configured coupling bounds without panicking.
///
/// `f32::clamp` panics when `min > max` (#81), and `BridgeConfig` is a plain
/// struct with public fields, so an inverted range can arrive from a caller
/// or be written later. The bounds are taken in order here (the smaller is
/// the floor). If both are NaN there is no usable range and `k` is returned
/// unchanged; a single NaN bound is ignored by `f32::min`/`max`.
fn clamp_to_bounds(k: f32, config: &BridgeConfig) -> f32 {
    let lo = config.k_min.min(config.k_max);
    let hi = config.k_min.max(config.k_max);
    if lo.is_nan() || hi.is_nan() {
        return k;
    }
    k.clamp(lo, hi)
}

impl CouplingBridge {
    pub fn new(config: BridgeConfig, mode: CouplingMode) -> Self {
        // Clamp k_effective to [k_min, k_max] on construction so
        // `coupling()` honors the configured bounds before the first
        // `update()` call. Previously a BridgeConfig with k_base outside
        // the configured range exposed an out-of-range initial value (#11).
        let k = clamp_to_bounds(config.k_base, &config);
        Self {
            config,
            mode,
            k_effective: k,
            signal_history: Vec::new(),
        }
    }

    /// Get the current effective coupling strength.
    pub fn coupling(&self) -> f32 {
        self.k_effective
    }

    /// Update coupling based on an external market/environmental signal.
    ///
    /// - **Static**: ignores signal, returns K_base
    /// - **MarketMediated**: K(t) = K_base × signal, clamped to [k_min, k_max]
    /// - **Adaptive**: adjusts K toward target coherence using current_coherence
    pub fn update(&mut self, signal: f32, current_coherence: f32) -> f32 {
        // Only record finite signals (#22). update() used to append the raw
        // signal before branching on mode, so a single non-finite sample
        // made mean_signal() return NaN forever — even in Static mode, which
        // is documented to ignore signals entirely. Diagnostics now stay
        // finite regardless of garbage input; an all-invalid history simply
        // collapses to the neutral 1.0 mean_signal() already returns when
        // empty.
        if signal.is_finite() {
            self.signal_history.push(signal);
            // Bounded ring-buffer behavior — drop the oldest sample once we
            // exceed the configured window. Without this `mean_signal`
            // grew toward the all-time mean instead of the windowed mean
            // its docstring promised, and memory leaked unbounded (#14).
            if self.config.max_signal_history > 0
                && self.signal_history.len() > self.config.max_signal_history
            {
                let drop = self.signal_history.len() - self.config.max_signal_history;
                self.signal_history.drain(0..drop);
            }
        }

        self.k_effective = match self.mode {
            // #77: Static ignores the signal but still honours the bounds,
            // as the constructor already does.
            CouplingMode::Static => clamp_to_bounds(self.config.k_base, &self.config),
            CouplingMode::MarketMediated => {
                // `f32::clamp` propagates NaN rather than bounding it, so a
                // non-finite signal used to return NaN coupling for the
                // affected tick (#41). The mean_signal() half of that issue
                // was already fixed above by not storing the sample; this
                // covers the coupling half. A garbage sample means "no new
                // information", so hold the last good coupling.
                if signal.is_finite() {
                    clamp_to_bounds(self.config.k_base * signal, &self.config)
                } else if self.k_effective.is_finite() {
                    self.k_effective
                } else {
                    clamp_to_bounds(self.config.k_base, &self.config)
                }
            }
            CouplingMode::Adaptive => {
                // Reject a non-finite coherence sample (#19). Without this, a
                // single NaN current_coherence makes `error`, then `new_k`,
                // NaN; f32::clamp preserves NaN, so k_effective would stay
                // NaN permanently and every later valid update would build on
                // the poisoned value. Hold the last good coupling on invalid
                // input (falling back to k_base if k_effective itself was
                // somehow corrupted) so the bridge can recover.
                let safe_prev = if self.k_effective.is_finite() {
                    self.k_effective
                } else {
                    self.config.k_base
                };
                if current_coherence.is_finite() {
                    let error = self.config.target_coherence - current_coherence;
                    clamp_to_bounds(safe_prev + self.config.adaptive_rate * error, &self.config)
                } else {
                    clamp_to_bounds(safe_prev, &self.config)
                }
            }
        };

        self.k_effective
    }

    /// Get the mean market signal over the history window.
    pub fn mean_signal(&self) -> f32 {
        if self.signal_history.is_empty() {
            return 1.0;
        }
        self.signal_history.iter().sum::<f32>() / self.signal_history.len() as f32
    }

    /// Clear signal history.
    pub fn reset_history(&mut self) {
        self.signal_history.clear();
    }
}

impl Default for CouplingBridge {
    fn default() -> Self {
        Self::new(BridgeConfig::default(), CouplingMode::Static)
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_mode_ignores_signal() {
        let mut bridge = CouplingBridge::new(
            BridgeConfig {
                k_base: 1.0,
                ..Default::default()
            },
            CouplingMode::Static,
        );
        let k = bridge.update(999.0, 0.5);
        assert_eq!(k, 1.0, "static mode should ignore signal");
    }

    #[test]
    fn market_mediated_scales_by_signal() {
        let mut bridge = CouplingBridge::new(
            BridgeConfig {
                k_base: 1.0,
                k_min: 0.0,
                k_max: 10.0,
                ..Default::default()
            },
            CouplingMode::MarketMediated,
        );
        let k = bridge.update(2.0, 0.5);
        assert!(
            (k - 2.0).abs() < 1e-5,
            "K = K_base × signal = 1 × 2 = 2, got {}",
            k
        );
    }

    #[test]
    fn market_mediated_clamped() {
        let mut bridge = CouplingBridge::new(
            BridgeConfig {
                k_base: 1.0,
                k_min: 0.1,
                k_max: 5.0,
                ..Default::default()
            },
            CouplingMode::MarketMediated,
        );
        let k = bridge.update(100.0, 0.5);
        assert_eq!(k, 5.0, "should clamp to k_max");

        let k = bridge.update(0.001, 0.5);
        assert_eq!(k, 0.1, "should clamp to k_min");
    }

    #[test]
    fn adaptive_increases_when_below_target() {
        let mut bridge = CouplingBridge::new(
            BridgeConfig {
                k_base: 1.0,
                adaptive_rate: 0.1,
                target_coherence: 0.8,
                ..Default::default()
            },
            CouplingMode::Adaptive,
        );
        let initial = bridge.coupling();
        let k = bridge.update(1.0, 0.3); // coherence < target
        assert!(
            k > initial,
            "should increase coupling when below target: {} → {}",
            initial,
            k
        );
    }

    #[test]
    fn adaptive_decreases_when_above_target() {
        let mut bridge = CouplingBridge::new(
            BridgeConfig {
                k_base: 1.0,
                adaptive_rate: 0.1,
                target_coherence: 0.5,
                ..Default::default()
            },
            CouplingMode::Adaptive,
        );
        let initial = bridge.coupling();
        let k = bridge.update(1.0, 0.9); // coherence > target
        assert!(
            k < initial,
            "should decrease coupling when above target: {} → {}",
            initial,
            k
        );
    }

    #[test]
    fn mean_signal_tracks_history() {
        let mut bridge = CouplingBridge::new(BridgeConfig::default(), CouplingMode::MarketMediated);
        bridge.update(1.0, 0.5);
        bridge.update(3.0, 0.5);
        assert!((bridge.mean_signal() - 2.0).abs() < 1e-5);
    }

    #[test]
    fn reset_clears_history() {
        let mut bridge = CouplingBridge::default();
        bridge.update(1.0, 0.5);
        bridge.reset_history();
        assert!(
            (bridge.mean_signal() - 1.0).abs() < 1e-5,
            "empty history → default 1.0"
        );
    }

    #[test]
    fn new_clamps_k_effective_to_bounds() {
        // Regression for #11 — k_base outside [k_min, k_max] used to land
        // verbatim in k_effective until the first update().
        let above = CouplingBridge::new(
            BridgeConfig {
                k_base: 99.0,
                k_min: 0.1,
                k_max: 5.0,
                ..Default::default()
            },
            CouplingMode::Static,
        );
        assert_eq!(
            above.coupling(),
            5.0,
            "k_base above k_max must clamp at construction"
        );

        let below = CouplingBridge::new(
            BridgeConfig {
                k_base: -1.0,
                k_min: 0.1,
                k_max: 5.0,
                ..Default::default()
            },
            CouplingMode::Static,
        );
        assert_eq!(
            below.coupling(),
            0.1,
            "k_base below k_min must clamp at construction"
        );
    }

    #[test]
    fn static_update_stays_inside_the_bounds() {
        // Regression for #77: the constructor clamped, then the first Static
        // update wrote the raw k_base back.
        let mut bridge = CouplingBridge::new(
            BridgeConfig {
                k_base: 10.0,
                k_min: 0.1,
                k_max: 5.0,
                ..Default::default()
            },
            CouplingMode::Static,
        );
        assert_eq!(bridge.coupling(), 5.0);
        assert_eq!(
            bridge.update(123.0, 0.5),
            5.0,
            "Static ignores the signal but not the bounds"
        );
        assert_eq!(bridge.coupling(), 5.0);
    }

    #[test]
    fn inverted_bounds_do_not_panic_in_any_mode() {
        // Regression for #81: f32::clamp panics when min > max, at
        // construction and on every update.
        for mode in [
            CouplingMode::Static,
            CouplingMode::MarketMediated,
            CouplingMode::Adaptive,
        ] {
            let mut bridge = CouplingBridge::new(
                BridgeConfig {
                    k_base: 1.0,
                    k_min: 2.0,
                    k_max: 1.0,
                    ..Default::default()
                },
                mode,
            );
            for (signal, coherence) in [(1.0, 0.5), (10.0, 0.0), (0.0, 1.0), (f32::NAN, f32::NAN)] {
                let k = bridge.update(signal, coherence);
                assert!(
                    (1.0..=2.0).contains(&k),
                    "{mode:?}: k={k} must lie in the ordered range [1, 2]"
                );
            }
        }
        // Both bounds NaN: no usable range, and still no panic.
        let mut bridge = CouplingBridge::new(
            BridgeConfig {
                k_base: 1.5,
                k_min: f32::NAN,
                k_max: f32::NAN,
                ..Default::default()
            },
            CouplingMode::MarketMediated,
        );
        assert_eq!(bridge.update(2.0, 0.5), 3.0);
    }

    #[test]
    fn adaptive_recovers_from_non_finite_coherence() {
        // Regression for #19 — a single NaN coherence made error → new_k →
        // NaN, and f32::clamp preserves NaN, so k_effective stayed NaN
        // permanently. Now a non-finite coherence holds the last good
        // coupling and later valid updates recover.
        let mut bridge = CouplingBridge::new(
            BridgeConfig {
                k_base: 1.0,
                adaptive_rate: 0.1,
                target_coherence: 0.8,
                ..Default::default()
            },
            CouplingMode::Adaptive,
        );
        let first = bridge.update(1.0, f32::NAN);
        assert!(
            first.is_finite(),
            "NaN coherence must not poison coupling: {first}"
        );
        let second = bridge.update(1.0, 0.3);
        assert!(
            second.is_finite(),
            "coupling must recover after a valid update: {second}"
        );
        assert!(bridge.coupling().is_finite());
    }

    #[test]
    fn adaptive_holds_coupling_on_non_finite_coherence() {
        // A non-finite coherence should freeze coupling at its last good
        // value rather than drift (#19).
        let mut bridge = CouplingBridge::new(
            BridgeConfig {
                k_base: 1.0,
                adaptive_rate: 0.1,
                target_coherence: 0.8,
                ..Default::default()
            },
            CouplingMode::Adaptive,
        );
        let before = bridge.coupling();
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let k = bridge.update(1.0, bad);
            assert!(
                (k - before).abs() < 1e-6,
                "coupling should hold on coherence {bad}: {before} → {k}"
            );
        }
    }

    #[test]
    fn static_mode_stays_diagnostically_finite() {
        // Regression for #22 — update() recorded the raw signal before
        // branching on mode, so a NaN signal poisoned mean_signal() even
        // though Static mode ignores signals for coupling. Non-finite
        // signals are no longer stored.
        let mut bridge = CouplingBridge::new(BridgeConfig::default(), CouplingMode::Static);
        let k = bridge.update(f32::NAN, 0.5);
        assert!(k.is_finite(), "static coupling stays finite: {k}");
        assert!(
            bridge.mean_signal().is_finite(),
            "mean_signal must ignore non-finite signals; got {}",
            bridge.mean_signal()
        );
        // A NaN-only history collapses to the neutral empty-history default.
        assert!((bridge.mean_signal() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn mean_signal_skips_non_finite_samples() {
        // A finite sample interleaved with garbage still averages cleanly
        // over just the finite samples (#22).
        let mut bridge = CouplingBridge::new(BridgeConfig::default(), CouplingMode::MarketMediated);
        bridge.update(2.0, 0.5);
        bridge.update(f32::NAN, 0.5);
        bridge.update(4.0, 0.5);
        bridge.update(f32::INFINITY, 0.5);
        assert!(
            (bridge.mean_signal() - 3.0).abs() < 1e-5,
            "mean over finite samples 2 and 4 = 3, got {}",
            bridge.mean_signal()
        );
    }

    #[test]
    fn market_mediated_holds_coupling_on_non_finite_signal() {
        // Regression for #41 — f32::clamp propagates NaN instead of
        // bounding it, so `k_base * NaN` came straight back out of
        // update() as NaN coupling for the affected tick.
        let mut bridge = CouplingBridge::new(
            BridgeConfig {
                k_base: 1.0,
                k_min: 0.1,
                k_max: 5.0,
                ..Default::default()
            },
            CouplingMode::MarketMediated,
        );
        let good = bridge.update(2.0, 0.5);
        assert!((good - 2.0).abs() < 1e-5);
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let k = bridge.update(bad, 0.5);
            assert!(
                k.is_finite(),
                "signal {bad} must not yield NaN coupling: {k}"
            );
            assert!(
                (k - good).abs() < 1e-6,
                "coupling should hold at the last good value on signal {bad}: {good} → {k}"
            );
        }
        // A later valid sample still steers coupling normally.
        let recovered = bridge.update(3.0, 0.5);
        assert!((recovered - 3.0).abs() < 1e-5);
        assert!(bridge.mean_signal().is_finite());
    }

    #[test]
    fn signal_history_is_bounded() {
        // Regression for #14 — history grew forever, leaking memory and
        // making mean_signal drift toward the all-time mean. Push more
        // than `max_signal_history` samples and confirm the window holds.
        let mut bridge = CouplingBridge::new(
            BridgeConfig {
                max_signal_history: 4,
                ..Default::default()
            },
            CouplingMode::MarketMediated,
        );
        for v in [10.0, 20.0, 30.0, 40.0, 50.0, 60.0] {
            bridge.update(v, 0.5);
        }
        // The last 4 samples (30, 40, 50, 60) average to 45.
        assert!(
            (bridge.mean_signal() - 45.0).abs() < 1e-5,
            "mean over the windowed last 4 samples, got {}",
            bridge.mean_signal()
        );
    }
}
