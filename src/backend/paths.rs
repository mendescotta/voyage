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

/// The first of these that exists.
fn first_existing(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| p.is_file()).cloned()
}

/// The app icon as a file, for when the icon theme does not have it (a dev build run from the source tree).
pub fn icon_file() -> Option<PathBuf> {
    let exe_dir = env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    first_existing(&[
        exe_dir.join("../share/icons/hicolor/scalable/apps/org.voidlinux.voyage.svg"),
        exe_dir.join("../../resources/images/voyage.svg"),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_existing_candidate_wins() {
        let dir = std::env::temp_dir().join(format!("voyage-paths-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b, missing) = (dir.join("a.svg"), dir.join("b.svg"), dir.join("nope.svg"));
        std::fs::write(&a, "a").unwrap();
        std::fs::write(&b, "b").unwrap();
        assert_eq!(
            first_existing(&[missing.clone(), b.clone(), a.clone()]),
            Some(b)
        );
        assert_eq!(first_existing(&[missing]), None);
        assert_eq!(first_existing(&[]), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_icon_ships_in_the_source_tree() {
        let icon =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/images/voyage.svg");
        let text = std::fs::read_to_string(icon).unwrap();
        assert!(
            text.contains("<svg") && text.contains("#5fb488"),
            "the Caerus-style backdrop"
        );
    }
}
