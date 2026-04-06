# neurofield

A Rust library and terminal UI for streaming real-time EEG data from
**Neurofield Q21** 20-channel EEG amplifiers over PCAN-USB.

Rust port of the [NeurofieldCommunityAPI](https://github.com/eugenehp/NeurofieldCommunityAPI)
(C#) with 100% protocol parity — identical CAN-bus message encoding, byte-level
ADC extraction, scale factors, impedance formula, bus-off recovery, and all
PCAN-Basic v4 constants.

The Q21 is a USA FDA approved, high-resolution, DC-coupled 20-channel
simultaneous-sampling amplifier with very low input noise and high Common Mode
Rejection Ratio.

## Installation

```shell
cargo add neurofield
```

---

## Supported hardware

| Model | Device Type | Scale Factor | Impedance |
|---|---|---|---|
| Q21 Rev-K (`0xA5`) | `Eeg21RevK` | −0.04470 µV/LSB | ✓ |
| Q21 Rev-A (`0xA4`) | `Eeg21RevA` | −0.04023 µV/LSB | ✗ |
| Q21 (`0xA3`) | `Eeg21` | −0.02087 µV/LSB | ✗ |
| 20ch Rev-B (`0xA2`) | `Eeg20RevB` | −0.02087 µV/LSB | ✗ |
| 20ch Rev-A (`0xA1`) | `Eeg20RevA` | −0.02087 µV/LSB | ✗ |

All models stream 20 channels at 256 Hz over CAN bus (500 kbit/s) via a
PEAK PCAN-USB adapter.

---

## Cross-platform

Works on **Windows**, **Linux**, and **macOS**.  The PCANBasic shared library
(`PCANBasic.dll` / `libPCANBasic.so` / `libPCANBasic.dylib`) is loaded at
runtime via `libloading` — no build-time C dependencies, no bindgen, no
system headers.

---

## Prerequisites

| Requirement | Notes |
|---|---|
| Rust ≥ 1.75 | `rustup update stable` |
| PCAN drivers | Install from [PEAK-System](https://www.peak-system.com/PCAN-Basic.239.0.html?&L=1) |
| PCAN-USB adapter | Connected to the Q21 via CAN bus |

---

## Features

### Library

Use `neurofield` as a library in your own project:

```toml
[dependencies]
# Full build (includes the ratatui TUI feature):
neurofield = "0.0.1"

# Library only — skips ratatui / crossterm compilation:
neurofield = { version = "0.0.1", default-features = false }
```

```rust
use neurofield::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to the first Q21 on USB1 (~1 s discovery)
    let mut api = Q21Api::new(UsbBus::USB1)?;

    println!("Device: {:?}, serial: {}", api.eeg_device_type(), api.eeg_device_serial());
    println!("Impedance: {}", api.impedance_enabled());

    // Start streaming (up to 8 hours)
    api.start_receiving_eeg()?;

    // Collect 4 seconds of data
    let n_samples = SAMPLING_RATE as usize * 4;
    for i in 0..n_samples {
        let sample = api.get_single_sample()?;
        // sample.data: [f64; 20] in µV
        // sample.timestamp_us: reception time in µs
        if i % SAMPLING_RATE as usize == 0 {
            println!("1 sec…");
        }
    }

    api.abort_receiving_eeg()?;
    Ok(())
}
```

### Channel order (20 channels)

```text
F7=0, T3=1, T4=2, T5=3, T6=4, Cz=5, Fz=6, Pz=7, F3=8, C4=9,
C3=10, P4=11, P3=12, O2=13, O1=14, F8=15, F4=16, Fp1=17, Fp2=18, HR=19
```

---

## Build

```bash
cargo build --release              # lib + both binaries (TUI on by default)
cargo build --release --no-default-features  # lib + headless CLI only
```

---

## TUI — real-time waveform viewer

```bash
cargo run --bin tui --release                # connect to Q21 on USB1
cargo run --bin tui --release -- --simulate  # built-in EEG simulator (no hardware)
```

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│  Neurofield Q21 Monitor  │  ● Connected  │  256.0 smp/s  │  ±500 µV        │
├──────────────────────────────────────────────────────────────────────────────┤
│ F7   min: -38.2  max: +41.5  rms: 17.8 µV                    [SMOOTH]       │
│ ⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿  braille waveform, rolling 2-second window                  │
├──────────────────────────────────────────────────────────────────────────────┤
│ T3   ...                                                                     │
├──────────────────────────────────────────────────────────────────────────────┤
│ T4   ...                                                                     │
├──────────────────────────────────────────────────────────────────────────────┤
│ T5   ...                                                                     │
├──────────────────────────────────────────────────────────────────────────────┤
│ remaining 16 channels in compact grid                                        │
├──────────────────────────────────────────────────────────────────────────────┤
│ [+/-]Scale  [a]Auto  [v]Smooth  [p]Pause  [r]Resume  [c]Clear  [q]Quit      │
│ Neurofield Q21 · 20 channels · 256 Hz · PCAN-USB                            │
└──────────────────────────────────────────────────────────────────────────────┘
```

All 20 channels are displayed: 4 large panels (F7, T3, T4, T5) + the remaining
16 in a compact 4×4 grid.  Borders turn **red** when clipping.  **Smooth mode**
(toggle `v`) overlays a 9-sample moving average on a dimmed raw trace.

### TUI key reference

| Key | Action |
|---|---|
| `+` / `=` | Zoom out (increase µV scale) |
| `-` | Zoom in (decrease µV scale) |
| `a` | Auto-scale Y axis to current peak |
| `v` | Toggle smooth overlay |
| `p` | Pause streaming |
| `r` | Resume streaming |
| `c` | Clear all waveform buffers |
| `q` / Esc / Ctrl-C | Quit |

### Simulator

The `--simulate` flag generates synthetic EEG (no hardware needed):

| Component | Frequency | Amplitude |
|---|---|---|
| Alpha | 10 Hz | ±20 µV |
| Beta | 22 Hz | ±6 µV |
| Theta | 6 Hz | ±10 µV |
| Noise | broadband | ±4 µV |

---

## Console streamer (`neurofield` binary)

```bash
cargo run --release --no-default-features
RUST_LOG=debug cargo run --release --no-default-features
```

Connects to USB1, streams 4 seconds to stdout, and exits.

---

## API overview

| Struct | C# equivalent | Description |
|---|---|---|
| `Q21Api` | `NeurofieldCommunityQ21API` | Top-level: scaled µV, impedance Ω |
| `EegApi` | `NeurofieldCommunityEEGAPI` | Mid-level: raw ADC, start/stop, mode switching |
| `CanBusBase` | `NeurofieldCommunityCANBUSApiBase` | Low-level: CAN transport, device discovery |

### Key methods on `Q21Api`

| Method | Description |
|---|---|
| `new(bus)` | Connect and discover devices (~1 s) |
| `start_receiving_eeg()` | Start streaming (up to 8 hours) |
| `get_single_sample()` | Receive one 20-ch sample in µV (~4 ms) |
| `get_single_raw_sample()` | Receive one raw 24-bit sample |
| `abort_receiving_eeg()` | Stop streaming |
| `switch_to_impedance_mode()` | Switch to impedance (Rev-K only) |
| `switch_to_eeg_mode()` | Switch back to EEG (Rev-K only) |
| `receive_single_impedance_sample()` | Read impedance in Ω (Rev-K only) |
| `blink()` | Blink the front LED 3 times |
| `get_online_pcan_interfaces()` | List available PCAN USB interfaces |
| `release()` | Release the CAN-USB interface |

---

## Project layout

```
neurofield-rs/
├── Cargo.toml
├── README.md
└── src/
    ├── lib.rs            # Crate root: modules + prelude
    ├── main.rs           # Headless CLI binary
    ├── bin/
    │   └── tui.rs        # Full-screen TUI binary (ratatui)
    ├── pcan.rs           # Cross-platform PCANBasic FFI (runtime-loaded)
    ├── canbus_base.rs    # CAN transport, discovery, send/receive
    ├── eeg_api.rs        # EEG protocol: streaming, raw ADC, mode switching
    ├── q21_api.rs        # Scaled µV, impedance, constants
    ├── device.rs         # DeviceType enum + Device struct
    ├── message_types.rs  # Q21MessageType enum + ExtendedHeader
    └── error.rs          # NeurofieldError
├── examples/
│   ├── read_eeg.rs       # 4-second EEG capture
│   └── read_impedance.rs # Impedance measurement (Rev-K)
└── tests/
    └── protocol_tests.rs # Header decoding, ADC extraction, sign extension
```

---

## Protocol notes

### CAN-bus extended header (29-bit ID)

```text
bit 24:       slave → host flag
bits 23..16:  module type (DeviceType byte)
bits 15..8:   serial number
bits 7..0:    message type (Q21MessageType byte)
```

### ADC data format

Each EEG sample arrives as a sequence of **10 CAN messages**, each carrying
2 channels × 3 bytes = 6 bytes of 24-bit signed big-endian ADC values.

```text
channel[i]     = (sbyte(byte[0]) << 16) | (byte[1] << 8) | byte[2]
channel[i + 1] = (sbyte(byte[3]) << 16) | (byte[4] << 8) | byte[5]
µV = raw × scale_factor
```

### Impedance format (Rev-K only)

20 CAN messages, each with 8 bytes: 4 bytes offset voltage + 4 bytes
impedance voltage as 32-bit big-endian signed integers.

```text
Z = (V_impedance - V_offset) / 6µA - 12kΩ
  clamped to minimum 1000 Ω
```

### Scale factors

| Revision | Formula | µV/LSB |
|---|---|---|
| Rev-K | 4.5V / 2²⁴ / 12 | −0.044703483581543 |
| Rev-A | 4.5V / 2²⁴ / 6.6667 / 2 | −0.040233115106831 |
| Others | 4.5V / 2²⁴ / 12.85 / 2 | −0.020873221905779 |

---

## Dependencies

| Crate | Purpose |
|---|---|
| [libloading](https://crates.io/crates/libloading) | Runtime DLL/so/dylib loading for PCANBasic |
| [thiserror](https://crates.io/crates/thiserror) | Error type derivation |
| [log](https://crates.io/crates/log) | Logging facade |
| [env_logger](https://crates.io/crates/env_logger) | Log output for binaries |
| [ratatui](https://ratatui.rs) | Terminal UI (optional, `tui` feature) |
| [crossterm](https://github.com/crossterm-rs/crossterm) | Terminal backend (optional, `tui` feature) |

---

## Running tests

```bash
cargo test
```

14 protocol-level unit tests cover header decoding, ADC sign extension,
impedance extraction, message type classification, device type roundtrips,
and timestamp conversion — all run without hardware.

---

## License

[MIT](./LICENSE)
