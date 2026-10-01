use std::env;
use std::path::{Path, PathBuf};

const BACKEND_DIR_ENV: &str = "VOYAGE_BACKEND_DIR";

pub fn backend_dir() -> PathBuf {
    if let Ok(dir) = env::var(BACKEND_DIR_ENV) {
        return PathBuf::from(dir);
    }

    let exe_dir = env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));

    let installed = exe_dir.join("../share/voyage/backend");
    if installed.is_dir() {
        return installed;
    }

    exe_dir.join("../../resources/backend")
}

pub fn backend_install_script() -> PathBuf {
    backend_dir().join("backend_install.sh")
}

pub fn auto_partition_script() -> PathBuf {
    backend_dir().join("auto_partition.sh")
}
