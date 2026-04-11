use std::fs::File;
use anyhow::Result;

/// Returns true if `file` lives on a remote/network filesystem.
/// Falls back to false (treat as local) if detection fails or platform is unsupported.
pub fn is_remote(file: &File) -> Result<bool> {
    detect(file)
}

// ─── Linux ───────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn detect(file: &File) -> Result<bool> {
    use std::os::unix::io::AsRawFd;
    use libc::statfs;

    const NFS_SUPER_MAGIC:  libc::c_long = 0x6969;
    const CIFS_MAGIC:       libc::c_long = 0xFF534D42_u32 as libc::c_long;
    const SMB2_MAGIC:       libc::c_long = 0xFE534D42_u32 as libc::c_long;
    const FUSE_SUPER_MAGIC: libc::c_long = 0x65735546;
    const AFS_SUPER_MAGIC:  libc::c_long = 0x5346414F;

    let mut buf: statfs = unsafe { std::mem::zeroed() };
    let ret = unsafe { libc::fstatfs(file.as_raw_fd(), &mut buf) };
    if ret != 0 {
        // Non-fatal — fall back to local strategy
        return Ok(false);
    }

    Ok(matches!(
        buf.f_type,
        NFS_SUPER_MAGIC | CIFS_MAGIC | SMB2_MAGIC | FUSE_SUPER_MAGIC | AFS_SUPER_MAGIC
    ))
}

// ─── macOS ───────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn detect(file: &File) -> Result<bool> {
    use std::os::unix::io::AsRawFd;

    let mut buf: libc::statfs = unsafe { std::mem::zeroed() };
    let ret = unsafe { libc::fstatfs(file.as_raw_fd(), &mut buf) };
    if ret != 0 {
        return Ok(false);
    }

    let fstype = unsafe { std::ffi::CStr::from_ptr(buf.f_fstypename.as_ptr()) }
        .to_string_lossy()
        .to_lowercase();

    Ok(matches!(fstype.as_str(), "nfs" | "smbfs" | "afpfs" | "webdav" | "cifs"))
}

// ─── Windows ─────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn detect(file: &File) -> Result<bool> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandleEx, FileRemoteProtocolInfo,
        FILE_REMOTE_PROTOCOL_INFO,
    };

    // GetFileInformationByHandleEx with FileRemoteProtocolInfo succeeds only
    // when the file is accessed via a remote protocol (SMB, NFS, WebDAV, etc.).
    // On a local filesystem it returns zero — no struct fields need to be read.
    let mut info: FILE_REMOTE_PROTOCOL_INFO = unsafe { std::mem::zeroed() };
    let ok = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle() as _,
            FileRemoteProtocolInfo,
            &mut info as *mut _ as *mut _,
            std::mem::size_of::<FILE_REMOTE_PROTOCOL_INFO>() as u32,
        )
    };

    Ok(ok != 0)
}

// ─── Fallback ────────────────────────────────────────────────────────────────

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn detect(_file: &File) -> Result<bool> {
    Ok(false)
}
