use super::*;

fn make_executable(path: &Path) {
    std::fs::write(path, b"helper").expect("write executable fixture");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)
            .expect("executable metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("chmod executable fixture");
    }
}

fn repo_sidecar_path(root: &Path) -> PathBuf {
    root.join("src-tauri")
        .join("binaries")
        .join(HELPER_SIDECAR_FILE)
}

fn target_debug_exe_path(root: &Path) -> PathBuf {
    root.join("target").join("debug").join("audiobook-boss")
}

fn packaged_exe_path(root: &Path) -> PathBuf {
    root.join("AudioBook Boss.app")
        .join("Contents")
        .join("MacOS")
        .join("audiobook-boss")
}

fn packaged_helper_path(root: &Path) -> PathBuf {
    root.join("AudioBook Boss.app")
        .join("Contents")
        .join("MacOS")
        .join(HELPER_NAME)
}

#[test]
fn env_helper_path_wins() {
    let root = tempfile::TempDir::new().expect("temp root");
    let env_path = root.path().join("custom-helper");

    assert_eq!(
        resolve_helper_path_from(Some(env_path.clone()), None, None),
        env_path
    );
}

#[test]
fn packaged_sibling_helper_wins() {
    let root = tempfile::TempDir::new().expect("temp root");
    let exe = packaged_exe_path(root.path());
    let helper = packaged_helper_path(root.path());
    std::fs::create_dir_all(helper.parent().expect("helper parent")).expect("create bundle");
    make_executable(&helper);

    assert_eq!(resolve_helper_path_from(None, Some(exe), None), helper);
}

#[test]
fn dev_build_finds_repo_sidecar_from_target_debug_executable() {
    let root = tempfile::TempDir::new().expect("temp root");
    let exe = target_debug_exe_path(root.path());
    let helper = repo_sidecar_path(root.path());
    std::fs::create_dir_all(exe.parent().expect("exe parent")).expect("create target dir");
    std::fs::create_dir_all(helper.parent().expect("helper parent")).expect("create sidecar dir");
    make_executable(&helper);

    assert_eq!(resolve_helper_path_from(None, Some(exe), None), helper);
}

#[test]
fn dev_build_finds_repo_sidecar_from_repo_cwd() {
    let root = tempfile::TempDir::new().expect("temp root");
    let helper = repo_sidecar_path(root.path());
    std::fs::create_dir_all(helper.parent().expect("helper parent")).expect("create sidecar dir");
    make_executable(&helper);

    assert_eq!(
        resolve_helper_path_from(None, None, Some(root.path().to_path_buf())),
        helper
    );
}
