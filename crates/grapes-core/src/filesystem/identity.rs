use std::fs::Metadata;
use std::path::Path;
#[cfg(not(any(unix, windows)))]
use std::time::UNIX_EPOCH;

pub fn file_identity(path: &Path) -> Option<String> {
    let identity = file_id::get_file_id(path).ok()?;
    Some(match identity {
        file_id::FileId::Inode {
            device_id,
            inode_number,
        } => format!("inode:{device_id}:{inode_number}"),
        file_id::FileId::LowRes {
            volume_serial_number,
            file_index,
        } => format!("ntfs-low:{volume_serial_number}:{file_index}"),
        file_id::FileId::HighRes {
            volume_serial_number,
            file_id,
        } => format!("ntfs-high:{volume_serial_number}:{file_id}"),
    })
}

#[cfg(unix)]
pub fn mtime_ns(metadata: &Metadata) -> i64 {
    use std::os::unix::fs::MetadataExt;
    metadata
        .mtime()
        .saturating_mul(1_000_000_000)
        .saturating_add(metadata.mtime_nsec())
}

#[cfg(windows)]
pub fn mtime_ns(metadata: &Metadata) -> i64 {
    use std::os::windows::fs::MetadataExt;
    const WINDOWS_TO_UNIX_EPOCH_100NS: u64 = 116_444_736_000_000_000;
    metadata
        .last_write_time()
        .saturating_sub(WINDOWS_TO_UNIX_EPOCH_100NS)
        .saturating_mul(100)
        .min(i64::MAX as u64) as i64
}

#[cfg(not(any(unix, windows)))]
pub fn mtime_ns(metadata: &Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| {
            duration
                .as_nanos()
                .min(i64::MAX as u128)
                .try_into()
                .unwrap_or(i64::MAX)
        })
        .unwrap_or_default()
}
