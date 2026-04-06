//! Example: read 4 seconds of EEG data from a Q21.
//!
//! ```bash
//! cargo run --example read_eeg --release --no-default-features
//! ```

use neurofield::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let mut api = Q21Api::new(UsbBus::USB1)?;

    println!("Connected to device...");
    println!("Device Type: {:?}", api.eeg_device_type());
    println!("Device Serial Number: {}", api.eeg_device_serial());
    println!(
        "{}",
        if api.impedance_enabled() {
            "Device has impedance measurement capability"
        } else {
            "This model can not measure channel impedance."
        }
    );

    api.start_receiving_eeg()?;

    let sampling_rate = SAMPLING_RATE as usize;
    let n_samples = sampling_rate * 4;
    let mut eeg_data = vec![[0.0f64; NUM_CHANNELS]; n_samples];

    for i_sample in 0..n_samples {
        let sample = api.get_single_sample()?;
        eeg_data[i_sample] = sample.data;
        if i_sample % sampling_rate == 0 {
            println!("1 sec Data...");
        }
    }

    api.abort_receiving_eeg()?;
    api.release();

    println!("Done — collected {} samples × {} channels", n_samples, NUM_CHANNELS);
    Ok(())
}
