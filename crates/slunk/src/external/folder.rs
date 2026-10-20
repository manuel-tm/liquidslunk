//! A destination's files (4_external_libraries.md §6.3): a folder library's
//! folder, or an iPod's disk. Files reach their final path whole, written
//! under a temporary name in liquidslunk's temporary folder there and then
//! renamed, which is atomic on one volume; folders are made as they're
//! needed and removed when a sync leaves them empty; and there are no
//! AppleDouble files.
//!
//! **AppleDouble files.** macOS gives every file and folder a process
//! creates an extended attribute, `com.apple.provenance`, which can't be
//! removed. FAT32 and exFAT keep extended attributes in a `._` file beside
//! the file, which some players list as a broken track. Opening an existing
//! file for writing adds one too, while a rename doesn't. So every file is
//! written new, never rewritten in place, and the `._` file each new file or
//! folder gets is deleted as soon as it's made: it's liquidslunk's own.

use std::fs::{self, File};
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

use super::manifest::ManifestFolder;
use crate::{Error, Result};

/// liquidslunk's own folder at a folder library's root
pub const OWN: &str = ".liquidslunk";

/// Where an iPod keeps its database, and liquidslunk its manifest
pub const IPOD_ITUNES: &str = "iPod_Control/iTunes";

/// A folder library's folder, or an iPod's disk.
#[derive(Debug, Clone)]
pub struct Folder {
    root: PathBuf,
    ipod: bool,
}

/// A file written under a temporary name, waiting to be put in place.
#[derive(Debug)]
pub struct Staged {
    path: PathBuf,
    pub size: u64,
    pub mtime: i64,
}

impl Folder {
    pub fn new(root: impl Into<PathBuf>) -> Folder {
        Folder {
            root: root.into(),
            ipod: false,
        }
    }

    /// An iPod's disk, at its mount point: its manifest and temporary
    /// folder are in `iPod_Control/iTunes`.
    pub fn ipod(mount: impl Into<PathBuf>) -> Folder {
        Folder {
            root: mount.into(),
            ipod: true,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Whether the folder is there.
    pub fn is_there(&self) -> bool {
        self.root.is_dir()
    }

    pub fn manifest(&self) -> ManifestFolder {
        if self.ipod {
            ManifestFolder::ipod(self.root.join(IPOD_ITUNES))
        } else {
            ManifestFolder::new(self.root.join(OWN))
        }
    }

    fn temporary_folder(&self) -> PathBuf {
        if self.ipod {
            self.root.join(IPOD_ITUNES).join("liquidslunk-tmp")
        } else {
            self.root.join(OWN).join("tmp")
        }
    }

    /// Readies it for a sync: makes its temporary folder, and empties it of
    /// what an earlier sync left there.
    pub fn prepare(&self) -> Result<()> {
        let temporary = self.temporary_folder();
        create_dirs(&temporary, &self.root)?;
        for entry in fs::read_dir(&temporary).map_err(Error::io(&temporary))? {
            let path = entry.map_err(Error::io(&temporary))?.path();
            remove_file(&path)?;
        }
        Ok(())
    }

    /// Removes its temporary folder once a sync is done with it.
    pub fn finish(&self) -> Result<()> {
        remove_empty(&self.temporary_folder()).map(|_| ())
    }

    /// A path in it, from one relative to it.
    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    /// A file's size and mtime, or `None` when there's no file there.
    pub fn stat(&self, relative: &str) -> Option<(u64, i64)> {
        stat(&self.path(relative))
    }

    /// Whether anything is at a path: a file, a folder or a link.
    pub fn exists(&self, relative: &str) -> bool {
        fs::symlink_metadata(self.path(relative)).is_ok()
    }

    /// Copies a file's bytes into the folder under a temporary name, and
    /// waits until they're on the disk. Only its bytes: none of its
    /// attributes.
    pub fn stage(&self, from: &Path) -> Result<Staged> {
        let mut source = File::open(from).map_err(Error::io(from))?;
        self.stage_with(|file| io::copy(&mut source, file).map(|_| ()))
    }

    /// Writes a file into the folder under a temporary name, with `write`,
    /// and waits until it's on the disk.
    pub fn stage_with(&self, write: impl FnOnce(&mut File) -> io::Result<()>) -> Result<Staged> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let name = format!(
            "{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let path = self.temporary_folder().join(name);
        let mut file = File::create(&path).map_err(Error::io(&path))?;
        let written = write(&mut file)
            .and_then(|()| file.sync_all())
            .map_err(Error::io(&path));
        drop(file);
        if let Err(error) = written {
            let _ = fs::remove_file(&path);
            return Err(error);
        }
        without_apple_double(&path)?;
        let (size, mtime) = stat(&path).ok_or_else(|| {
            Error::io(&path)(io::Error::new(ErrorKind::NotFound, "it was just written"))
        })?;
        Ok(Staged { path, size, mtime })
    }

    /// Puts a staged file at its path, over what's there, making its
    /// folders.
    pub fn place(&self, staged: Staged, relative: &str) -> Result<()> {
        let target = self.path(relative);
        if let Some(parent) = target.parent() {
            create_dirs(parent, &self.root)?;
        }
        fs::rename(&staged.path, &target).map_err(Error::io(&target))
    }

    /// Moves a file within the folder, making the folders it goes to.
    pub fn rename(&self, from: &str, to: &str) -> Result<()> {
        let target = self.path(to);
        if let Some(parent) = target.parent() {
            create_dirs(parent, &self.root)?;
        }
        fs::rename(self.path(from), &target).map_err(Error::io(&target))
    }

    /// Removes a file. One that's already gone is fine.
    pub fn remove(&self, relative: &str) -> Result<()> {
        remove_file(&self.path(relative))
    }

    /// Removes the folders that hold these paths when they're empty, and
    /// then their parents, up to the root, which stays.
    pub fn remove_empty_folders<'a>(&self, paths: impl IntoIterator<Item = &'a str>) -> Result<()> {
        let mut folders: Vec<PathBuf> = Vec::new();
        for path in paths {
            let mut folder = self.path(path);
            while folder.pop() && folder.starts_with(&self.root) && folder != self.root {
                folders.push(folder.clone());
            }
        }
        // The deepest first, so a parent is empty once its folders are gone
        folders.sort_by_key(|folder| std::cmp::Reverse(folder.components().count()));
        folders.dedup();
        for folder in folders {
            remove_empty(&folder)?;
        }
        Ok(())
    }
}

/// A file's size, and its mtime in nanoseconds since the epoch, or `None`
/// when there's no file there.
pub fn stat(path: &Path) -> Option<(u64, i64)> {
    let metadata = fs::metadata(path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    let mtime = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some((metadata.len(), i64::try_from(mtime.as_nanos()).ok()?))
}

/// Deletes the `._` file macOS made for a file or folder liquidslunk just
/// made, on a disk without extended attributes of its own.
pub fn without_apple_double(path: &Path) -> Result<()> {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Ok(());
    };
    let mut apple_double = std::ffi::OsString::from("._");
    apple_double.push(name);
    remove_file(&parent.join(apple_double))
}

/// Makes a folder inside `within`, and those above it up to `within`, each
/// without an AppleDouble file, except one that was there before it. Never
/// `within` itself, or anything outside it: when a destination's folder has
/// gone, with its disk ejected or unplugged, nothing is made in its place.
pub fn create_dirs(path: &Path, within: &Path) -> Result<()> {
    if !within.is_dir() {
        return Err(gone(within));
    }
    if path.is_dir() {
        return Ok(());
    }
    if !path.starts_with(within) || path == within {
        return Err(gone(path));
    }
    if let Some(parent) = path.parent()
        && parent != within
    {
        create_dirs(parent, within)?;
    }
    let had_one = apple_double_of(path).is_some_and(|path| path.exists());
    match fs::create_dir(path) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists && path.is_dir() => return Ok(()),
        Err(error) => return Err(Error::io(path)(error)),
    }
    if !had_one {
        without_apple_double(path)?;
    }
    Ok(())
}

