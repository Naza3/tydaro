use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(crate) struct TestDirectory(pub(crate) PathBuf);
impl TestDirectory {
    pub(crate) fn new() -> Self {
        let n = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("tydaro-document-test-{}-{n}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    pub(crate) fn file(&self) -> PathBuf {
        self.0.join("desktop.json")
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
