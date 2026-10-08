use std::{env, fs, path::PathBuf};

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    // Keep the linked library's diagnostic identity with the selected package release.
    let version = env::var("CARGO_PKG_VERSION").expect("FAAC package version");
    let big_endian = u8::from(env::var("CARGO_CFG_TARGET_ENDIAN").as_deref() == Ok("big"));
    fs::write(
        out.join("config.h"),
        format!("#define PACKAGE_VERSION \"{version}\"\n#define MAX_CHANNELS 2\n#define FAAC_SBR_DECIMATION 1\n#define WORDS_BIGENDIAN {big_endian}\n"),
    ).expect("write FAAC build configuration");

    // Portable scalar source lists from upstream libfaac/, libfaad/ and common/ meson.build.
    let sources = [
        "libfaac/bitstream.c",
        "libfaac/blockswitch.c",
        "libfaac/channels.c",
        "libfaac/cpu_compute.c",
        "libfaac/faac.c",
        "libfaac/filtbank.c",
        "libfaac/frame.c",
        "libfaac/huff2.c",
        "libfaac/huffdata.c",
        "libfaac/quantize.c",
        "libfaac/ratecontrol.c",
        "libfaac/sbr.c",
        "libfaac/sbr_bitstream.c",
        "libfaac/sbr_huff_tables.c",
        "libfaac/sbr_analysis.c",
        "libfaac/resample.c",
        "libfaac/stereo.c",
        "libfaac/tns.c",
        "libfaac/util.c",
        "libfaad/decoder.c",
        "libfaad/bits.c",
        "libfaad/asc.c",
        "libfaad/huffman.c",
        "libfaad/syntax.c",
        "libfaad/dequant.c",
        "libfaad/stereo.c",
        "libfaad/tns.c",
        "libfaad/imdct.c",
        "libfaad/sbr.c",
        "libfaad/ps.c",
        "libfaad/sbr_dec_tables.c",
        "common/fft.c",
        "common/sbr_tables.c",
        "common/sfb_tables.c",
    ];
    let mut build = cc::Build::new();
    build
        .include("upstream")
        .include("upstream/include")
        .include("upstream/common")
        .include(&out)
        .flag(
            if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
                "/FI"
            } else {
                "-include"
            },
        )
        .flag(out.join("config.h").to_str().expect("configuration path"))
        .std("c11")
        .opt_level(3);
    for source in sources {
        build.file(format!("upstream/{source}"));
    }
    build.compile("faac");
    if env::var("CARGO_CFG_TARGET_FAMILY").as_deref() == Ok("unix") {
        println!("cargo:rustc-link-lib=m");
    }
    println!("cargo:rerun-if-changed=upstream");
    bindgen::Builder::default()
        .header("upstream/include/faac.h")
        .header("upstream/include/faad.h")
        .allowlist_function("faa[cd]_.*")
        .allowlist_type("faa[cd]_.*")
        .allowlist_var("FAA[CD]_.*")
        .prepend_enum_name(false)
        .derive_default(true)
        .generate_comments(false)
        .generate()
        .expect("generate FAAC and FAAD public API bindings")
        .write_to_file(out.join("bindings.rs"))
        .expect("write FAAC bindings");
}
