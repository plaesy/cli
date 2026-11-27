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
//! Scope: Linux rejects symlinks while walking every root component. Other Unix systems allow
//! ancestor resolution (needed for trusted system links such as macOS `/var`) but reject a symlink
//! at the final root component. All Unix operations after that initial open are descriptor-relative
//! and refuse symlinks in child components. Non-Unix platforms use path-based fallbacks.

use std::io;
use std::path::Path;
#[cfg(unix)]
use std::{
    ffi::CString,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::ffi::OsStrExt,
    },
};

/// An open directory descriptor used as the base for `*at` operations.
pub struct DirHandle {
    #[cfg(unix)]
    fd: OwnedFd,
    #[cfg(not(unix))]
    path: std::path::PathBuf,
}

#[cfg(unix)]
fn to_cstring(path: &Path) -> io::Result<CString> {
    CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains a NUL byte"))
}

#[cfg(unix)]
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
        #[cfg(unix)]
        {
            Ok(Self {
                fd: self.fd.try_clone()?,
            })
        }
        #[cfg(not(unix))]
        {
            Ok(Self {
                path: self.path.clone(),
            })
        }
    }

    /// Read metadata for the directory represented by this handle.
    #[cfg(unix)]
    pub fn metadata(&self) -> io::Result<std::fs::Metadata> {
        std::fs::File::from(self.fd.try_clone()?).metadata()
    }

    #[cfg(not(unix))]
    pub fn metadata(&self) -> io::Result<std::fs::Metadata> {
        std::fs::metadata(&self.path)
    }

    /// Restrict permissions on the directory represented by this handle.
    #[cfg(unix)]
    pub fn set_permissions(&self, mode: u32) -> io::Result<()> {
        // SAFETY: `self.fd` is an open descriptor owned by this handle, and the C string argument is NUL-terminated and alive for the call.
        let rc = unsafe { libc::fchmod(self.fd.as_raw_fd(), mode as libc::mode_t) };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    #[cfg(windows)]
    pub fn set_permissions(&self, _mode: u32) -> io::Result<()> {
        Ok(())
    }

    /// Create a single directory relative to this directory handle.
    #[cfg(unix)]
    pub fn create_dir(&self, name: &Path, mode: u32) -> io::Result<()> {
        let c_name = to_component_cstring(name)?;
        let rc =
            // SAFETY: `self.fd` is an open descriptor owned by this handle, and the C string argument is NUL-terminated and alive for the call.
            unsafe { libc::mkdirat(self.fd.as_raw_fd(), c_name.as_ptr(), mode as libc::mode_t) };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    #[cfg(not(unix))]
    pub fn create_dir(&self, name: &Path, _mode: u32) -> io::Result<()> {
        std::fs::create_dir(self.child_path_fallback(name)?)
    }

    /// Open a directory beneath this handle, rejecting symlinks and escape paths atomically.
    #[cfg(unix)]
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
        #[cfg(target_os = "linux")]
        {
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
            // SAFETY: `how` is a `repr(C)` struct matching the kernel's `open_how` layout and outlives the call; `path` is NUL-terminated; the size passed is `size_of::<OpenHow>()`.
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
                    // SAFETY: `fd` is a freshly opened descriptor that no other owner holds; taking it here makes this value its sole owner.
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
        }

        // Older Linux kernels may not provide openat2, and other Unix targets (macOS, BSD)
        // never do; walk each component with openat and
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
            // SAFETY: `current` is an open directory descriptor owned by this loop, and the C string argument is NUL-terminated and alive for the call.
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
            // SAFETY: `fd` is a freshly opened descriptor that no other owner holds; taking it here makes this value its sole owner.
            current = unsafe { OwnedFd::from_raw_fd(next_fd) };
        }
        Ok(Self { fd: current })
    }

    /// Open a directory beneath this handle. Non-Linux fallback: path-based, rejecting symlinks
    /// and parent-escape components, without the atomic guarantee of `openat2`.
    #[cfg(not(unix))]
    pub fn open_dir_beneath(&self, relative: &Path) -> io::Result<Self> {
        let full = self.child_path_fallback(relative)?;
        if relative.as_os_str().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "path must contain only normal relative components",
            ));
        }
        let meta = std::fs::symlink_metadata(&full)?;
        if !meta.file_type().is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a directory (or is a symlink)",
            ));
        }
        Ok(Self {
            path: std::fs::canonicalize(full)?,
        })
    }

    /// Open `dir` as a pinned directory capability without following its final component.
    #[cfg(target_os = "linux")]
    pub fn open_nofollow(dir: &Path) -> io::Result<Self> {
        use std::path::Component;

        let anchor = if dir.is_absolute() { c"/" } else { c"." };
        // SAFETY: `anchor` is a valid NUL-terminated string that lives across the call.
        let root_fd = unsafe {
            libc::open(
                anchor.as_ptr(),
                libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if root_fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `fd` is a freshly opened descriptor that no other owner holds; taking it here makes this value its sole owner.
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
            // SAFETY: `current` is an open directory descriptor owned by this loop, and the C string argument is NUL-terminated and alive for the call.
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
            // SAFETY: `fd` is a freshly opened descriptor that no other owner holds; taking it here makes this value its sole owner.
            current = unsafe { OwnedFd::from_raw_fd(fd) };
        }
        Ok(Self { fd: current })
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    pub fn open_nofollow(dir: &Path) -> io::Result<Self> {
        let path = to_cstring(dir)?;
        // Non-Linux Unix systems can have trusted system symlinks above a project root (macOS
        // `/var` -> `/private/var`). Open the requested root in one operation so ancestors resolve
        // normally, while O_NOFOLLOW still rejects a symlink at the final component. Subsequent
        // operations stay pinned to this descriptor and use component-wise openat with O_NOFOLLOW.
        // SAFETY: `path` is NUL-terminated and alive for the call; a successful descriptor is new
        // and becomes owned by the returned handle.
        let fd = unsafe {
            libc::open(
                path.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            // SAFETY: `fd` is a freshly opened descriptor with no other owner.
            fd: unsafe { OwnedFd::from_raw_fd(fd) },
        })
    }

    #[cfg(not(unix))]
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
    #[cfg(unix)]
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
                // SAFETY: `current` is an open directory descriptor owned by this loop, and the C string argument is NUL-terminated and alive for the call.
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
                // SAFETY: `fd` is a freshly opened descriptor that no other owner holds; taking it here makes this value its sole owner.
                let file = unsafe { std::fs::File::from_raw_fd(fd) };
                if !file.metadata()?.is_file() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "not a regular file",
                    ));
                }
                return Ok(file);
            }
            // SAFETY: `current` is an open directory descriptor owned by this loop, and the C string argument is NUL-terminated and alive for the call.
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
            // SAFETY: `fd` is a freshly opened descriptor that no other owner holds; taking it here makes this value its sole owner.
            current = unsafe { OwnedFd::from_raw_fd(fd) };
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path must name a file",
        ))
    }

    #[cfg(not(unix))]
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
        // Check every component below the canonical directory, not only the last one:
        // `symlink_metadata` on the full path follows an intermediate symlink, which would let a
        // link like `.plaesy/spec/topic -> /elsewhere` read files outside the directory.
        let mut current = self.path.clone();
        for part in path.components() {
            current.push(part.as_os_str());
            if std::fs::symlink_metadata(&current)?
                .file_type()
                .is_symlink()
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "symlink in path component",
                ));
            }
        }
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
    #[cfg(unix)]
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
        // SAFETY: `self.fd` is an open descriptor owned by this handle, and the C string argument is NUL-terminated and alive for the call.
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
        // SAFETY: `fd` is a freshly opened descriptor that no other owner holds; taking it here makes this value its sole owner.
        let file = unsafe { std::fs::File::from_raw_fd(fd) };
        if !file.metadata()?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a regular file",
            ));
        }
        Ok(file)
    }

    #[cfg(not(unix))]
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
    #[cfg(unix)]
    pub fn open_or_create_file(&self, name: &Path, mode: u32) -> io::Result<std::fs::File> {
        let c_name = to_component_cstring(name)?;
        // SAFETY: `self.fd` is an open descriptor owned by this handle, and the C string argument is NUL-terminated and alive for the call.
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
                mode as libc::mode_t as libc::c_uint,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `fd` is a freshly opened descriptor that no other owner holds; taking it here makes this value its sole owner.
        let file = unsafe { std::fs::File::from_raw_fd(fd) };
        if !file.metadata()?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a regular file",
            ));
        }
        Ok(file)
    }

    #[cfg(windows)]
    pub fn open_or_create_file(&self, name: &Path, _mode: u32) -> io::Result<std::fs::File> {
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
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
    #[cfg(unix)]
    pub fn create_new_file(&self, name: &Path) -> io::Result<std::fs::File> {
        let c_name = to_component_cstring(name)?;
        // SAFETY: `self.fd` is an open descriptor owned by this handle, and the C string argument is NUL-terminated and alive for the call.
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
        // SAFETY: `fd` is a freshly opened descriptor that no other owner holds; taking it here makes this value its sole owner.
        Ok(unsafe { std::fs::File::from_raw_fd(fd) })
    }

    #[cfg(not(unix))]
    pub fn create_new_file(&self, name: &Path) -> io::Result<std::fs::File> {
        // Read access too: model pull hashes the same handle after writing it.
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(self.child_path_fallback(name)?)
    }

    /// Hard-link `old_name` (inside this directory) to `new_name` (inside this directory),
    /// resolved relative to the held descriptor rather than by re-walking the path.
    #[cfg(unix)]
    pub fn link_within(&self, old_name: &Path, new_name: &Path) -> io::Result<()> {
        let c_old = to_component_cstring(old_name)?;
        let c_new = to_component_cstring(new_name)?;
        // SAFETY: `self.fd` is an open descriptor owned by this handle, and the C string argument is NUL-terminated and alive for the call.
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
    #[cfg(unix)]
    pub fn replace_within(&self, old_name: &Path, new_name: &Path) -> io::Result<()> {
        let c_old = to_component_cstring(old_name)?;
        let c_new = to_component_cstring(new_name)?;
        // SAFETY: `self.fd` is an open descriptor owned by this handle, and the C string argument is NUL-terminated and alive for the call.
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

    #[cfg(not(unix))]
    pub fn link_within(&self, old_name: &Path, new_name: &Path) -> io::Result<()> {
        std::fs::hard_link(
            self.child_path_fallback(old_name)?,
            self.child_path_fallback(new_name)?,
        )
    }

    /// Atomically replace `new_name` with `old_name` inside this directory.
    #[cfg(not(unix))]
    pub fn replace_within(&self, old_name: &Path, new_name: &Path) -> io::Result<()> {
        std::fs::rename(
            self.child_path_fallback(old_name)?,
            self.child_path_fallback(new_name)?,
        )
    }

    /// Remove an empty directory named `name` directly inside this directory.
    #[cfg(unix)]
    pub fn remove_dir(&self, name: &Path) -> io::Result<()> {
        let c_name = to_component_cstring(name)?;
        let rc =
            // SAFETY: `self.fd` is an open descriptor owned by this handle, and the C string argument is NUL-terminated and alive for the call.
            unsafe { libc::unlinkat(self.fd.as_raw_fd(), c_name.as_ptr(), libc::AT_REMOVEDIR) };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    #[cfg(not(unix))]
    pub fn remove_dir(&self, name: &Path) -> io::Result<()> {
        std::fs::remove_dir(self.child_component_fallback(name)?)
    }

    /// Remove a regular file named `name` directly inside this directory.
    #[cfg(unix)]
    pub fn remove_file(&self, name: &Path) -> io::Result<()> {
        let c_name = to_component_cstring(name)?;
        // SAFETY: `self.fd` is an open descriptor owned by this handle, and the C string argument is NUL-terminated and alive for the call.
        let rc = unsafe { libc::unlinkat(self.fd.as_raw_fd(), c_name.as_ptr(), 0) };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    #[cfg(not(unix))]
    pub fn remove_file(&self, name: &Path) -> io::Result<()> {
        std::fs::remove_file(self.child_path_fallback(name)?)
    }

    /// Flush this directory's own metadata/entries to durable storage.
    #[cfg(unix)]
    pub fn sync(&self) -> io::Result<()> {
        // SAFETY: `self.fd` is an open descriptor owned by this handle, and the C string argument is NUL-terminated and alive for the call.
        let rc = unsafe { libc::fsync(self.fd.as_raw_fd()) };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    #[cfg(not(unix))]
    pub fn sync(&self) -> io::Result<()> {
        // File contents are synced before publication. Rust has no portable directory fsync API.
        Ok(())
    }

    #[cfg(not(unix))]
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

    #[cfg(not(unix))]
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
#[path = "fs_safety_test.rs"]
mod fs_safety_test;
