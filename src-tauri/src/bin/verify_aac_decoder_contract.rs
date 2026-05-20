use audiobook_boss_lib::audio::{
    detect_aac_decoder_availability, preferred_aac_decoder_order_labels,
};

fn main() {
    let availability = detect_aac_decoder_availability();
    let preferred_order = preferred_aac_decoder_order_labels(availability).join(",");

    #[cfg(target_os = "macos")]
    let contract_ok = availability.has_compatible_named_decoder();

    #[cfg(not(target_os = "macos"))]
    let contract_ok = true;

    println!(
        "default_aac={} aac_at={} libfdk_aac={} preferred_order={} contract={}",
        availability.default_aac,
        availability.aac_at,
        availability.libfdk_aac,
        preferred_order,
        if contract_ok {
            "ok"
        } else {
            "missing_named_decoder"
        }
    );

    if !contract_ok {
        eprintln!(
            "macOS AAC decoder contract failed: expected at least one compatible named AAC decoder (aac_at or libfdk_aac)"
        );
        std::process::exit(1);
    }
}