/// A destination's folder that isn't there any more.
pub fn gone(folder: &Path) -> Error {
    Error::Invalid(format!(
        "{} isn't there any more: its disk may have been ejected or unplugged. Connect it again, and the next sync carries on from there.",
        folder.display()
    ))
}

/// Removes a folder if nothing at all is in it. Whether it was removed.
pub fn remove_empty(path: &Path) -> Result<bool> {
    let mut entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(Error::io(path)(error)),
    };
    if entries.next().is_some() {
        return Ok(false);
    }
    match fs::remove_dir(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        // Something appeared in it meanwhile
        Err(error) if error.kind() == ErrorKind::DirectoryNotEmpty => Ok(false),
        Err(error) => Err(Error::io(path)(error)),
    }
}

fn apple_double_of(path: &Path) -> Option<PathBuf> {
    let mut name = std::ffi::OsString::from("._");
    name.push(path.file_name()?);
    Some(path.parent()?.join(name))
}

fn remove_file(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Error::io(path)(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_staged_then_placed_and_empty_folders_go() {
        let dir = tempfile::tempdir().unwrap();
        let folder = Folder::new(dir.path().join("Car"));
        fs::create_dir(folder.root()).unwrap();
        folder.prepare().unwrap();
        let source = dir.path().join("source.bin");
        fs::write(&source, b"0123456789").unwrap();
        let staged = folder.stage(&source).unwrap();
        assert_eq!(staged.size, 10);
        folder.place(staged, "A/B/one.flac").unwrap();
        assert_eq!(
            fs::read(folder.path("A/B/one.flac")).unwrap(),
            b"0123456789"
        );
        fs::write(folder.path("A/theirs.txt"), b"").unwrap();
        folder.rename("A/B/one.flac", "C/one.flac").unwrap();
        folder.remove_empty_folders(["A/B/one.flac"]).unwrap();
        // B is empty and goes; A holds someone else's file and stays
        assert!(!folder.path("A/B").exists());
        assert!(folder.path("A/theirs.txt").exists());
        folder.remove("C/one.flac").unwrap();
        folder.remove("C/one.flac").unwrap();
        folder.remove_empty_folders(["C/one.flac"]).unwrap();
        assert!(!folder.path("C").exists());
        assert!(folder.root().exists());
    }

    #[test]
    fn a_new_file_loses_its_apple_double_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("one.flac");
        fs::write(&file, b"").unwrap();
        fs::write(dir.path().join("._one.flac"), b"").unwrap();
        without_apple_double(&file).unwrap();
        assert!(!dir.path().join("._one.flac").exists());
        // A folder's own, made before it, stays
        fs::write(dir.path().join("._A"), b"").unwrap();
        create_dirs(&dir.path().join("A/B"), dir.path()).unwrap();
        assert!(dir.path().join("._A").exists());
        // Nothing is made outside the folder, or in place of one that's gone
        let gone = dir.path().join("Gone");
        assert!(create_dirs(&gone.join("A"), &gone).is_err());
        assert!(create_dirs(&dir.path().join("Other"), &dir.path().join("A")).is_err());
        assert!(!gone.exists());
    }
}
