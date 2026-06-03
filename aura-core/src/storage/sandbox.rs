use crate::Result;
use std::path::{Path, PathBuf};

pub(crate) fn check_path_sandbox_impl(
    path: &Path,
    config: &Option<std::sync::Arc<arc_swap::ArcSwap<crate::Config>>>,
) -> Result<()> {
    let sandbox_root = if let Some(ref config_holder) = config {
        let active_config = config_holder.load();
        if let Some(ref root_str) = active_config.storage.sandbox_root {
            Some(PathBuf::from(root_str))
        } else {
            Some(PathBuf::from(&active_config.storage.download_dir))
        }
    } else {
        None
    };

    if let Some(root_path) = sandbox_root {
        // Canonicalize sandbox root. If it doesn't exist yet, use it as-is.
        let canonical_root =
            std::fs::canonicalize(&root_path).unwrap_or_else(|_| root_path.clone());

        // Find the longest existing parent directory of target path to resolve symlinks
        let mut ancestor = path;
        let mut components_to_append = Vec::new();
        while !ancestor.exists() {
            if let Some(parent) = ancestor.parent() {
                if let Some(name) = ancestor.file_name() {
                    components_to_append.push(name);
                }
                ancestor = parent;
            } else {
                break;
            }
        }

        let mut resolved =
            std::fs::canonicalize(ancestor).unwrap_or_else(|_| ancestor.to_path_buf());
        for name in components_to_append.into_iter().rev() {
            resolved.push(name);
        }

        // Normalize path to resolve any ".." or "." in the appended components
        let mut canonical_target = PathBuf::new();
        for component in resolved.components() {
            match component {
                std::path::Component::ParentDir => {
                    canonical_target.pop();
                }
                std::path::Component::Normal(c) => {
                    canonical_target.push(c);
                }
                std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                    canonical_target = PathBuf::from(component.as_os_str());
                }
                std::path::Component::CurDir => {}
            }
        }

        // Double check: if target is a symlink, check the link destination as well
        if let Ok(metadata) = std::fs::symlink_metadata(path) {
            if metadata.is_symlink() {
                if let Ok(real_path) = std::fs::read_link(path) {
                    let canonical_real = std::fs::canonicalize(&real_path).unwrap_or(real_path);
                    if !canonical_real.starts_with(&canonical_root) {
                        return Err(crate::Error::Storage(format!(
                            "Path traversal detected: symlink destination {:?} escapes sandbox {:?}",
                            canonical_real, canonical_root
                        )));
                    }
                }
            }
        }

        if !canonical_target.starts_with(&canonical_root) {
            return Err(crate::Error::Storage(format!(
                "Path traversal detected: path {:?} escapes sandbox {:?}",
                canonical_target, canonical_root
            )));
        }
    }

    // Check for special reserved device files on Windows
    if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
        let lower = file_name.to_lowercase();
        // Windows reserved names
        if lower == "con"
            || lower == "prn"
            || lower == "aux"
            || lower == "nul"
            || lower.starts_with("com")
            || lower.starts_with("lpt")
        {
            return Err(crate::Error::Storage(format!(
                "Reserved system device filename rejected: {}",
                file_name
            )));
        }
    }
    #[cfg(unix)]
    {
        if path.starts_with("/dev/") {
            return Err(crate::Error::Storage(format!(
                "Access to /dev/ directory is rejected: {:?}",
                path
            )));
        }
    }

    Ok(())
}
