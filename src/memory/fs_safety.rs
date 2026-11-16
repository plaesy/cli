//! Descriptor-relative filesystem helpers.
//!
//! `std::fs` path-based APIs re-resolve every path component on each call. Between a
//! `canonicalize()` check and a later `create_dir`/`write`/`rename`, an attacker (or just an
//! unlucky concurrent process) can replace a directory component with a symlink and redirect
//! the operation outside the intended root (TOCTOU).
//!
//! This module opens the target directory exactly once with `O_DIRECTORY | O_NOFOLLOW` (so the
//! final path component cannot itself be a symlink), then performs every subsequent operation
//! with `*at` syscalls relative to that open file descriptor. The descriptor keeps pointing at
//! the same inode for the rest of the operation regardless of what happens to the path later,
//! which is the actual guarantee `openat`-style APIs provide over path-based ones.
//!
//! Scope: this protects the *final* directory component used for note/lock publication on
//! Linux. It does not re-verify every ancestor component on every call (that would need a
//! `resolve_beneath`-style walk); `write_note`'s existing `canonicalize` + `starts_with` check
//! is still responsible for rejecting a root that has already been redirected before the first
//! `open_dir_nofollow` call. On non-Linux platforms these helpers fall back to the path-based
//! `std::fs` equivalents, which keep today's (slightly weaker but still real) protection.

use std::io;
use std::path::Path;
#[cfg(target_os = "linux")]
use std::{
    ffi::CString,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::ffi::OsStrExt,
    },
};

/// An open directory descriptor used as the base for `*at` operations.
pub struct DirHandle {
    #[cfg(target_os = "linux")]
    fd: OwnedFd,
    #[cfg(not(target_os = "linux"))]
    path: std::path::PathBuf,
}

#[cfg(target_os = "linux")]
fn to_cstring(path: &Path) -> io::Result<CString> {
    CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains a NUL byte"))
}

#[cfg(target_os = "linux")]
fn to_component_cstring(path: &Path) -> io::Result<CString> {
    use std::path::Component;
    if path.components().count() != 1
        || !matches!(path.components().next(), Some(Component::Normal(_)))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "name must be one normal path component",
        ));
    }
    to_cstring(path)
}

impl DirHandle {
    /// Clone this directory capability while keeping it anchored to the same inode.
    pub fn try_clone(&self) -> io::Result<Self> {
        #[cfg(target_os = "linux")]
        {
            Ok(Self {
                fd: self.fd.try_clone()?,
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            Ok(Self {
                path: self.path.clone(),
            })
        }
    }

    /// Read metadata for the directory represented by this handle.
    #[cfg(target_os = "linux")]
    pub fn metadata(&self) -> io::Result<std::fs::Metadata> {
        std::fs::File::from(self.fd.try_clone()?).metadata()
    }

    #[cfg(not(target_os = "linux"))]
    pub fn metadata(&self) -> io::Result<std::fs::Metadata> {
        std::fs::metadata(&self.path)
    }

    /// Restrict permissions on the directory represented by this handle.
    #[cfg(target_os = "linux")]
    pub fn set_permissions(&self, mode: u32) -> io::Result<()> {
        let rc = unsafe { libc::fchmod(self.fd.as_raw_fd(), mode as libc::mode_t) };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    pub fn set_permissions(&self, mode: u32) -> io::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(mode))
    }

    #[cfg(windows)]
    pub fn set_permissions(&self, _mode: u32) -> io::Result<()> {
        Ok(())
    }

    /// Create a single directory relative to this directory handle.
    #[cfg(target_os = "linux")]
    pub fn create_dir(&self, name: &Path, mode: u32) -> io::Result<()> {
        let c_name = to_component_cstring(name)?;
        let rc =
            unsafe { libc::mkdirat(self.fd.as_raw_fd(), c_name.as_ptr(), mode as libc::mode_t) };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn create_dir(&self, name: &Path, _mode: u32) -> io::Result<()> {
        std::fs::create_dir(self.child_path_fallback(name)?)
    }

    /// Open a directory beneath this handle, rejecting symlinks and escape paths atomically.
    #[cfg(target_os = "linux")]
    pub fn open_dir_beneath(&self, relative: &Path) -> io::Result<Self> {
        use std::path::Component;

        if relative.as_os_str().is_empty()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "path must contain only normal relative components",
            ));
        }
        #[repr(C)]
        struct OpenHow {
            flags: u64,
            mode: u64,
            resolve: u64,
        }
        const RESOLVE_NO_MAGICLINKS: u64 = 0x02;
        const RESOLVE_NO_SYMLINKS: u64 = 0x04;
        const RESOLVE_BENEATH: u64 = 0x08;
        let path = to_cstring(relative)?;
        let how = OpenHow {
            flags: (libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC) as u64,
            mode: 0,
            resolve: RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS,
        };
        let fd = unsafe {
            libc::syscall(
                libc::SYS_openat2,
                self.fd.as_raw_fd(),
                path.as_ptr(),
                &how as *const OpenHow,
                std::mem::size_of::<OpenHow>(),
            ) as libc::c_int
        };
        if fd >= 0 {
            return Ok(Self {
                fd: unsafe { OwnedFd::from_raw_fd(fd) },
            });
        }
        let error = io::Error::last_os_error();
        if !matches!(
            error.raw_os_error(),
            Some(libc::ENOSYS) | Some(libc::EINVAL) | Some(libc::E2BIG)
        ) {
            return Err(error);
        }

