# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.0.1] — 2026-04-06

### Added

- Full Rust port of the [NeurofieldCommunityAPI](https://github.com/eugenehp/NeurofieldCommunityAPI) (C#) with 100% protocol parity
- Cross-platform PCANBasic FFI layer via `libloading` (Windows, Linux, macOS)
- All PCAN-Basic v4 constants verified against `peak-can-sys` bindings
- `Q21Api` — top-level API: scaled µV samples, impedance in Ω
- `EegApi` — mid-level: raw 24-bit ADC, start/stop streaming, mode switching
- `CanBusBase` — low-level: CAN transport, device discovery, bus-off recovery
- Support for all Q21 hardware revisions (Rev-K, Rev-A, 21, 20-RevB, 20-RevA)
- Impedance measurement (Rev-K only) with cancellation support
- `prelude` module for one-line glob imports
- Headless CLI binary (`neurofield`) — streams 4s of EEG to stdout
- Real-time TUI binary (`tui`) with ratatui — 20-channel braille waveform viewer
  - Smooth mode (9-sample moving average overlay)
  - Auto-scale, manual scale (±10 to ±2000 µV)
  - Clip detection (red borders)
  - Live min/max/RMS stats per channel
  - Pause/resume/clear
  - Built-in EEG simulator (`--simulate`) — alpha + beta + theta + noise
- 14 protocol-level unit tests (header decoding, ADC sign extension, impedance extraction)
- Examples: `read_eeg`, `read_impedance`
- Comprehensive documentation (rustdoc + README)
