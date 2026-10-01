use abb_engine::audio::{detect_aac_decoder_availability, preferred_aac_decoder_order_labels};

fn main() {
    let availability = detect_aac_decoder_availability();
    let preferred_order = preferred_aac_decoder_order_labels(availability).join(",");

    #[cfg(target_os = "macos")]
    let contract_ok = availability.has_named_decoder();

    #[cfg(not(target_os = "macos"))]
    let contract_ok = true;

    println!(
        "default_aac={} aac_at={} preferred_order={} contract={}",
        availability.default_aac,
        availability.aac_at,
        preferred_order,
        if contract_ok {
            "ok"
        } else {
            "missing_named_decoder"
        }
    );

    if !contract_ok {
        eprintln!("macOS AAC decoder contract failed: expected the named AAC decoder aac_at");
        std::process::exit(1);
    }
}
