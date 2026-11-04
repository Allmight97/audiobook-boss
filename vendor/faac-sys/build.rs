use std::{env, fs, path::PathBuf};

fn compile_slice(
    name: &str,
    sources: &[&str],
    prefix: &str,
    max_channels: u32,
    out: &std::path::Path,
) {
    let mut build = cc::Build::new();
    build
        .include("upstream")
        .include("upstream/include")
        .include("upstream/common")
        .include(out)
        .define("COMMON_PREFIX", prefix)
        .define("MAX_CHANNELS", max_channels.to_string().as_str())
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
    build.compile(name);
}

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    let version = env::var("CARGO_PKG_VERSION").expect("FAAC package version");
    fs::write(
        out.join("config.h"),
        format!(
            "#define PACKAGE \"faac\"\n\
             #define PACKAGE_VERSION \"{version}\"\n\
             #define FAAC_SBR_DECIMATION 1\n\
             #define FAAC_ENCODER_SBR 1\n\
             #define FAAC_ENCODER_PS 0\n\
             #define FAAC_ENCODER_DRC 1\n"
        ),
    )
    .expect("write FAAC build configuration");

    // Portable scalar lists from upstream meson.build for ABB's options:
    // encoder-sbr on, encoder-ps off, encoder-drc on, decoder SBR, PS, DRC and
    // DAB on. The encoder writes mono or stereo; the decoder reads up to eight
    // channels so surround sources reach ABB's downmix. `common/` is compiled
    // once per library under that library's symbol prefix, as upstream does.
    let faac_sources = [
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
        "libfaac/stereo.c",
        "libfaac/tns.c",
        "libfaac/util.c",
        "libfaac/sbr.c",
        "libfaac/sbr_analysis.c",
        "libfaac/sbr_bitstream.c",
        "libfaac/sbr_huff_tables.c",
        "libfaac/resample.c",
        "libfaac/drc.c",
        "libfaac/drc_analyzer.c",
        "common/fft.c",
        "common/sbr_tables.c",
        "common/sfb_tables.c",
    ];
    let faad_sources = [
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
        "libfaad/coupling.c",
        "common/fft.c",
        "common/sbr_tables.c",
        "common/sfb_tables.c",
    ];
    compile_slice("faac", &faac_sources, "faac_", 2, &out);
    compile_slice("faad3", &faad_sources, "faad_", 8, &out);
    if env::var("CARGO_CFG_TARGET_FAMILY").as_deref() == Ok("unix") {
        println!("cargo:rustc-link-lib=m");
    }
    println!("cargo:rerun-if-changed=upstream");
    bindgen::Builder::default()
        .header("upstream/include/faac.h")
        .header("upstream/include/faad3.h")
        .allowlist_function("faa[cd]_.*")
        .allowlist_type("faa[cd]_.*")
        .allowlist_var("FAA[CD]_.*")
        .prepend_enum_name(false)
        .derive_default(true)
        .generate_comments(false)
        .generate()
        .expect("generate FAAC and FAAD public API bindings")
        .write_to_file(out.join("bindings.rs"))
        .expect("write FAAC and FAAD bindings");
}
