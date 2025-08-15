fn main() {
    // Log build configuration for safe-ffmpeg builds
    if cfg!(feature = "safe-ffmpeg") {
        println!("cargo:warning=Building with safe-ffmpeg feature: using ffmpeg-next library instead of external binary");
        println!("cargo:warning=External FFmpeg binary bundled in tauri.conf.json but will not be used");
        println!("cargo:warning=Consider packaging optimization: exclude externalBin for production safe-ffmpeg builds");
    }
    
    tauri_build::build()
}
