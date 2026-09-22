//! Resolves the vendored backend script/resource directory at runtime.
//!
//! Locates the backend directory relative to the running executable so the
//! app works both from a `cargo run` dev build and from an installed
//! location, with an environment variable escape hatch for packaging.

use std::env;
use std::path::{Path, PathBuf};

const BACKEND_DIR_ENV: &str = "VOYAGE_BACKEND_DIR";

/// Directory containing `backend_install.sh`, `auto_partition.sh`, and the
/// refind theme. Resolution order:
/// 1. `VOYAGE_BACKEND_DIR` environment variable, if set.
/// 2. `<exe_dir>/../share/voyage/backend` (an installed layout).
/// 3. `<exe_dir>/../../resources/backend` (a `cargo run`/`target/debug`
///    dev build, where `resources/` sits next to `Cargo.toml`).
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
