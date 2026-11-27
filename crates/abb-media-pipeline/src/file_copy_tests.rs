use super::*;
use tempfile::TempDir;

#[test]
fn copy_data_only_preserves_bytes_across_empty_small_and_multi_chunk_files() {
    let dir = TempDir::new().expect("temp dir");
    for (name, size) in [
        ("empty.bin", 0usize),
        ("small.bin", 4096),
        ("large.bin", (5 * 1024 * 1024) + 123),
    ] {
        let src = dir.path().join(name);
        let dst = dir.path().join(format!("copy-{name}"));
        let payload: Vec<u8> = (0..size).map(|idx| (idx % 251) as u8).collect();
        std::fs::write(&src, &payload).expect("write source");

        let written = copy_data_only(&src, &dst).expect("copy data only");

        assert_eq!(written, size as u64);
        assert_eq!(std::fs::read(&dst).expect("read copy"), payload);
    }
}

#[test]
fn clone_or_copy_data_produces_identical_bytes_and_filesystem_label() {
    let dir = TempDir::new().expect("temp dir");
    let src = dir.path().join("source.bin");
    let dst = dir.path().join("cloned.bin");
    let payload = vec![0x5Au8; 64 * 1024];
    std::fs::write(&src, &payload).expect("write source");

    let file = File::open(&src).expect("open source");
    let label = filesystem_label_for_file(&file);
    assert!(!label.is_empty());
    drop(file);

    let stats = clone_or_copy_data(&src, &dst).expect("clone or copy");
    assert_eq!(stats.fs, label);
    assert!(matches!(stats.strategy, "cow_clone" | "data_copy"));
    assert_eq!(std::fs::read(&dst).expect("read destination"), payload);
}

#[test]
fn copy_stats_log_fields_formats_fields_cleanly() {
    let cow = CopyStats {
        strategy: "cow_clone",
        fs: "apfs",
        bytes_written: 0,
    };
    assert_eq!(
        cow.log_fields(5),
        "strategy=cow_clone fs=apfs bytes_written=0 mb_s=0.0"
    );

    let data = CopyStats {
        strategy: "data_copy",
        fs: "smbfs",
        bytes_written: 10 * 1024 * 1024,
    };
    assert_eq!(
        data.log_fields(1000),
        "strategy=data_copy fs=smbfs bytes_written=10485760 mb_s=10.0"
    );
}
