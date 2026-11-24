<!--
Sync Impact Report
- Version change: 0.0.0 (uninitialized) → 1.0.0
- Modified principles:
  - [PRINCIPLE_1_NAME] → I. Workspace Isolation & Parity (`fast-photon-core`)
  - [PRINCIPLE_2_NAME] → II. Memory Safety & Deterministic Resource Lifecycle
  - [PRINCIPLE_3_NAME] → III. Wire, Schema & Protocol Compatibility
  - [PRINCIPLE_4_NAME] → IV. High-Throughput Concurrency & Zero-Copy Hot Paths
  - [PRINCIPLE_5_NAME] → V. Test-Driven Parity & Benchmarking Discipline
- Added sections:
  - Core Principles (I–V)
  - Technical & Migration Constraints
  - Development Workflow & Quality Gates
  - Governance
- Removed sections: None
- Templates requiring updates:
  - ✅ .specify/templates/plan-template.md (synced & updated Constitution Check gates)
  - ✅ .specify/templates/spec-template.md (synced & aligned with parity/migration requirements)
  - ✅ .specify/templates/tasks-template.md (synced & aligned with Rust `fast-photon-core/` paths and verification tasks)
  - ✅ .agents/skills/speckit.plan/templates/plan-template.md (updated Constitution Check gates)
- Follow-up TODOs: None
-->

# PhotonVision (`fast-photon-core` Rust Migration) Constitution

## Core Principles

### I. Workspace Isolation & Parity (`fast-photon-core`)

All Rust migration code for `photon-core` MUST reside in the top-level `fast-photon-core/` directory as a self-contained Cargo workspace or crate hierarchy. The existing Java `photon-core/` implementation MUST remain intact and buildable as the behavioral reference oracle until full parity is verified. Every module in `fast-photon-core` MUST map deterministically to its corresponding `photon-core` domain (`configuration`, `dataflow`, `hardware`, `networking`, `logging`, `vision::camera`, `vision::frame`, `vision::pipe`, `vision::pipeline`, `vision::processes`, `vision::target`, and `vision::calibration`).

**Rationale**: Isolating the Rust implementation in `fast-photon-core/` prevents regressions in the production Gradle build while enabling side-by-side differential testing against the Java baseline.

### II. Memory Safety & Deterministic Resource Lifecycle

All image buffers, OpenCV matrices, and hardware handles MUST leverage Rust's ownership and RAII (`Drop`) semantics instead of manual `Releasable` tracking. `unsafe` blocks are forbidden in application and pipeline logic; `unsafe` code MUST be restricted to thin, audited FFI boundary wrappers (e.g., `cscore`, `libcamera`, `mrcal`, `rknn`, `tflite`, or OpenCV C++ bindings) with documented safety invariants. Frame buffers on the hot path MUST use bounded pools or pre-allocated arenas to prevent per-frame heap allocation churn.

**Rationale**: A primary defect class in the Java `photon-core` is native memory leaks (`CVMat` use-after-free or missed `release()` calls). Rust's ownership model eliminates manual release bugs at compile time.

### III. Wire, Schema & Protocol Compatibility

`fast-photon-core` MUST maintain 100% external protocol compatibility with existing PhotonVision consumers (`photon-client`, `photon-server`, `photon-lib`, and FRC robot controllers):
- **SQLite & JSON Config (`photon.sqlite`)**: MUST read and write the exact schema, migration versions (`user_version`), and JSON structure produced by `ConfigProvider` and `avaje-jsonb`.
- **NetworkTables (NT4)**: MUST publish and subscribe to the exact topic names, types, `PhotonPipelineResult` packet/struct/protobuf payloads, and time-sync subtables expected by `photon-lib`.
- **WebSocket & UI DTOs**: MUST serialize and deserialize `UIPhotonConfiguration`, `UICameraConfiguration`, and `DataChangeEvent` payloads with identical field names and semantics.

**Rationale**: Drop-in replacement of `photon-core` requires that neither robot code (`photon-lib`) nor the web UI (`photon-client`) can distinguish whether the backend runs on Java or Rust.

### IV. High-Throughput Concurrency & Zero-Copy Hot Paths

