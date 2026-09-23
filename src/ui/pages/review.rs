//! Final review/summary page shown before the install actually starts —
//! the second-stage destructive confirmation (the disks page's
//! auto-partition warning dialog is the first stage for that path).

use adw::prelude::*;

use crate::backend::config_schema::{InstallConfig, Partition};

pub const TITLE: &str = "Review";

pub fn review_rows(config: &InstallConfig) -> Vec<(String, String)> {
    vec![
        ("Language".to_string(), config.locale.clone()),
        ("Timezone".to_string(), config.timezone.clone()),
        ("Keyboard layout".to_string(), config.keymap.clone()),
        ("Computer name".to_string(), config.hostname.clone()),
        ("User account".to_string(), format!("{} ({})", config.username, config.userlogin)),
        ("User password".to_string(), password_state(&config.userpassword)),
        ("Root password".to_string(), password_state(&config.rootpassword)),
        ("Mirror".to_string(), config.mirror.clone()),
        ("System updates".to_string(), yes_no(config.update)),
        ("Disk layout".to_string(), partitions_summary(&config.partitions)),
        ("Swap".to_string(), swap_summary(config)),
        ("Filesystem".to_string(), root_filesystem(&config.partitions)),
        ("Btrfs layout".to_string(), btrfs_summary(config)),
        ("Bootloader".to_string(), format!("{} on {}", config.bootloader_type, config.bootloader_disk)),
    ]
}

fn password_state(pwd: &str) -> String {
    if pwd.is_empty() { "(none)".to_string() } else { "(set)".to_string() }
}

fn yes_no(b: bool) -> String {
    if b { "Enabled".to_string() } else { "Disabled".to_string() }
}

fn partitions_summary(parts: &[Partition]) -> String {
    parts
        .iter()
        .map(|p| format!("{} \u{2192} {} ({}{})", p.dev, p.point, p.fs, if p.format { ", format" } else { "" }))
        .collect::<Vec<_>>()
        .join("\n")
}

fn swap_summary(config: &InstallConfig) -> String {
    match config.swap_strategy.as_str() {
        "partition" => "Partition".to_string(),
        "swapfile" => "Swap file".to_string(),
        _ => "None".to_string(),
    }
}

fn root_filesystem(parts: &[Partition]) -> String {
    parts.iter().find(|p| p.point == "/").map(|p| p.fs.clone()).unwrap_or_default()
}

fn btrfs_summary(config: &InstallConfig) -> String {
    if root_filesystem(&config.partitions) != "btrfs" {
        return "N/A".to_string();
    }
    let layout = if config.btrfs_flat { "Flat" } else { "Subvolumes (@, @home, @log, @pkg)" };
    if config.btrfs_snapshots {
        format!("{layout} + snapshots")
    } else {
        layout.to_string()
    }
}

pub struct ReviewPage {
    pub widget: gtk::Box,
    group: adw::PreferencesGroup,
    pub install_button: gtk::Button,
}

impl ReviewPage {
    pub fn new() -> Self {
        let widget = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(18)
            .margin_top(24)
            .margin_start(24)
            .margin_end(24)
            .build();

        let warning = gtk::Label::builder()
            .label("Review your choices below. Installing will erase the selected partitions and cannot be undone.")
            .css_classes(["dim-label"])
            .wrap(true)
            .halign(gtk::Align::Start)
            .build();
        widget.append(&warning);

        let group = adw::PreferencesGroup::builder().title("Summary").build();
        widget.append(&group);

        let install_button = gtk::Button::builder()
            .label("Install Now")
            .halign(gtk::Align::End)
            .css_classes(["destructive-action"])
            .build();
        widget.append(&install_button);

        Self { widget, group, install_button }
    }

    pub fn set_config(&self, config: &InstallConfig) {
        while let Some(child) = self.group.first_child() {
            self.group.remove(&child);
        }
        for (label, value) in review_rows(config) {
            let row = adw::ActionRow::builder().title(label).subtitle(value).build();
            self.group.add(&row);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> InstallConfig {
        InstallConfig {
            locale: "en_US.UTF-8".to_string(),
            timezone: "America/New_York".to_string(),
            keymap: "us".to_string(),
            hostname: "void-box".to_string(),
            userlogin: "gui".to_string(),
            username: "Gui".to_string(),
            userpassword: "hunter2".to_string(),
            rootpassword: "".to_string(),
            usergroups: "wheel".to_string(),
            display_manager: "gdm".to_string(),
            autologin: false,
            mirror: "Default".to_string(),
            update: true,
            nonfree: false,
            nvidia: false,
            intel: false,
            partitions: vec![
                Partition { dev: "/dev/sda1".to_string(), point: "/boot/efi".to_string(), fs: "vfat".to_string(), format: true },
                Partition { dev: "/dev/sda2".to_string(), point: "/".to_string(), fs: "btrfs".to_string(), format: true },
            ],
            bootloader_disk: "/dev/sda".to_string(),
            bootloader_type: "grub".to_string(),
            swap_strategy: "swapfile".to_string(),
            btrfs_flat: false,
            btrfs_snapshots: true,
        }
    }

    #[test]
    fn review_rows_never_shows_password_values() {
        let rows = review_rows(&sample_config());
        for (_, value) in &rows {
            assert!(!value.contains("hunter2"));
        }
    }

    #[test]
    fn review_rows_shows_set_and_none_for_passwords() {
        let rows = review_rows(&sample_config());
        let user_pw = rows.iter().find(|(k, _)| k == "User password").unwrap();
        let root_pw = rows.iter().find(|(k, _)| k == "Root password").unwrap();
        assert_eq!(user_pw.1, "(set)");
        assert_eq!(root_pw.1, "(none)");
    }

    #[test]
    fn review_rows_summarizes_btrfs_layout() {
        let rows = review_rows(&sample_config());
        let btrfs = rows.iter().find(|(k, _)| k == "Btrfs layout").unwrap();
        assert!(btrfs.1.contains("snapshots"));
    }

    #[test]
    fn review_rows_handles_non_btrfs_root_without_panicking() {
        let mut config = sample_config();
        config.partitions[1].fs = "ext4".to_string();
        let rows = review_rows(&config);
        let btrfs = rows.iter().find(|(k, _)| k == "Btrfs layout").unwrap();
        assert_eq!(btrfs.1, "N/A");
    }
}
