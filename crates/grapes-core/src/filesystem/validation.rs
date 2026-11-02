use crate::error::{GrapesError, Result};

const INVALID_WINDOWS_CHARS: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
const RESERVED_WINDOWS_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

// Workspace file operations intentionally use Windows-safe names on every
// platform so a workspace can be moved between Windows installations.
pub fn validate_name(name: &str) -> Result<()> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
        return Err(GrapesError::InvalidPath("名称不能为空".into()));
    }
    if name != trimmed {
        return Err(GrapesError::InvalidPath("名称首尾不能包含空格".into()));
    }
    if name.ends_with('.') {
        return Err(GrapesError::InvalidPath("名称不能以句点结尾".into()));
    }
    if name
        .chars()
        .any(|character| INVALID_WINDOWS_CHARS.contains(&character) || character.is_control())
    {
        return Err(GrapesError::InvalidPath(format!(
            "名称包含 Windows 不允许的字符：{name}"
        )));
    }

    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    if RESERVED_WINDOWS_NAMES.contains(&stem.as_str()) {
        return Err(GrapesError::InvalidPath(format!(
            "名称是 Windows 保留名称：{name}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_cross_platform_safe_names() {
        assert!(validate_name("Vancouver 2026").is_ok());
        assert!(validate_name("照片.jpg").is_ok());
        assert!(validate_name("").is_err());
        assert!(validate_name("bad/name").is_err());
        assert!(validate_name("CON.txt").is_err());
        assert!(validate_name("trailing.").is_err());
    }
}
