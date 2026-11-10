//! Quotas apply only to regenerable PCM caches; never to source media or voicebanks.
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Limits {
    pub cache_bytes: u64,
    pub minimum_free_bytes: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            cache_bytes: 4 * 1024 * 1024 * 1024,
            minimum_free_bytes: 64 * 1024 * 1024,
        }
    }
}
#[cfg(unix)]
pub fn available(path: &Path) -> Result<u64> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(path.as_os_str().as_bytes())?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: CString is terminated; stat points to a correctly sized writable structure.
    ensure!(
        unsafe { libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) } == 0,
        "free-space query failed: {}",
        std::io::Error::last_os_error()
    );
    let stat = unsafe { stat.assume_init() };
    Ok((u128::from(stat.f_bavail) * u128::from(stat.f_frsize)).min(u128::from(u64::MAX)) as u64)
}
#[cfg(windows)]
pub fn available(path: &Path) -> Result<u64> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetDiskFreeSpaceExW(
            path: *const u16,
            available: *mut u64,
            total: *mut u64,
            free: *mut u64,
        ) -> i32;
    }
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut free = 0;
    ensure!(
        unsafe {
            GetDiskFreeSpaceExW(
                path.as_ptr(),
                &mut free,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        } != 0,
        "free-space query failed"
    );
    Ok(free)
}
#[cfg(not(any(unix, windows)))]
pub fn available(_: &Path) -> Result<u64> {
    anyhow::bail!("free-space queries unsupported on this platform")
}
pub fn preflight(path: &Path, bytes: u64, reserve: u64) -> Result<()> {
    let directory = if path.is_dir() {
        path
    } else {
        path.parent()
            .context("output requires a parent directory")?
    };
    ensure!(
        available(directory)? >= bytes.saturating_add(reserve),
        "insufficient disk space: need {bytes} bytes plus {reserve} bytes safety reserve"
    );
    Ok(())
}
struct Lock(PathBuf, Option<std::fs::File>);
impl Drop for Lock {
    fn drop(&mut self) {
        self.1.take();
        let _ = fs::remove_file(&self.0);
    }
}
pub struct Lease {
    pub path: PathBuf,
    reservation: PathBuf,
}
impl Lease {
    pub fn reserve(path: PathBuf, bytes: u64, limits: &Limits) -> Result<Self> {
        let root = path.parent().context("cache needs a directory")?;
        fs::create_dir_all(root)?;
        ensure!(
            !fs::symlink_metadata(root)?.file_type().is_symlink(),
            "cache directory cannot be a symlink"
        );
        let lock_path = root.join(".budget-lock");
        let lock = OpenOptions::new().write(true).create_new(true).open(&lock_path).context("cache budget is busy or has an interrupted lock; close other instances before removing cache/.budget-lock")?;
        let mut guard = Lock(lock_path, Some(lock));
        writeln!(guard.1.as_mut().unwrap(), "{}", std::process::id())?;
        let mut occupied = 0u64;
        let mut orphans = vec![];
        for (i, entry) in fs::read_dir(root)?.enumerate() {
            ensure!(i < 16384, "cache directory entry limit 16384");
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let file = entry.path();
            if file.extension().is_some_and(|s| s == "reserve") {
                ensure!(file.metadata()?.len() <= 32, "invalid cache reservation");
                occupied =
                    occupied.saturating_add(fs::read_to_string(&file)?.trim().parse::<u64>()?);
            } else if file.extension().is_some_and(|s| s == "raw")
                && !file.with_extension("reserve").exists()
                && file
                    .file_name()
                    .is_some_and(|s| s.to_string_lossy().starts_with("pcm-"))
            {
                let meta = file.metadata()?;
                occupied = occupied.saturating_add(meta.len());
                orphans.push((meta.modified()?, file, meta.len()));
            }
        }
        ensure!(
            bytes <= limits.cache_bytes,
            "decoded asset exceeds aggregate cache quota"
        );
        orphans.sort_by_key(|v| v.0);
        for (_, file, size) in orphans {
            if occupied.saturating_add(bytes) <= limits.cache_bytes {
                break;
            }
            fs::remove_file(file)?;
            occupied = occupied.saturating_sub(size);
        }
        ensure!(
            occupied.saturating_add(bytes) <= limits.cache_bytes,
            "aggregate cache quota exceeded; active caches are retained"
        );
        preflight(root, bytes, limits.minimum_free_bytes)?;
        let reservation = path.with_extension("reserve");
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&reservation)?;
        let result = write!(file, "{bytes}").and_then(|_| file.sync_all());
        drop(file);
        if let Err(error) = result {
            let _ = fs::remove_file(&reservation);
            return Err(error.into());
        }
        Ok(Self { path, reservation })
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        // File handles owned by Asset are dropped before its lease (also on Windows).
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_file(&self.reservation);
    }
}
