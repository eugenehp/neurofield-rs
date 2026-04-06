//! Example: read impedance values from a Q21 Rev-K.
//!
//! ```bash
//! cargo run --example read_impedance --release --no-default-features
//! ```

use neurofield::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let mut api = Q21Api::new(UsbBus::USB1)?;

    api.switch_to_impedance_mode()?;

    let n_data = 20;
    for i in 0..n_data {
        let sample = api.receive_single_impedance_sample(None)?;
        println!("Sample {}: {:?}", i, sample.data);
    }

    api.abort_receiving_eeg()?;

    println!("Done — collected {} impedance samples", n_data);
    Ok(())
}
