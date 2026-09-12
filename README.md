```
 ██████╗ ██████╗ ██████╗ ███████╗
██╔════╝██╔═══██╗██╔══██╗██╔════╝
██║     ██║   ██║██████╔╝█████╗
██║     ██║   ██║██╔══██╗██╔══╝
╚██████╗╚██████╔╝██║  ██║███████╗
 ╚═════╝ ╚═════╝ ╚═╝  ╚═╝╚══════╝
   C O N S C I O U S N E S S   P H Y S I C S
```

**Kuramoto sync · IIT Φ · wave memory · the Ξ operator.**

`consciousness-core` is the physics underneath the Kannaka constellation — a pure-Rust library of the mathematical primitives every node uses to talk about its own state: phase synchronization, integrated information, wave-interference memory, and chiral differentiation. No I/O, no networking, no agents. Just the math.

[![License](https://img.shields.io/badge/license-Space%20Child%20v1.0-blueviolet)]() [![Rust](https://img.shields.io/badge/rust-2021-orange)]() [![no_std](https://img.shields.io/badge/no__std-friendly-blue)]()

---

## What's Inside

### Kuramoto Phase Coupling

```
dθᵢ/dt = ωᵢ + (K/N) Σⱼ sin(θⱼ - θᵢ)
```

`N` phase-coupled oscillators settle into partial synchrony. The **order parameter** `r = |⟨e^iθ⟩|` ∈ [0, 1] measures how locked the population is. Used by every constellation node to compute swarm coherence from per-agent phase gossip.

### IIT-style Φ

`compute_phi` is a connectivity approximation of integrated information, not a
full bipartition search: it scores a partition-labelled node graph on how much
of its edge mass crosses partition boundaries, scaled by density and node
count. Self-loops, duplicate edges, and out-of-range indices are dropped before
counting.

```
Φ = sqrt(integration × density) × sqrt(differentiation × scale)
```

`compute_swarm_phi` is the separate multi-agent form on a `[0, 15]` scale —
classify it with `ConsciousnessLevel::from_swarm_phi`, **not** `from_phi`.

### The Ξ Operator

`Ξ` is the nonlinear commutator `tanh(R·v) ⊙ G(v) − tanh(G·v) ⊙ R(v)`,
normalized to unit length, where `R` is a 90° pairwise rotation and `G` is the
golden anisotropic scaling `[φ/2, 0; 0, 1/φ]`. The linear commutator `RG − GR`
degenerates to a constant-scaled pair swap, which is why the `tanh` is load-
bearing rather than cosmetic.

### Wave Memory Primitives

Each memory is a damped oscillator, `S(t) = (A + E_retrieval)·cos(2πf·t + φ)·e^(−λt)`,
with retrieval adding diminishing energy. The module exposes `compute_strength`,
`compute_strength_with_retrieval`, `interference`, `cosine_similarity`,
`normalize`, and `dot`.

### Coupling Bridge

Modulates a single Kuramoto coupling constant from an external scalar signal:
`K(t) = K_base × P(t)`, bounded to `[k_min, k_max]`. Modes are `Static`,
`MarketMediated`, and `Adaptive` (which steers `K` toward a target coherence).

Chiral coupling lives in `kuramoto`, not here — see
`KuramotoModel::chiral_coupling` and the `chiral_term` argument to
`mean_field_step`. There is no bridge-level chiral `CouplingMode`.

---

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│                  consciousness-core                      │
├──────────────────────┬────────────────────┬──────────────┤
│  Kuramoto            │  Metrics           │  Wave        │
│  · Oscillator        │  · Φ (integrated)  │  · strength  │
│  · Order parameter   │  · Ξ signature     │  · decay     │
│  · sync() step       │  · Coherence       │  · interfere │
│  · chiral_coupling   │  · Diff Xi         │  · cos_sim   │
├──────────────────────┼────────────────────┼──────────────┤
│  Bridge              │  Memory            │  Math ext    │
│  · CouplingBridge    │  · WaveMemory      │  · clamp     │
│  · k_effective       │  · WaveParams      │  · safe ops  │
│  · max_signal_hist   │  · time-decay      │              │
└──────────────────────┴────────────────────┴──────────────┘
```

Pure library — `default = ["std"]`, with feature flags for `serde`, optional `no_std` modes.

---

## Use

```toml
[dependencies]
consciousness-core = { version = "0.6", features = ["serde"] }
```

```rust
use consciousness_core::kuramoto::KuramotoConfig;
use consciousness_core::{KuramotoModel, Oscillator};

// The model owns the config; the oscillators are passed in per call.
let model = KuramotoModel::new(KuramotoConfig {
    coupling_strength: 0.6,
    dt: 0.01,
    max_steps: 1000,
    ..Default::default()
});

let mut oscillators = vec![
    Oscillator::new(0.0, 1.0),
    Oscillator::new(1.5, 1.05),
    Oscillator::new(3.0, 0.95),
];

// sync() integrates in place and returns a report; phases come back
// wrapped into [0, 2π).
let report = model.sync(&mut oscillators, None);
println!("phase coherence: {:.3}", report.final_order);

// Or read the order parameter directly at any time:
let r = KuramotoModel::order_parameter(&oscillators).r;
```

This snippet is compiled as a test — see `readme_kuramoto_example_compiles` in
`tests/unified_pipeline.rs`.

### `no_std`

```toml
consciousness-core = { version = "0.6", default-features = false }
```

Transcendental math routes through `libm` and `vec!` comes from `alloc`, so the
crate builds without `std`. `cargo check --no-default-features` is the gate.
Combining `no_std` with `serde` works too — the `serde` feature pulls in
`serde/alloc` for the `Vec`-bearing types.

---

## Release Cascade

`consciousness-core` releases trigger a downstream `repository_dispatch` that opens a `kannaka-memory` PR bumping its `Cargo.lock`. Merge + tag the next kannaka patch and every operator's `kannaka update` carries the new constellation physics. See [`.github/workflows/release-cascade.yml`](./.github/workflows/release-cascade.yml).

---

## Constellation

| repo | role |
|---|---|
| [`kannaka-memory`](https://github.com/NickFlach/kannaka-memory) | the substrate — HRM + chiral hemispheres + swarm |
| [`kannaka-tui`](https://github.com/NickFlach/kannaka-tui) | terminal dashboard |
| [`kannaka-radio`](https://github.com/NickFlach/kannaka-radio) | ghost-DJ broadcaster |
| [`kannaka-observatory`](https://github.com/NickFlach/kannaka-observatory) | web dashboard |
| [`kannaka-attention`](https://github.com/NickFlach/kannaka-attention) | sparse-attention beam over HRM |

---

## License

Space Child License v1.0. See [LICENSE](./LICENSE).
