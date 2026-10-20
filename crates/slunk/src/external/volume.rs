//! The volume a destination is on: its free space, and ejecting it with
//! `diskutil`, so it can be unplugged (backend.md, "Syncing a folder library").

use std::ffi::{CStr, CString};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{Error, Result};

const DISKUTIL: &str = "/usr/sbin/diskutil";

/// The bytes free for the user on the volume a path is on.
pub fn free_space(path: &Path) -> Option<u64> {
    let path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stats: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `path` is a C string and `stats` is ours to fill
    if unsafe { libc::statvfs(path.as_ptr(), &mut stats) } != 0 {
        return None;
    }
    #[allow(clippy::unnecessary_cast)]
    Some(stats.f_bavail as u64 * stats.f_frsize as u64)
}

/// Where the volume a path is on is mounted.
#[cfg(target_os = "macos")]
pub fn mount_point(path: &Path) -> Option<PathBuf> {
    let path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stats: libc::statfs = unsafe { std::mem::zeroed() };
    // SAFETY: `path` is a C string and `stats` is ours to fill
    if unsafe { libc::statfs(path.as_ptr(), &mut stats) } != 0 {
        return None;
    }
    // SAFETY: the kernel ends `f_mntonname` with a null
    let mount = unsafe { CStr::from_ptr(stats.f_mntonname.as_ptr()) };
    Some(PathBuf::from(std::ffi::OsStr::from_bytes(mount.to_bytes())))
}

#[cfg(not(target_os = "macos"))]
pub fn mount_point(_path: &Path) -> Option<PathBuf> {
    None
}

/// The volume a path is on, when it's one that's ejected, like a card or a
/// drive: any mount but the Mac's own disk, `/` and the volumes in
/// `/System/Volumes`.
pub fn ejectable(path: &Path) -> Option<PathBuf> {
    mount_point(path)
        .filter(|mount| mount != Path::new("/") && !mount.starts_with("/System/Volumes"))
}

/// Ejects a volume, with `diskutil eject`, so it can be unplugged.
pub fn eject(mount: &Path) -> Result<()> {
    let output = Command::new(DISKUTIL)
        .arg("eject")
        .arg(mount)
        .output()
        .map_err(Error::io(Path::new(DISKUTIL)))?;
    if output.status.success() {
        return Ok(());
    }
    let said = String::from_utf8_lossy(&output.stderr);
    let said = said.trim();
    Err(Error::Invalid(if said.is_empty() {
        format!("{} couldn't be ejected", mount.display())
    } else {
        format!("{} couldn't be ejected: {said}", mount.display())
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_macs_own_disk_isnt_ejected() {
        let dir = tempfile::tempdir().unwrap();
        assert!(free_space(dir.path()).is_some_and(|free| free > 0));
        assert!(ejectable(dir.path()).is_none());
        #[cfg(target_os = "macos")]
        assert!(mount_point(dir.path()).is_some());
    }
}
