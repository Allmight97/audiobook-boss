// Quick test to verify 32000 sample rate is now accepted
use std::process::Command;

fn main() {
    println!("Testing sample rate validation fix...");
    
    let output = Command::new("cargo")
        .args(&["test", "test_validate_sample_rate_config", "--", "--nocapture"])
        .current_dir("/Users/jstar/Projects/audiobook-boss/src-tauri")
        .output()
        .expect("Failed to run cargo test");
    
    println!("Exit status: {}", output.status);
    println!("STDOUT:\n{}", String::from_utf8_lossy(&output.stdout));
    println!("STDERR:\n{}", String::from_utf8_lossy(&output.stderr));
}