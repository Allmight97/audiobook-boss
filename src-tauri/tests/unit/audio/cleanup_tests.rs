#[cfg(test)]
mod tests {
    use crate::audio::cleanup::*;
    use tempfile::TempDir;
    use std::process::{Command, Stdio};
    
    #[test]
    fn test_cleanup_guard_creation() {
        let guard = CleanupGuard::new("test-session".to_string());
        assert_eq!(guard.session_id(), "test-session");
        assert_eq!(guard.path_count(), 0);
    }
    
    #[test]
    fn test_cleanup_guard_add_remove_paths() {
        let mut guard = CleanupGuard::new("test-session".to_string());
        let temp_path = PathBuf::from("/tmp/test");
        
        guard.add_path(&temp_path);
        assert_eq!(guard.path_count(), 1);
        
        let removed = guard.remove_path(&temp_path);
        assert!(removed);
        assert_eq!(guard.path_count(), 0);
        
        let removed_again = guard.remove_path(&temp_path);
        assert!(!removed_again);
    }
    
    #[test]
    fn test_cleanup_guard_add_multiple_paths() {
        let mut guard = CleanupGuard::new("test-session".to_string());
        let paths = vec![
            PathBuf::from("/tmp/test1"),
            PathBuf::from("/tmp/test2"),
            PathBuf::from("/tmp/test3"),
        ];
        
        guard.add_paths(&paths);
        assert_eq!(guard.path_count(), 3);
    }
    
    #[test]
    fn test_cleanup_guard_enable_disable() {
        let mut guard = CleanupGuard::new("test-session".to_string());
        
        guard.disable_cleanup();
        // We can't easily test the actual cleanup behavior without creating real files
        // but we can test the state changes
        
        guard.enable_cleanup();
    }
    
    // Additional tests would be extracted here...
}