Each active camera (`VisionModule`) MUST execute frame acquisition and `CVPipeline` processing on dedicated worker threads or bounded async tasks decoupled from MJPEG streaming, WebSocket broadcasting, and SQLite disk I/O. Inter-component messaging (`DataChangeService`) MUST use lock-free or bounded multi-producer/multi-consumer channels (`crossbeam-channel` or `tokio::sync::mpsc`) rather than coarse global mutexes. Frame handoff from `FrameProvider` through `CVPipe` stages MUST minimize cloning and avoid redundant color-space conversions.

**Rationale**: Real-time FRC vision processing on embedded ARM64 coprocessors (Raspberry Pi, Orange Pi 5 RK3588, Rubik Pi 3 QCS6490) demands low jitter and deterministic frame latency.

### V. Test-Driven Parity & Benchmarking Discipline

Every migrated pipe (`CVPipe`), pipeline (`CVPipeline`), target calculation (`TrackedTarget`, `TargetCalculations`), and SQLite migration MUST include automated unit and integration tests executable via `cargo test`. Numerical and geometric outputs (2D contour metrics, yaw/pitch/area/skew, 3D SolvePnP / AprilTag pose transforms, and calibration coefficients) MUST match the Java `photon-core` test suite within defined floating-point tolerances using shared `test-resources/`. Hot-path vision pipes SHOULD include benchmarks (`cargo bench` or `rust-anything` profiling) demonstrating equal or superior throughput compared to the Java baseline.

**Rationale**: Vision math and pose estimation errors directly impact robot autonomous accuracy on the field; parity tests against `test-resources/` guarantee functional equivalence.

## Technical & Migration Constraints

- **Target Directory**: `/fast-photon-core` at the repository root.
- **Toolchain & Edition**: Stable Rust (Edition 2021 or 2024), formatted with `rustfmt` and linted with `clippy` (`cargo clippy --all-targets -- -D warnings`).
- **Supported Platforms**: `linux-arm64` (Raspberry Pi, RK3588, QCS6490), `linux-x86_64`, and `windows-x86_64`. Hardware-specific backends (`rknn`, `libcamera`, `diozero`/GPIO) MUST be gated behind Cargo features or platform traits with graceful fallback (`NullModel` / simulated hardware) on desktop development machines.
- **Error Handling**: Panics (`unwrap()` / `expect()`) are forbidden in runtime pipeline, camera, network, and configuration code paths; all fallible operations MUST return typed `Result<T, E>` using `thiserror` or `anyhow`.

## Development Workflow & Quality Gates

1. **Specification & Planning**: Every migration slice MUST trace back to `.specify/features/photon-core/` (or feature-specific `specs/` directories) with explicit mapping to the legacy Java classes being replaced.
2. **Static Analysis Gate**: All code changes in `fast-photon-core/` MUST pass `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` with zero warnings.
3. **Parity Verification Gate**: All unit and integration tests (`cargo test`) MUST pass before a migration task is marked complete.
4. **No Autonomous PRs or Issues**: In compliance with repository rules (`AGENTS.md`), agents MUST NEVER create GitHub issues or pull requests.

## Governance

This Constitution is the authoritative governance document for migrating `photon-core` to Rust in `fast-photon-core/`. It supersedes ad-hoc implementation choices across all Speckit phases (`specify`, `clarify`, `plan`, `tasks`, `analyze`, `implement`, `validate`).

- **Amendment Procedure**: Any modification to core principles, target paths, or compatibility requirements MUST be documented via `/01-speckit.constitution` with an updated Sync Impact Report and propagated to `.specify/templates/`.
- **Versioning Policy**: Semantic Versioning (`MAJOR.MINOR.PATCH`):
  - `MAJOR`: Removal or backward-incompatible redefinition of a Core Principle or architectural boundary.
  - `MINOR`: Addition of a new principle, target platform, or mandatory quality gate.
  - `PATCH`: Wording clarifications, typo fixes, or non-semantic refinements.
- **Compliance Review**: Every `/04-speckit.plan` (Constitution Check section) and `/06-speckit.analyze` run MUST verify compliance against Principles I–V and halt on unjustified violations.

**Version**: 1.0.0 | **Ratified**: 2026-11-23 | **Last Amended**: 2026-11-23
