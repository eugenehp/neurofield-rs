//! Headless CLI for the Neurofield Q21 EEG amplifier.
//!
//! Connects to the first Q21 found on USB1, streams 4 seconds of EEG data
//! to stdout, then disconnects.
//!
//! ```bash
//! cargo run --release --no-default-features
//! RUST_LOG=debug cargo run --release --no-default-features
//! ```

use neurofield::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Connecting to Q21 on USB1…");
    let mut api = Q21Api::new(UsbBus::USB1)?;

    log::info!("Connected to device.");
    println!("Device Type:   {:?}", api.eeg_device_type());
    println!("Serial Number: {}", api.eeg_device_serial());
    println!(
        "Impedance:     {}",
        if api.impedance_enabled() {
            "supported (Rev-K)"
        } else {
            "not supported"
        }
    );

    // Start streaming
    api.start_receiving_eeg()?;
    println!("\nStreaming 4 seconds of EEG…\n");

    let sampling_rate = SAMPLING_RATE as usize;
    let n_samples = sampling_rate * 4;

    for i_sample in 0..n_samples {
        let sample = api.get_single_sample()?;

        if i_sample % sampling_rate == 0 {
            let sec = i_sample / sampling_rate;
            println!("── second {} ──", sec);
        }

        // Print one line per sample with first 4 channels
        if i_sample % 64 == 0 {
            print!("  t={:6.3}s ", i_sample as f64 / SAMPLING_RATE);
            for (ch, &name) in EEG_CHANNEL_NAMES.iter().enumerate().take(4) {
                print!(" {}:{:+8.2}µV", name, sample.data[ch]);
            }
            println!("  …");
        }
    }

    println!("\nDone — collected {} samples × {} channels.", n_samples, NUM_CHANNELS);

    api.abort_receiving_eeg()?;
    api.release();
    Ok(())
}
