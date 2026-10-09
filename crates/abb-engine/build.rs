fn main() {
    let target = std::env::var("TARGET").expect("Cargo sets TARGET for build scripts");
    println!("cargo:rustc-env=ABB_TARGET_TRIPLE={target}");
}
