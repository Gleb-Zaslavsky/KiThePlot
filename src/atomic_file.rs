//! Small same-directory atomic file replacement helper.

use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

/// A unique sibling path that is removed on drop unless committed.
pub(crate) struct TempSibling {
    path: std::path::PathBuf,
    committed: bool,
}

impl TempSibling {
    pub(crate) fn new(destination: &Path) -> std::io::Result<Self> {
        let parent = destination
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let file_name = destination.file_name().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "destination has no file name",
            )
        })?;
        loop {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let mut name = destination.file_stem().unwrap_or(file_name).to_os_string();
            name.push(format!(".{}.{}", std::process::id(), id));
            if let Some(extension) = destination.extension() {
                name.push(".");
                name.push(extension);
            } else {
                name.push(".tmp");
            }
            let path = parent.join(name);
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(file) => {
                    drop(file);
                    return Ok(Self {
                        path,
                        committed: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn commit(mut self, destination: &Path) -> std::io::Result<()> {
        std::fs::rename(&self.path, destination)?;
        self.committed = true;
        Ok(())
    }
}

impl Drop for TempSibling {
    fn drop(&mut self) {
        if !self.committed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Write bytes to a unique sibling file and replace the destination only after
/// the complete contents have reached the filesystem.
pub(crate) fn write_atomic(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let file_name = path.file_name().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "destination has no file name",
        )
    })?;

    let (temp_path, mut temp_file) = loop {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let mut temp_name = file_name.to_os_string();
        temp_name.push(format!(".{}.{}.tmp", std::process::id(), id));
        let candidate = parent.join(temp_name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => break (candidate, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };

    let result = (|| {
        temp_file.write_all(contents)?;
        temp_file.sync_all()?;
        drop(temp_file);
        std::fs::rename(&temp_path, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::write_atomic;

    #[test]
    fn replaces_destination_only_after_full_write() {
        let path = std::env::temp_dir().join(format!(
            "kitheplot_atomic_{}_{}.kplot",
            std::process::id(),
            super::NEXT_TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::write(&path, b"old").expect("fixture should be written");
        write_atomic(&path, b"new complete contents").expect("atomic replacement should work");
        assert_eq!(std::fs::read(&path).unwrap(), b"new complete contents");
        std::fs::remove_file(path).expect("fixture should be removed");
    }
}
