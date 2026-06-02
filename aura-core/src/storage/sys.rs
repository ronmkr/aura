use std::fs::File;
use std::io::Result;
use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
pub(crate) fn harden_file(file: &File, length: u64) -> Result<()> {
    use std::os::unix::io::AsRawFd;
    let fd = file.as_raw_fd();

    unsafe {
        let mut stat: libc::statfs = std::mem::zeroed();
        if libc::fstatfs(fd, &mut stat) == 0 {
            let f_type = stat.f_type as i64;
            // BTRFS_SUPER_MAGIC = 0x9123683E
            // ZFS_SUPER_MAGIC = 0x2FC12FC1
            if f_type == 0x9123683E || f_type == 0x2FC12FC1 {
                // Disable COW (FS_NOCOW_FL)
                let mut flags: libc::c_long = 0;
                const FS_IOC_GETFLAGS: libc::c_ulong = 0x80086601;
                const FS_IOC_SETFLAGS: libc::c_ulong = 0x40086602;
                const FS_NOCOW_FL: libc::c_long = 0x00800000;

                if libc::ioctl(fd, FS_IOC_GETFLAGS, &mut flags) == 0 && (flags & FS_NOCOW_FL) == 0 {
                    flags |= FS_NOCOW_FL;
                    let _ = libc::ioctl(fd, FS_IOC_SETFLAGS, &flags);
                }
                // Skip fallocate on COW filesystems as per ADR 0035
                return Ok(());
            }

            // Check for network shares to skip fallocate (ADR 0021)
            // NFS = 0x6969, SMB = 0x517B, CIFS = 0xFF534D42
            if f_type == 0x6969 || f_type == 0x517B || f_type == 0xFF534D42 {
                return Ok(());
            }
        }

        // Standard ext4/xfs: Fallocate for actual block allocation
        let _ = libc::fallocate(fd, 0, 0, length as libc::off_t);
    }

    Ok(())
}

#[cfg(target_os = "linux")]
pub(crate) fn is_network_share_fd(fd: std::os::unix::io::RawFd) -> bool {
    unsafe {
        let mut stat: libc::statfs = std::mem::zeroed();
        if libc::fstatfs(fd, &mut stat) == 0 {
            let f_type = stat.f_type as i64;
            if f_type == 0x6969 || f_type == 0x517B || f_type == 0xFF534D42 {
                return true;
            }
        }
    }
    false
}

#[cfg(target_os = "macos")]
pub(crate) fn is_network_share_fd(fd: std::os::unix::io::RawFd) -> bool {
    unsafe {
        let mut stat: libc::statfs = std::mem::zeroed();
        if libc::fstatfs(fd, &mut stat) == 0 {
            let fstype = &stat.f_fstypename;
            let mut len = 0;
            while len < fstype.len() && fstype[len] != 0 {
                len += 1;
            }
            if let Ok(name) = std::str::from_utf8(std::slice::from_raw_parts(
                fstype.as_ptr() as *const u8,
                len,
            )) {
                if name == "nfs" || name == "smbfs" || name == "afpfs" {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub(crate) fn is_network_share_fd(_fd: std::os::unix::io::RawFd) -> bool {
    false
}

pub(crate) fn is_network_share(file: &tokio::fs::File) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        is_network_share_fd(file.as_raw_fd())
    }
    #[cfg(not(unix))]
    {
        let _ = file;
        false
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn harden_file(file: &File, length: u64) -> Result<()> {
    use std::os::unix::io::AsRawFd;
    let fd = file.as_raw_fd();

    if is_network_share_fd(fd) {
        return Ok(());
    }

    unsafe {
        let mut store = libc::fstore_t {
            fst_flags: libc::F_ALLOCATECONTIG,
            fst_posmode: libc::F_PEOFPOSMODE,
            fst_offset: 0,
            fst_length: length as libc::off_t,
            fst_bytesalloc: 0,
        };

        // Try allocating contiguous blocks
        let mut ret = libc::fcntl(fd, libc::F_PREALLOCATE, &store);
        if ret == -1 {
            // Fallback to non-contiguous blocks
            store.fst_flags = libc::F_ALLOCATEALL;
            ret = libc::fcntl(fd, libc::F_PREALLOCATE, &store);
        }

        if ret != -1 {
            // macOS requires ftruncate after F_PREALLOCATE
            let _ = libc::ftruncate(fd, length as libc::off_t);
        }
    }

    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub(crate) fn harden_file(_file: &File, _length: u64) -> Result<()> {
    Ok(())
}

pub(crate) fn harden_path(path: &Path) -> PathBuf {
    // Truncate path components that are too long (e.g., > 255 chars)
    let mut safe_path = PathBuf::new();
    for component in path.components() {
        let s = component.as_os_str().to_string_lossy();
        if s.len() > 255 {
            let p = Path::new(&*s);
            let ext = p.extension().unwrap_or_default().to_string_lossy();
            let safe_len = 255 - ext.len() - 1; // -1 for the dot
            let name = p.file_stem().unwrap_or_default().to_string_lossy();

            let truncated_name = name.chars().take(safe_len).collect::<String>();
            if ext.is_empty() {
                safe_path.push(truncated_name);
            } else {
                safe_path.push(format!("{}.{}", truncated_name, ext));
            }
        } else {
            safe_path.push(component);
        }
    }

    #[cfg(windows)]
    {
        let s = safe_path.to_string_lossy();
        if !s.starts_with("\\\\?\\") && safe_path.is_absolute() {
            let mut prefix = std::ffi::OsString::from("\\\\?\\");
            prefix.push(safe_path.as_os_str());
            return PathBuf::from(prefix);
        }
    }
    safe_path
}

pub(crate) fn apply_fadvise_dontneed(file: &tokio::fs::File, offset: u64, len: u64) {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::io::AsRawFd;
        let fd = file.as_raw_fd();
        unsafe {
            libc::posix_fadvise(
                fd,
                offset as libc::off_t,
                len as libc::off_t,
                libc::POSIX_FADV_DONTNEED,
            );
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (file, offset, len);
    }
}

pub(crate) fn apply_fadvise_sequential(file: &tokio::fs::File) {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::io::AsRawFd;
        let fd = file.as_raw_fd();
        unsafe {
            libc::posix_fadvise(fd, 0, 0, libc::POSIX_FADV_SEQUENTIAL);
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = file;
    }
}
