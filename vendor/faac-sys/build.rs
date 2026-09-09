use std::{env, fs, path::PathBuf};

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    fs::write(
        out.join("config.h"),
        "#define PACKAGE_VERSION \"2.1.0\"\n#define MAX_CHANNELS 2\n#define FAAC_SBR_DECIMATION 1\n",
    ).expect("write FAAC build configuration");

    // Portable scalar source list from upstream libfaac/meson.build.
    let sources = [
        "bitstream.c",
        "blockswitch.c",
        "channels.c",
        "cpu_compute.c",
        "faac.c",
        "filtbank.c",
        "fft.c",
        "frame.c",
        "huff2.c",
        "huffdata.c",
        "quantize.c",
        "sbr.c",
        "sbr_bitstream.c",
        "sbr_tables.c",
        "sbr_analysis.c",
        "resample.c",
        "stereo.c",
        "tns.c",
        "util.c",
    ];
    let mut build = cc::Build::new();
    build
        .include("upstream/include")
        .include(&out)
        .define("HAVE_CONFIG_H", None)
        .std("c11")
        .opt_level(3);
    for source in sources {
        build.file(format!("upstream/libfaac/{source}"));
    }
    build.compile("faac");
    if env::var("CARGO_CFG_TARGET_FAMILY").as_deref() == Ok("unix") {
        println!("cargo:rustc-link-lib=m");
    }
    println!("cargo:rerun-if-changed=upstream");
    bindgen::Builder::default()
        .header("upstream/include/faac.h")
        .allowlist_function("faac_.*")
        .allowlist_type("faac_.*")
        .allowlist_var("FAAC_.*")
        .prepend_enum_name(false)
        .derive_default(true)
        .generate_comments(false)
        .generate()
        .expect("generate FAAC public API bindings")
        .write_to_file(out.join("bindings.rs"))
        .expect("write FAAC bindings");
}
