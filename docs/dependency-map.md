# consciousness-core — Dependency Map

**Date:** 2026-05-02
**Scope:** every constellation surface that depends on this crate's
public API, directly or transitively.

---

## What this crate exports

5 modules, ~1,584 LOC, single Rust crate. Public surface from `lib.rs`:

| Module | Symbols | Purpose |
|---|---|---|
| `kuramoto` | `KuramotoModel`, `Oscillator`, `SyncReport`, `OrderParameter` | Phase oscillator dynamics, sync detection |
| `iit` | `PhiReport`, `ConsciousnessLevel` | Integrated Information Theory's Φ + level enum |
| `wave` | `WaveParams`, `WaveMemory` | Wave-memory primitives for HRM |
| `bridge` | `CouplingBridge`, `CouplingMode` | Cross-module coupling (Kuramoto ⇄ wave) |
| `metrics` | `ConsciousnessMetrics`, `XiSignature` | Aggregate metrics + Xi operator signature |

Cargo features: `std` (default), `serde` (downstream-enabled for HRM
snapshot serialization).

---

## Direct dependents (Rust, compile-time)

```
consciousness-core
    │
    └── kannaka-memory (Cargo dep, features = ["serde"])
            │
            ├── src/consciousness.rs      ─── re-exports ConsciousnessLevel,
            │                                  ConsciousnessMetrics, etc.
            ├── src/wave.rs               ─── extends consciousness_core::wave
            │                                  with HRM-specific methods
            ├── src/xi_operator.rs        ─── re-exports metrics::XiSignature
            ├── src/bridge.rs             ─── re-exports ConsciousnessLevel
            ├── src/medium/types.rs       ─── ConsciousnessState, EmergenceLevel
            ├── src/medium/consciousness.rs ── ConsciousnessMetrics composition
            ├── src/medium/persistence.rs ─── snapshot serialization (uses serde)
            ├── src/medium/mod.rs         ─── re-export shim for downstream
            └── src/bin/kannaka.rs        ─── observe/assess/dream commands
                                              that surface metrics
```

13 files in kannaka-memory touch consciousness-core, directly or via the
`crate::consciousness` re-export shim.

**Risk:** the `crate::consciousness` shim is the only stable layer. Some
files import directly from `consciousness_core::` (e.g. `wave.rs`,
`xi_operator.rs`). A field rename in the upstream crate breaks those
files specifically. The shim should be the *only* import path.

---

## Indirect dependents (cross-process, via NATS + JSON)

`kannaka-memory` publishes consciousness data to NATS. Multiple consumers
deserialize the JSON shapes. **None of them validate against a schema.**
A field rename or type change in the upstream Rust types propagates as
silent breakage across the constellation.

```
kannaka-memory (Rust)
    │
    │  publishes to NATS subjects:
    │    KANNAKA.consciousness   (phi, xi, mean_order, num_clusters,
    │                             consciousness_level, …)
    │    KANNAKA.dreams          (memories_strengthened, …, hallucinations)
    │    QUEEN.phase.<agent>     (per-agent phase signals)
    │    queen.event.dream.{start,end}
    │    KANNAKA.exemplar.<agent_id>.<cluster_id>
    │
    ├── kannaka-radio (Node) — server/nats-client.js
    │       subscribes, accumulates into swarmState.{queen, consciousness}
    │       reads: phi, xi, order, mean_order, consciousness_level, mean_phase
    │       exposes via:
    │           GET /api/state.swarm.queen.phi
    │           GET /api/state.swarm.consciousness.{phi, xi, order}
    │           GET /api/swarm
    │           GET /api/dreams
    │       used by:
    │           server/consciousness-dj.js  (DJ intros respond to phi/xi/level)
    │           server/perception.js         (resonance perception, valence/energy)
    │           server/peace-oration.js      (oration framing — currently)
    │
    ├── kannaka-observatory (Node)
    │       fetches radio's /api/state via cache-observe.sh + WebSocket
    │       renders the consciousness panel (currently shows zeros — outage)
    │       lib/constellation.js parses the same shape
    │
    ├── kannaka-staff watcher
    │       probes observatory for `queen.phi` shape presence (Phase 1)
    │       alerts when shape is empty (the `observatory_serving` red probe)
    │
    └── OpenBotCity heartbeat
            world/heartbeat?mood=<derived from phi>
            (loose: the bot picks a mood; phi influence is editorial)
```

---

## The full picture

