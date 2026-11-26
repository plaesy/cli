//! Free disk space on the volume that holds a path. Offline: a single OS call, no network.

use std::path::Path;

/// Bytes available to unprivileged users on the volume that holds `path`.
///
/// # Errors
///
/// Returns the OS error when the query fails (for example, the path does not exist on Unix).
/// Platforms other than Unix and Windows return [`std::io::ErrorKind::Unsupported`].
pub fn available_bytes(path: &Path) -> std::io::Result<u64> {
    available_bytes_impl(path)
}

#[cfg(unix)]
#[allow(clippy::useless_conversion)] // statvfs field widths differ by target (u32 on some, u64 on others).
fn available_bytes_impl(path: &Path) -> std::io::Result<u64> {
    use std::os::unix::ffi::OsStrExt;

    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    // SAFETY: `statvfs` is plain old data, so the all-zero bit pattern is a valid value.
    let mut stats: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c_path` is a valid NUL-terminated string that outlives the call, and `stats` is a
    // valid out-pointer for the duration of the call.
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut stats) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let blocks = u64::from(stats.f_bavail);
    let block_size = u64::try_from(stats.f_frsize).unwrap_or(u64::MAX);
    Ok(blocks.saturating_mul(block_size))
}

#[cfg(windows)]
fn available_bytes_impl(path: &Path) -> std::io::Result<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut available: u64 = 0;
    // SAFETY: `wide` is NUL-terminated and alive for the call. `available` is a valid out-pointer;
    // the two total-size out-pointers are optional and may be null.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(available)
    }
}

#[cfg(not(any(unix, windows)))]
fn available_bytes_impl(_path: &Path) -> std::io::Result<u64> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "free disk space is not supported on this platform",
    ))
}