        // Older Linux kernels may not provide openat2; walk each component with openat and
        // O_NOFOLLOW so the fallback retains the same symlink/escape protections.
        let mut current = self.fd.try_clone()?;
        for component in relative.components() {
            let Component::Normal(name) = component else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid relative path",
                ));
            };
            let c_name = CString::new(name.as_bytes()).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "path contains a NUL byte")
            })?;
            let next_fd = unsafe {
                libc::openat(
                    current.as_raw_fd(),
                    c_name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if next_fd < 0 {
                return Err(io::Error::last_os_error());
            }
            current = unsafe { OwnedFd::from_raw_fd(next_fd) };
        }
        Ok(Self { fd: current })
    }

    /// Open `dir` without following symlinks in any path component.
    #[cfg(target_os = "linux")]
    pub fn open_nofollow(dir: &Path) -> io::Result<Self> {
        use std::path::Component;

        let anchor = if dir.is_absolute() { "/" } else { "." };
        let anchor = CString::new(anchor).expect("constant path has no NUL byte");
        let root_fd = unsafe {
            libc::open(
                anchor.as_ptr(),
                libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if root_fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut current = unsafe { OwnedFd::from_raw_fd(root_fd) };
        for component in dir.components() {
            let name = match component {
                Component::RootDir | Component::CurDir => continue,
                Component::Normal(name) => name,
                Component::ParentDir | Component::Prefix(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "directory path must not contain parent or prefix components",
                    ));
                }
            };
            let c_name = CString::new(name.as_bytes()).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "path contains a NUL byte")
            })?;
            let fd = unsafe {
                libc::openat(
                    current.as_raw_fd(),
                    c_name.as_ptr(),
                    libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            current = unsafe { OwnedFd::from_raw_fd(fd) };
        }
        Ok(Self { fd: current })
    }

    #[cfg(not(target_os = "linux"))]
    pub fn open_nofollow(dir: &Path) -> io::Result<Self> {
        // Non-Linux fallback: still reject a symlinked directory, just without the atomic
        // guarantee that a path-based open provides on Linux via O_NOFOLLOW + O_DIRECTORY.
        let meta = std::fs::symlink_metadata(dir)?;
        if !meta.file_type().is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a directory (or is a symlink)",
            ));
        }
        Ok(Self {
            path: std::fs::canonicalize(dir)?,
        })
    }

    /// Open a regular file at a relative path, refusing symlinks in every component on Linux.
    #[cfg(target_os = "linux")]
    pub fn open_file_read_relative_nofollow(&self, path: &Path) -> io::Result<std::fs::File> {
        use std::path::Component;

        let mut components = path.components().peekable();
        let mut current = self.fd.try_clone()?;
        while let Some(component) = components.next() {
            let name = match component {
                Component::Normal(name) => name,
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "relative path must contain only normal components",
                    ));
                }
            };
            let c_name = CString::new(name.as_bytes()).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "path contains a NUL byte")
            })?;
            if components.peek().is_none() {
                let fd = unsafe {
                    libc::openat(
                        current.as_raw_fd(),
                        c_name.as_ptr(),
                        libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
                    )
                };
                if fd < 0 {
                    return Err(io::Error::last_os_error());
                }
                let file = unsafe { std::fs::File::from_raw_fd(fd) };
                if !file.metadata()?.is_file() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "not a regular file",
                    ));
                }
                return Ok(file);
            }
            let fd = unsafe {
                libc::openat(
                    current.as_raw_fd(),
                    c_name.as_ptr(),
                    libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            current = unsafe { OwnedFd::from_raw_fd(fd) };
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path must name a file",
        ))
    }

    #[cfg(not(target_os = "linux"))]
    pub fn open_file_read_relative_nofollow(&self, path: &Path) -> io::Result<std::fs::File> {
        use std::path::Component;
        if path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid relative path",
            ));
        }
        let full = self.child_path_fallback(path)?;
        let metadata = std::fs::symlink_metadata(&full)?;
        if !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a regular file",
            ));
        }
        std::fs::File::open(full)
    }

    /// Open a regular file directly inside this directory, refusing a symlink at the final component.
    #[cfg(target_os = "linux")]
    pub fn open_file_read_nofollow(&self, name: &Path) -> io::Result<std::fs::File> {
        use std::path::Component;

        if name.components().count() != 1
            || !matches!(name.components().next(), Some(Component::Normal(_)))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "child name must be one normal path component",
            ));
        }
        let c_name = to_cstring(name)?;
        let fd = unsafe {
            libc::openat(
                self.fd.as_raw_fd(),
                c_name.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let file = unsafe { std::fs::File::from_raw_fd(fd) };
        if !file.metadata()?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a regular file",
            ));
        }
        Ok(file)
    }

    #[cfg(not(target_os = "linux"))]
    pub fn open_file_read_nofollow(&self, name: &Path) -> io::Result<std::fs::File> {
        use std::path::Component;

        if name.components().count() != 1
            || !matches!(name.components().next(), Some(Component::Normal(_)))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "child name must be one normal path component",
            ));
        }
        let path = self.child_path_fallback(name)?;
        let metadata = std::fs::symlink_metadata(&path)?;
        if !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a regular file",
            ));
        }
        std::fs::File::open(path)
    }

    /// Open or create a regular file directly inside this directory without following symlinks.
    #[cfg(target_os = "linux")]
    pub fn open_or_create_file(&self, name: &Path, mode: u32) -> io::Result<std::fs::File> {
        let c_name = to_component_cstring(name)?;
        let fd = unsafe {
            libc::openat(
                self.fd.as_raw_fd(),
                c_name.as_ptr(),
                libc::O_RDWR
                    | libc::O_CREAT
                    | libc::O_NOFOLLOW
                    | libc::O_CLOEXEC
                    | libc::O_NONBLOCK
                    | libc::O_NOCTTY,
                mode as libc::mode_t,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let file = unsafe { std::fs::File::from_raw_fd(fd) };
        if !file.metadata()?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a regular file",
            ));
        }
        Ok(file)
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    pub fn open_or_create_file(&self, name: &Path, _mode: u32) -> io::Result<std::fs::File> {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(self.child_component_fallback(name)?)
    }

    #[cfg(windows)]
    pub fn open_or_create_file(&self, name: &Path, _mode: u32) -> io::Result<std::fs::File> {
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(self.child_path_fallback(name)?)
    }

    #[cfg(not(any(unix, windows)))]
    pub fn open_or_create_file(&self, name: &Path, _mode: u32) -> io::Result<std::fs::File> {
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(self.child_component_fallback(name)?)
    }

    /// Create a regular file named `name` directly inside this directory, failing if it already
    /// exists (`O_EXCL`), without ever re-resolving the directory path.
    #[cfg(target_os = "linux")]
    pub fn create_new_file(&self, name: &Path) -> io::Result<std::fs::File> {
        let c_name = to_component_cstring(name)?;
        let fd = unsafe {
            libc::openat(
                self.fd.as_raw_fd(),
                c_name.as_ptr(),
                libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(unsafe { std::fs::File::from_raw_fd(fd) })
    }

    #[cfg(not(target_os = "linux"))]
    pub fn create_new_file(&self, name: &Path) -> io::Result<std::fs::File> {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.child_path_fallback(name)?)
    }

    /// Hard-link `old_name` (inside this directory) to `new_name` (inside this directory),
    /// resolved relative to the held descriptor rather than by re-walking the path.
    #[cfg(target_os = "linux")]
    pub fn link_within(&self, old_name: &Path, new_name: &Path) -> io::Result<()> {
        let c_old = to_component_cstring(old_name)?;
        let c_new = to_component_cstring(new_name)?;
        let rc = unsafe {
            libc::linkat(
                self.fd.as_raw_fd(),
                c_old.as_ptr(),
                self.fd.as_raw_fd(),
                c_new.as_ptr(),
                0,
            )
        };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// Atomically replace `new_name` with `old_name` inside this directory.
    #[cfg(target_os = "linux")]
    pub fn replace_within(&self, old_name: &Path, new_name: &Path) -> io::Result<()> {
        let c_old = to_component_cstring(old_name)?;
        let c_new = to_component_cstring(new_name)?;
        let rc = unsafe {
            libc::renameat(
                self.fd.as_raw_fd(),
                c_old.as_ptr(),
                self.fd.as_raw_fd(),
                c_new.as_ptr(),
            )
        };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn link_within(&self, old_name: &Path, new_name: &Path) -> io::Result<()> {
        std::fs::hard_link(
            self.child_path_fallback(old_name)?,
            self.child_path_fallback(new_name)?,
        )
    }

    /// Atomically replace `new_name` with `old_name` inside this directory.
    #[cfg(not(target_os = "linux"))]
    pub fn replace_within(&self, old_name: &Path, new_name: &Path) -> io::Result<()> {
        std::fs::rename(
            self.child_path_fallback(old_name)?,
            self.child_path_fallback(new_name)?,
        )
    }

    /// Remove an empty directory named `name` directly inside this directory.
    #[cfg(target_os = "linux")]
    pub fn remove_dir(&self, name: &Path) -> io::Result<()> {
        let c_name = to_component_cstring(name)?;
        let rc =
            unsafe { libc::unlinkat(self.fd.as_raw_fd(), c_name.as_ptr(), libc::AT_REMOVEDIR) };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn remove_dir(&self, name: &Path) -> io::Result<()> {
        std::fs::remove_dir(self.child_component_fallback(name)?)
    }

    /// Remove a regular file named `name` directly inside this directory.
    #[cfg(target_os = "linux")]
    pub fn remove_file(&self, name: &Path) -> io::Result<()> {
        let c_name = to_component_cstring(name)?;
        let rc = unsafe { libc::unlinkat(self.fd.as_raw_fd(), c_name.as_ptr(), 0) };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn remove_file(&self, name: &Path) -> io::Result<()> {
        std::fs::remove_file(self.child_path_fallback(name)?)
    }

    /// Flush this directory's own metadata/entries to durable storage.
    #[cfg(target_os = "linux")]
    pub fn sync(&self) -> io::Result<()> {
        let rc = unsafe { libc::fsync(self.fd.as_raw_fd()) };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn sync(&self) -> io::Result<()> {
        // File contents are synced before publication. Rust has no portable directory fsync API.
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    fn child_component_fallback(&self, name: &Path) -> std::io::Result<std::path::PathBuf> {
        use std::path::Component;
        if name.components().count() != 1
            || !matches!(name.components().next(), Some(Component::Normal(_)))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "name must be one normal path component",
            ));
        }
        Ok(self.path.join(name))
    }

    #[cfg(not(target_os = "linux"))]
    fn child_path_fallback(&self, name: &Path) -> std::io::Result<std::path::PathBuf> {
        use std::path::Component;

        if name
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "child name must be a relative path without parent components",
            ));
        }
        Ok(self.path.join(name))
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn rejects_symlinked_directory() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir(&real).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert!(DirHandle::open_nofollow(&link).is_err());
        assert!(DirHandle::open_nofollow(&real).is_ok());
    }

    #[test]
    fn rejects_symlinked_ancestor_directory() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir(&real).unwrap();
        let child = real.join("child");
        std::fs::create_dir(&child).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert!(DirHandle::open_nofollow(&link.join("child")).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn relative_file_open_rejects_symlinked_intermediate_and_leaf() {
        use std::io::Read;
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let actual = dir.path().join("actual");
        std::fs::create_dir(&actual).unwrap();
        std::fs::write(actual.join("value"), b"inside").unwrap();
        let linked_dir = dir.path().join("linked");
        symlink(&actual, &linked_dir).unwrap();
        let linked_file = dir.path().join("linked-file");
        symlink(actual.join("value"), &linked_file).unwrap();
        let handle = DirHandle::open_nofollow(dir.path()).unwrap();

        assert!(handle
            .open_file_read_relative_nofollow(Path::new("linked/value"))
            .is_err());
        assert!(handle
            .open_file_read_relative_nofollow(Path::new("linked-file"))
            .is_err());
        let mut file = handle
            .open_file_read_relative_nofollow(Path::new("actual/value"))
            .unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        assert_eq!(contents, "inside");
    }

    #[test]
    fn open_dir_beneath_pins_directory_against_path_replacement() {
        let root = tempfile::tempdir().unwrap();
        let tasks = root.path().join("tasks");
        let task = tasks.join("todo/item");
        std::fs::create_dir_all(&task).unwrap();
        let outside = root.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        let root_handle = DirHandle::open_nofollow(&tasks).unwrap();
        let task_handle = root_handle
            .open_dir_beneath(Path::new("todo/item"))
            .unwrap();

        std::fs::rename(&task, tasks.join("todo/item-original")).unwrap();
        std::os::unix::fs::symlink(&outside, &task).unwrap();
        let mut file = task_handle.create_new_file(Path::new("sentinel")).unwrap();
        use std::io::Write;
        file.write_all(b"pinned").unwrap();

        assert_eq!(
            std::fs::read(outside.join("sentinel")).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        assert_eq!(
            std::fs::read(tasks.join("todo/item-original/sentinel")).unwrap(),
            b"pinned"
        );
        assert!(root_handle
            .open_dir_beneath(Path::new("todo/item/../item-original"))
            .is_err());
    }

    #[test]
    fn create_link_remove_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let handle = DirHandle::open_nofollow(dir.path()).unwrap();
        let tmp = Path::new(".note.tmp");
        let dest = Path::new("note.md");
        {
            use std::io::Write;
            let mut f = handle.create_new_file(tmp).unwrap();
            f.write_all(b"hello").unwrap();
            f.sync_all().unwrap();
        }
        handle.link_within(tmp, dest).unwrap();
        handle.remove_file(tmp).unwrap();
        handle.sync().unwrap();
        let content = std::fs::read_to_string(dir.path().join("note.md")).unwrap();
        assert_eq!(content, "hello");
        // Destination already exists: must fail rather than silently replace it.
        assert!(handle.create_new_file(dest).is_err());
    }

    #[test]
    fn descriptor_relative_mutations_reject_path_escape() {
        use std::io::Write;

        let root = tempfile::tempdir().unwrap();
        let held = root.path().join("held");
        let outside = root.path().join("outside");
        std::fs::create_dir(&held).unwrap();
        std::fs::create_dir(&outside).unwrap();
        let sentinel = outside.join("sentinel");
        std::fs::write(&sentinel, b"unchanged").unwrap();
        let handle = DirHandle::open_nofollow(&held).unwrap();

        assert!(handle
            .create_new_file(Path::new("../outside/created"))
            .is_err());
        assert!(handle.create_new_file(&sentinel).is_err());
        assert!(handle
            .link_within(Path::new("../outside/sentinel"), Path::new("linked"))
            .is_err());
        assert!(handle
            .link_within(Path::new("sentinel"), Path::new("../outside/linked"))
            .is_err());
        assert!(handle
            .replace_within(Path::new("../outside/sentinel"), Path::new("replaced"))
            .is_err());
        assert!(handle
            .replace_within(Path::new("missing"), Path::new("../outside/replaced"))
            .is_err());
        assert!(handle
            .remove_file(Path::new("../outside/sentinel"))
            .is_err());
        assert!(handle.remove_file(&sentinel).is_err());

        let mut created = handle.create_new_file(Path::new("inside")).unwrap();
        created.write_all(b"inside").unwrap();
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"unchanged");
        assert!(!outside.join("created").exists());
        assert!(!outside.join("linked").exists());
        assert!(!outside.join("replaced").exists());
    }

    #[test]
    fn open_or_create_file_does_not_block_on_fifo() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("pipe");
        let fifo_c = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
        let rc = unsafe { libc::mkfifo(fifo_c.as_ptr(), 0o600) };
        assert_eq!(rc, 0, "mkfifo failed: {}", io::Error::last_os_error());

        let handle = DirHandle::open_nofollow(dir.path()).unwrap();
        assert!(handle
            .open_or_create_file(Path::new("pipe"), 0o600)
            .is_err());
    }

    #[test]
    fn create_new_file_rejects_existing_symlink_target() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let bait = outside.path().join("bait");
        std::fs::write(&bait, "outside").unwrap();
        let link_name = Path::new("escape");
        std::os::unix::fs::symlink(&bait, dir.path().join(link_name)).unwrap();
        let handle = DirHandle::open_nofollow(dir.path()).unwrap();
        // O_EXCL already rejects anything pre-existing at that name, symlink or not.
        assert!(handle.create_new_file(link_name).is_err());
        assert_eq!(std::fs::read_to_string(&bait).unwrap(), "outside");
    }
}
