# Change: Remove Tracing System

## Why

The tracing system (including trace collection and optimization) has proven to be too slow, complex, and not as useful
as anticipated. Removing it will simplify the codebase, reduce maintenance burden, and potentially improve performance
by removing overhead.

## What Changes

- Remove the `trace-collection` capability entirely.
- Remove the `trace-optimization` capability entirely.
- Remove all tracing-related code from `ceres-core`, `ceres-std`, and `ceres-test-runner`.
- Remove tracing documentation.

## Impact

- Affected specs: `trace-collection`, `trace-optimization`
- Affected code: `ceres-core/src/trace.rs`, `ceres-std/src/tracing.rs`, `ceres-test-runner`, `docs/TRACING_GUIDE.md`
- **BREAKING**: The tracing API and CLI flags will no longer be available.
