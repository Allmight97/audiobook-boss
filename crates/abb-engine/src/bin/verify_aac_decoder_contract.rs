use abb_engine::audio::{detect_aac_decoder_availability, preferred_aac_decoder_order_labels};
use std::process::ExitCode;

fn main() -> ExitCode {
    let availability = detect_aac_decoder_availability();
    let preferred_order = preferred_aac_decoder_order_labels(availability).join(",");
    let validation = availability.validate_runtime_contract();

    println!(
        "default_aac={} aac_at={} preferred_order={} contract={}",
        availability.default_aac,
        availability.aac_at,
        preferred_order,
        if validation.is_ok() {
            "ok"
        } else {
            "missing_required_decoder"
        }
    );

    match validation {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
