use std::fs::File;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::fs::OpenOptions;
use std::io;
#[cfg(target_os = "macos")]
use std::io::{Read, Write};
use std::path::Path;

#[cfg(target_os = "macos")]
const COPY_CHUNK_BYTES: usize = 2 * 1024 * 1024;

#[cfg(target_os = "macos")]
const MACOS_FS_LABELS: &[(&str, &str)] = &[
    ("apfs", "apfs"),
    ("smbfs", "smbfs"),
    ("nfs", "nfs"),
    ("hfs", "hfs"),
    ("exfat", "exfat"),
    ("msdos", "msdos"),
    ("afpfs", "afpfs"),
    ("webdav", "webdav"),
    ("autofs", "autofs"),
];

#[cfg(target_os = "linux")]
const LINUX_FS_LABELS: &[(u64, &str)] = &[
    (0xef53, "ext4"),
    (0x9123_683e, "btrfs"),
    (0x5846_5342, "xfs"),
    (0xfe53_4d42, "smb2"),
    (0xff53_4d42, "cifs"),
    (0x6969, "nfs"),
    (0x0102_1994, "tmpfs"),
    (0x6573_5546, "fuse"),
    (0x794c_7630, "overlay"),
    (0x2fc1_2fc1, "zfs"),
];

pub(crate) struct CopyStats {
    pub(crate) strategy: &'static str,
    pub(crate) fs: &'static str,
    pub(crate) bytes_written: u64,
}

impl CopyStats {
    pub(crate) fn log_fields(&self, elapsed_ms: u128) -> String {
        let mb_s = if elapsed_ms == 0 || self.bytes_written == 0 {
            0.0
        } else {
            (self.bytes_written as f64 / (1024.0 * 1024.0)) / (elapsed_ms as f64 / 1000.0)
        };
        format!(
            "strategy={} fs={} bytes_written={} mb_s={mb_s:.1}",
            self.strategy, self.fs, self.bytes_written
        )
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn filesystem_label_for_file(file: &File) -> &'static str {
    let Ok(stat) = rustix::fs::fstatfs(file) else {
        return "other";
    };
    let bytes: Vec<u8> = stat
        .f_fstypename
        .iter()
        .copied()
        .take_while(|&b| b != 0)
        .map(|b| b.to_ne_bytes()[0])
        .collect();
    let Ok(name) = std::str::from_utf8(&bytes) else {
        return "other";
    };
    MACOS_FS_LABELS
        .iter()
        .find_map(|&(key, label)| (key == name).then_some(label))
        .unwrap_or("other")
}

#[cfg(target_os = "linux")]
pub(crate) fn filesystem_label_for_file(file: &File) -> &'static str {
    let Ok(stat) = rustix::fs::fstatfs(file) else {
        return "other";
    };
    let magic = (stat.f_type as u64) & 0xFFFF_FFFF;
    LINUX_FS_LABELS
        .iter()
        .find_map(|&(key, label)| (key == magic).then_some(label))
        .unwrap_or("other")
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub(crate) fn filesystem_label_for_file(_file: &File) -> &'static str {
    "other"
}

pub(crate) fn try_cow_clone(src: &Path, dst: &Path) -> io::Result<bool> {
    #[cfg(target_os = "macos")]
    {
        let src_file = File::open(src)?;
        let parent = dst
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let parent_dir = File::open(parent)?;
        let dst_name = dst.file_name().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "destination has no file name")
        })?;
        match rustix::fs::fclonefileat(
            &src_file,
            &parent_dir,
            dst_name,
            rustix::fs::CloneFlags::empty(),
        ) {
            Ok(()) => Ok(true),
            Err(
                rustix::io::Errno::NOTSUP
                | rustix::io::Errno::OPNOTSUPP
                | rustix::io::Errno::XDEV
                | rustix::io::Errno::INVAL,
            ) => Ok(false),
            Err(errno) => Err(io::Error::from(errno)),
        }
    }
    #[cfg(target_os = "linux")]
    {
        let src_file = File::open(src)?;
        let dst_file = OpenOptions::new().write(true).create_new(true).open(dst)?;
        match rustix::fs::ioctl_ficlone(&dst_file, &src_file) {
            Ok(()) => Ok(true),
            Err(
                rustix::io::Errno::OPNOTSUPP
                | rustix::io::Errno::XDEV
                | rustix::io::Errno::INVAL
                | rustix::io::Errno::NOTTY,
            ) => {
                drop(dst_file);
                remove_temp_best_effort(dst);
                Ok(false)
            }
            Err(errno) => {
                drop(dst_file);
                remove_temp_best_effort(dst);
                Err(io::Error::from(errno))
            }
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (src, dst);
        Ok(false)
    }
}

pub(crate) fn copy_for_save(src: &Path, dst: &Path, fs: &str) -> io::Result<u64> {
    #[cfg(target_os = "macos")]
    {
        if fs == "smbfs" {
            return copy_data_only(src, dst);
        }
        match std::fs::copy(src, dst) {
            Ok(bytes) => Ok(bytes),
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                remove_temp_best_effort(dst);
                copy_data_only(src, dst)
            }
            Err(error) => Err(error),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = fs;
        std::fs::copy(src, dst)
    }
}

pub(crate) fn copy_data_only(src: &Path, dst: &Path) -> io::Result<u64> {
    #[cfg(target_os = "macos")]
    {
        let mut reader = File::open(src)?;
        let mut writer = OpenOptions::new().write(true).create_new(true).open(dst)?;
        match copy_buffered(&mut reader, &mut writer) {
            Ok(written) => Ok(written),
            Err(error) => {
                drop(writer);
                remove_temp_best_effort(dst);
                Err(error)
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::fs::copy(src, dst)
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn remove_temp_best_effort(path: &Path) {
    if let Err(error) = std::fs::remove_file(path) {
        if error.kind() != io::ErrorKind::NotFound {
            log::warn!(
                "file_copy cleanup failed temp={} error={error}",
                crate::diagnostics::artifact_id(path)
            );
        }
    }
}

#[cfg(target_os = "macos")]
fn copy_buffered(reader: &mut File, writer: &mut File) -> io::Result<u64> {
    let mut buf = vec![0u8; COPY_CHUNK_BYTES];
    let mut total = 0u64;
    loop {
        let read = match reader.read(&mut buf) {
            Ok(0) => return Ok(total),
            Ok(n) => n,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        writer.write_all(&buf[..read])?;
        total += read as u64;
    }
}

#[cfg(test)]
pub(crate) fn clone_or_copy_data(src: &Path, dst: &Path) -> io::Result<CopyStats> {
    let file = File::open(src)?;
    let fs = filesystem_label_for_file(&file);
    drop(file);
    if try_cow_clone(src, dst)? {
        return Ok(CopyStats {
            strategy: "cow_clone",
            fs,
            bytes_written: 0,
        });
    }
    let bytes_written = copy_data_only(src, dst)?;
    Ok(CopyStats {
        strategy: "data_copy",
        fs,
        bytes_written,
    })
}

#[cfg(test)]
#[path = "file_copy_tests.rs"]
mod tests;