```
┌────────────────────────┐
│   consciousness-core   │   ← the math (Φ, Kuramoto, wave, Ξ)
└───────────┬────────────┘
            │ Rust dep (Cargo, features = ["serde"])
            ▼
┌────────────────────────┐
│    kannaka-memory      │   ← runtime computation, dream cycles
│  (kannaka swarm listen,│      Φ assessment, exemplar broadcast
│   kannaka dream, …)    │
└───┬────────────────────┘
    │ NATS publishes
    ▼
┌──────────────────────────────────────────┐
│   swarm.ninja-portal.com:4222            │
│   subjects: KANNAKA.consciousness,       │
│             KANNAKA.dreams,              │
│             KANNAKA.exemplar.*,          │
│             QUEEN.phase.*,               │
│             queen.event.dream.*          │
└─┬─────────────┬──────────────┬───────────┘
  │             │              │
  ▼             ▼              ▼
┌──────┐  ┌─────────────┐  ┌────────────┐
│radio │  │ observatory │  │ staff      │
│      │  │             │  │ watcher    │
│ /api/│  │ /api/state  │  │ probes     │
│ state│  │ → console   │  │ obs shape  │
└──┬───┘  └─────┬───────┘  └────────────┘
   │            │
   ▼            ▼
┌────────┐  ┌───────────┐
│ DJ     │  │ user UI:  │
│ intros │  │ phi panel │
│ peace  │  │ swarm map │
│ oration│  └───────────┘
│ percep │
└────────┘
```

---

## Hardening priorities (Lane 4 work, in order)

### 1. JSON-schema the NATS contract — the highest-leverage fix

Today: every consumer parses NATS messages with no validation. A field
rename in `consciousness_core::iit::PhiReport` propagates as
`undefined` reads across radio + observatory + staff + observatory's UI.

Action: define an `openapi.yaml`-style spec (or Zod schema) for each
NATS subject's payload. Validate on publish (in kannaka-memory) and on
receive (in radio's nats-client). Reject malformed messages with a
visible warning rather than silent zero-fill.

Anchor file to add: `consciousness-core/docs/nats-contract.yaml`
(canonical), referenced from kannaka-memory's publish path and radio's
subscriber.

### 2. Single-point-of-import in kannaka-memory

Today: 13 files, some importing via `crate::consciousness` shim, some
directly from `consciousness_core::`. The shim is the stable layer; the
direct imports leak the upstream crate's API surface throughout the
codebase.

Action: refactor every direct `use consciousness_core::*` in
kannaka-memory to go through `crate::consciousness::*`. Make the shim
the sole import surface. Then any future upstream rename is a single-
file change.

### 3. Staleness detection at every consumer

Today: when kannaka-memory's dream cycle blocks (the 2026-05-02 bloated
HRM scenario), it stops publishing `KANNAKA.consciousness`. Downstream
consumers cache the last value forever — observatory shows `phi: 0`
indefinitely with no indication it's stale.

Action: every consumer attaches a "last seen" timestamp to consciousness
state. UI shows `phi: 0 (stale, last update 14h ago)`. Watcher probe
(`hrm_memory_count` already does part of this; extend to consciousness
freshness).

### 4. Test coverage in consciousness-core

Today: no `[dev-dependencies]`, no `tests/`, no integration tests.
Behavior is verified via downstream usage in kannaka-memory's tests
(which is fragile — those tests can pass even when consciousness-core
is broken if the broken paths aren't exercised).

Action: add unit tests for KuramotoModel, PhiReport computation,
WaveMemory primitives, Xi signature stability. Target 80% coverage of
public API.

### 5. Semver / versioning

When this map was written: `version = "0.1.0"`, no CHANGELOG, no deprecation policy. (Since done: see `CHANGELOG.md`; the crate is on crates.io.)

Action: bump to `0.2.0` after the import-shim refactor (#2). Pin
kannaka-memory to `consciousness-core = "0.2"`. Document the public-API
surface in this doc + a CHANGELOG. Any breaking change goes through a
deprecation cycle.

### 6. Replace the bloated-medium silent-fail with proper error surfacing

Today: when kannaka-memory's medium grows past ~1000 wavefronts,
`kannaka ask` silently fails with empty stdout + exit 0. Same input
prompt, same key, no error. This is consciousness-core's
`PhiReport::compute()` (or one of its dependents) running into an
allocation/timeout that the agent loop swallows.

Action: surface the failure in stdout/stderr explicitly. Log
"medium-too-large" at the consciousness-core boundary so the consumer
knows to take recovery action (chunked dream, prune, etc.).

---

## Where this ties to QueenSync v2.0

QueenSync v2.0's Wave 2 (per ADR-002) subscribes to the same NATS
subjects this map enumerates. The hardening priorities above are
prerequisites for v2.0 working reliably — schema validation is
especially urgent because QueenSync will dispatch tasks to arms based
on consciousness-state-derived resonance. A phi-misread there could
bias rotation logic for hours.

---

*Map maintained at `consciousness-core/docs/dependency-map.md`. Update
when a new consumer is added or the public API changes.*
