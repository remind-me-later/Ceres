# Ceres Emulator

Ceres is an experimental Game Boy and Game Boy Color emulator written in Rust.
The project keeps core emulation logic separate from frontend crates.

## Repository Map

- `ceres-core`: CPU, APU, PPU, memory, cartridge, and timing logic.
- `ceres-std`: Desktop support code such as audio, threading, and WebGPU.
- `ceres-winit`: Minimal cross-platform frontend.
- `ceres-egui`: egui-based GUI frontend.
- `ceres-gtk`: GTK4 frontend for Linux.
- `ceres-test-runner`: Integration tests using ROMs and screenshot comparison.
- `external/gb-bootroms`: Boot ROM sources and build scripts.
- `external/reference-implementations`: Reference emulators and related code,
  including SameBoy, Gambatte, Mooneye, and Metroboy.
- `external/test-roms`: Bundled compiled test ROM suites used for emulator
  validation.
- `external/test-sources`: Upstream test ROM source trees, including SameSuite,
  Mooneye, GBMicrotest, AGE, and gb-test-roms.

## References

- `external/reference-implementations/metroboy` (Gateboy/Metroboy) is the most
  hardware-accurate local reference, but it is usually overkill because it
  emulates signals and gates directly.
- Use `external/reference-implementations/SameBoy` as the main local correctness
  reference.
- Use `external/reference-implementations/gambatte-core` and
  `external/reference-implementations/mooneye-gb` as secondary references when
  useful.
- Use [Pan Docs](https://gbdev.io/pandocs/) for Game Boy hardware behavior.
- Treat the integration tests in `ceres-test-runner` as the project spec.
- Use `external/test-roms` and `external/test-sources` as the main test corpus
  and supporting behavioral reference.

## Development Rules

- Check `README.md` for build and run instructions.
- Check `CONTRIBUTING.md` for project-wide development guidelines.
- Format code with `cargo fmt`.
- Format TOML with `tombi format`.
- Format JSON, Markdown, and YAML with `prettier`.
- Use Conventional Commits.

## Accuracy Workflow

When fixing emulator accuracy issues or failing integration tests:

1. Isolate the hardware behavior that is wrong.
2. Compare against SameBoy or Gambatte at sub-M-cycle granularity when needed.
3. Add small, focused unit tests in the relevant `ceres-core` module.
4. Encode timing assumptions in tests so hardware quirks are documented.
5. Verify timing at individual T-cycles when exact ordering matters.
6. After the local fix passes, run `cargo test -p ceres-test-runner` to catch
   regressions.
