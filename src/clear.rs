use crate::errors::{AcsError, ConfigError};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearTargetKind {
    FileOrDir,
    DirContents,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClearTarget {
    pub path: PathBuf,
    pub kind: ClearTargetKind,
}

impl ClearTarget {
    pub fn file_or_dir(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into(), kind: ClearTargetKind::FileOrDir }
    }

    pub fn dir_contents(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into(), kind: ClearTargetKind::DirContents }
    }

    pub fn absolute_path(&self) -> Result<PathBuf, AcsError> {
        absolute_path(&self.path)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ClearStats {
    pub removed: usize,
}

impl ClearStats {
    fn add(&mut self, count: usize) {
        self.removed += count;
    }
}

fn absolute_path(path: &Path) -> Result<PathBuf, AcsError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    let cwd = std::env::current_dir().map_err(|e| ConfigError::load(path, e))?;
    Ok(cwd.join(path))
}

fn remove_entry(path: &Path) -> Result<usize, AcsError> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.file_type().is_dir() && !meta.file_type().is_symlink() {
                fs::remove_dir_all(path).map_err(|e| ConfigError::remove(path, e))?;
            } else {
                fs::remove_file(path).map_err(|e| ConfigError::remove(path, e))?;
            }
            Ok(1)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(e) => Err(ConfigError::remove(path, e).into()),
    }
}

pub fn clear_dir_contents(path: impl AsRef<Path>) -> Result<ClearStats, AcsError> {
    let path = path.as_ref();
    let mut stats = ClearStats::default();

    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.file_type().is_symlink() || !meta.file_type().is_dir() {
                stats.add(remove_entry(path)?);
                return Ok(stats);
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(stats),
        Err(e) => return Err(ConfigError::remove(path, e).into()),
    }

    for entry in fs::read_dir(path).map_err(|e| ConfigError::load(path, e))? {
        let entry = entry.map_err(|e| ConfigError::load(path, e))?;
        stats.add(remove_entry(&entry.path())?);
    }

    Ok(stats)
}

pub fn clear_targets(targets: &[ClearTarget]) -> Result<ClearStats, AcsError> {
    let mut stats = ClearStats::default();
    for target in targets {
        match target.kind {
            ClearTargetKind::FileOrDir => stats.add(remove_entry(&target.path)?),
            ClearTargetKind::DirContents => stats.add(clear_dir_contents(&target.path)?.removed),
        }
    }
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn setup_temp_dir() -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("acp_clear_test_{}", id));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[cfg(unix)]
    fn symlink_dir(src: &Path, dst: &Path) {
        std::os::unix::fs::symlink(src, dst).unwrap();
    }

    #[test]
    fn test_clear_dir_contents_keeps_parent_and_removes_entries() {
        let dir = setup_temp_dir();
        let target = dir.join("sessions");
        fs::create_dir_all(target.join("nested")).unwrap();
        fs::write(target.join("entry.txt"), "entry").unwrap();
        fs::write(target.join("nested").join("entry.txt"), "nested").unwrap();

        let stats = clear_dir_contents(&target).unwrap();

        assert_eq!(stats.removed, 2);
        assert!(target.exists());
        assert!(fs::read_dir(target).unwrap().next().is_none());
    }

    #[test]
    fn test_clear_dir_contents_removes_file_target() {
        let dir = setup_temp_dir();
        let target = dir.join("sessions");
        fs::write(&target, "not a directory").unwrap();

        let stats = clear_dir_contents(&target).unwrap();

        assert_eq!(stats.removed, 1);
        assert!(!target.exists());
    }

    #[test]
    fn test_clear_dir_contents_ignores_missing_target() {
        let dir = setup_temp_dir();
        let target = dir.join("missing");

        let stats = clear_dir_contents(&target).unwrap();

        assert_eq!(stats.removed, 0);
        assert!(!target.exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_clear_dir_contents_removes_symlink_without_touching_destination() {
        let dir = setup_temp_dir();
        let destination = dir.join("destination");
        let target = dir.join("sessions");
        fs::create_dir_all(&destination).unwrap();
        fs::write(destination.join("entry.txt"), "entry").unwrap();
        symlink_dir(&destination, &target);

        let stats = clear_dir_contents(&target).unwrap();

        assert_eq!(stats.removed, 1);
        assert!(!target.exists());
        assert!(destination.join("entry.txt").exists());
    }
}
