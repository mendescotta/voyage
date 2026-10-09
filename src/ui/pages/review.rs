use gtk::prelude::*;

use crate::backend::config_schema::{InstallConfig, Partition};
use crate::ui::widgets::{ActionRow, Group};

pub const TITLE: &str = "Review";

pub fn review_rows(config: &InstallConfig) -> Vec<(String, String)> {
    vec![
        ("Language".to_string(), config.locale.clone()),
        ("Timezone".to_string(), config.timezone.clone()),
        ("Keyboard layout".to_string(), config.keymap.clone()),
        ("Computer name".to_string(), config.hostname.clone()),
        (
            "User account".to_string(),
            format!("{} ({})", config.username, config.userlogin),
        ),
        ("User shell".to_string(), shell_label(&config.user_shell)),
        ("Root shell".to_string(), shell_label(&config.root_shell)),
        (
            "User password".to_string(),
            password_state(&config.userpassword),
        ),
        (
            "Root password".to_string(),
            password_state(&config.rootpassword),
        ),
        (
            "Display manager".to_string(),
            display_manager_label(&config.display_manager),
        ),
        ("Auto login".to_string(), yes_no(config.autologin)),
        ("Mirror".to_string(), config.mirror.clone()),
        ("System updates".to_string(), yes_no(config.update)),
        ("Nonfree repository".to_string(), yes_no(config.nonfree)),
        (
            "VirtualBox guest additions".to_string(),
            yes_no(config.vbox_guest),
        ),
        (
            "Hardware drivers".to_string(),
            if config.hw_drivers {
                "Detected for this machine".to_string()
            } else {
                "Disabled".to_string()
            },
        ),
        (
            "Initramfs drivers".to_string(),
            match config.driver_set {
                crate::backend::config_schema::DriverSet::Generic => {
                    "Generic (any hardware)".to_string()
                }
                crate::backend::config_schema::DriverSet::Targeted => {
                    "Targeted (this machine only)".to_string()
                }
            },
        ),
        (
            "Disk layout".to_string(),
            partitions_summary(&config.partitions),
        ),
        ("Swap".to_string(), swap_summary(config)),
        (
            "Filesystem".to_string(),
            root_filesystem(&config.partitions),
        ),
        ("Btrfs layout".to_string(), btrfs_summary(config)),
        (
            "Bootloader".to_string(),
            format!("{} on {}", config.bootloader_type, config.bootloader_disk),
        ),
    ]
}

/// The setup page where a Review row's choice is made (None when it cannot be changed here).
pub fn edit_page(label: &str) -> Option<usize> {
    use crate::ui::nav::{DISKS, MIRRORS, USERS, WELCOME};
    match label {
        "Language" | "Timezone" | "Keyboard layout" => Some(WELCOME),
        "Computer name" | "User account" | "User shell" | "Root shell" | "User password"
        | "Root password" | "Auto login" => Some(USERS),
        "Mirror"
        | "System updates"
        | "Nonfree repository"
        | "VirtualBox guest additions"
        | "Hardware drivers"
        | "Initramfs drivers" => Some(MIRRORS),
        "Disk layout" | "Swap" | "Filesystem" | "Btrfs layout" | "Bootloader" => Some(DISKS),
        _ => None,
    }
}

fn shell_label(name: &str) -> String {
    if name.is_empty() {
        "Distro default".to_string()
    } else {
        name.to_string()
    }
}

fn display_manager_label(display_manager: &str) -> String {
    if display_manager.is_empty() {
        "None".to_string()
    } else {
        display_manager.to_string()
    }
}

fn password_state(pwd: &str) -> String {
    if pwd.is_empty() {
        "(none)".to_string()
    } else {
        "(set)".to_string()
    }
}

fn yes_no(b: bool) -> String {
    if b {
        "Enabled".to_string()
    } else {
        "Disabled".to_string()
    }
}

fn partitions_summary(parts: &[Partition]) -> String {
    parts
        .iter()
        .map(|p| {
            format!(
                "{} \u{2192} {} ({}{})",
                p.dev,
                p.point,
                p.fs,
                if p.format { ", format" } else { "" }
            )
        })
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
    parts
        .iter()
        .find(|p| p.point == "/")
        .map(|p| p.fs.clone())
        .unwrap_or_default()
}

fn btrfs_summary(config: &InstallConfig) -> String {
    if root_filesystem(&config.partitions) != "btrfs" {
        return "N/A".to_string();
    }
    let layout = if config.btrfs_flat {
        "Flat"
    } else {
        "Subvolumes (@, @home, @log, @pkg)"
    };
    if config.btrfs_snapshots {
        format!("{layout} + snapshots")
    } else {
        layout.to_string()
    }
}

type EditHandler = std::rc::Rc<std::cell::RefCell<Option<Box<dyn Fn(usize)>>>>;

pub struct ReviewPage {
    pub widget: gtk::Box,
    group: Group,
    rows: std::cell::RefCell<Vec<ActionRow>>,
    pub install_button: gtk::Button,
    edit_handler: EditHandler,
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

        let group = Group::new("Summary");
        widget.append(group.as_ref());

        let install_button = gtk::Button::builder()
            .label("Install Now")
            .halign(gtk::Align::End)
            .css_classes(["destructive-action"])
            .build();
        widget.append(&install_button);

        Self {
            widget,
            group,
            rows: std::cell::RefCell::new(Vec::new()),
            install_button,
            edit_handler: EditHandler::default(),
        }
    }

    /// Called with the page to open when the user presses Edit on a row.
    pub fn set_edit_handler(&self, handler: impl Fn(usize) + 'static) {
        *self.edit_handler.borrow_mut() = Some(Box::new(handler));
    }

    pub fn set_config(&self, config: &InstallConfig) {
        let mut rows = self.rows.borrow_mut();
        for row in rows.drain(..) {
            self.group.remove(&row);
        }
        for (label, value) in review_rows(config) {
            let row = ActionRow::new(&label);
            row.set_subtitle(&value);
            if let Some(page) = edit_page(&label) {
                let button = gtk::Button::builder()
                    .label("Edit")
                    .valign(gtk::Align::Center)
                    .css_classes(["flat"])
                    .tooltip_text(format!("Change \"{label}\""))
                    .build();
                let handler = self.edit_handler.clone();
                button.connect_clicked(move |_| {
                    if let Some(f) = handler.borrow().as_ref() {
                        f(page);
                    }
                });
                row.add_suffix(button.upcast_ref());
            }
            self.group.add(&row);
            rows.push(row);
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
            userlogin: "user".to_string(),
            username: "Gui".to_string(),
            userpassword: "hunter2".to_string(),
            rootpassword: "".to_string(),
            usergroups: "wheel".to_string(),
            display_manager: "gdm".to_string(),
            autologin: false,
            mirror: "Default".to_string(),
            update: true,
            nonfree: false,
            hw_drivers: false,
            driver_set: crate::backend::config_schema::DriverSet::Generic,
            user_shell: "zsh".to_string(),
            root_shell: "bash".to_string(),
            vbox_guest: false,
            partitions: vec![
                Partition {
                    dev: "/dev/sda1".to_string(),
                    point: "/boot/efi".to_string(),
                    fs: "vfat".to_string(),
                    format: true,
                },
                Partition {
                    dev: "/dev/sda2".to_string(),
                    point: "/".to_string(),
                    fs: "btrfs".to_string(),
                    format: true,
                },
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
    fn review_rows_includes_every_collected_choice() {
        let rows = review_rows(&sample_config());
        for key in [
            "Display manager",
            "Auto login",
            "Nonfree repository",
            "Hardware drivers",
            "Initramfs drivers",
        ] {
            assert!(rows.iter().any(|(k, _)| k == key), "missing row: {key}");
        }
    }

    #[test]
    fn review_rows_handles_non_btrfs_root_without_panicking() {
        let mut config = sample_config();
        config.partitions[1].fs = "ext4".to_string();
        let rows = review_rows(&config);
        let btrfs = rows.iter().find(|(k, _)| k == "Btrfs layout").unwrap();
        assert_eq!(btrfs.1, "N/A");
    }

    #[test]
    fn every_editable_row_points_at_a_setup_page() {
        use crate::ui::nav::{DISKS, MIRRORS, USERS, WELCOME};
        let rows = review_rows(&sample_config());
        let mut read_only = Vec::new();
        for (label, _) in &rows {
            match edit_page(label) {
                Some(page) => assert!(
                    [WELCOME, MIRRORS, USERS, DISKS].contains(&page),
                    "{label} -> {page}"
                ),
                None => read_only.push(label.as_str()),
            }
        }
        // the only choice the user cannot change here is the one detected from the live system
        assert_eq!(read_only, vec!["Display manager"]);
    }

    #[test]
    fn rows_belong_to_the_page_where_the_choice_is_made() {
        use crate::ui::nav::{DISKS, MIRRORS, USERS, WELCOME};
        assert_eq!(edit_page("Timezone"), Some(WELCOME));
        assert_eq!(edit_page("User password"), Some(USERS));
        assert_eq!(edit_page("Nonfree repository"), Some(MIRRORS));
        assert_eq!(edit_page("User shell"), Some(USERS));
        assert_eq!(edit_page("Root shell"), Some(USERS));
        assert_eq!(edit_page("VirtualBox guest additions"), Some(MIRRORS));
        assert_eq!(edit_page("Bootloader"), Some(DISKS));
        assert_eq!(edit_page("Something new"), None);
    }

    /// All buttons below `widget`, depth first.
    fn buttons(widget: &gtk::Widget, out: &mut Vec<gtk::Button>) {
        if let Some(b) = widget.downcast_ref::<gtk::Button>() {
            out.push(b.clone());
        }
        let mut child = widget.first_child();
        while let Some(c) = child {
            buttons(&c, out);
            child = c.next_sibling();
        }
    }

    #[test]
    fn edit_buttons_open_the_page_that_owns_the_choice() {
        crate::ui::gtk_test::run("the Review test", || {
            use crate::ui::nav::{DISKS, MIRRORS, USERS, WELCOME};
            use std::cell::RefCell;
            use std::rc::Rc;

            let page = ReviewPage::new();
            let opened = Rc::new(RefCell::new(Vec::new()));
            {
                let opened = opened.clone();
                page.set_edit_handler(move |p| opened.borrow_mut().push(p));
            }
            page.set_config(&sample_config());

            let mut all = Vec::new();
            buttons(page.widget.upcast_ref(), &mut all);
            let edit: Vec<_> = all
                .iter()
                .filter(|b| b.label().as_deref() == Some("Edit"))
                .collect();
            let editable = review_rows(&sample_config())
                .iter()
                .filter(|(l, _)| edit_page(l).is_some())
                .count();
            assert_eq!(edit.len(), editable, "one Edit button per editable row");

            for b in &edit {
                b.emit_clicked();
            }
            let opened = opened.borrow();
            for page in [WELCOME, USERS, MIRRORS, DISKS] {
                assert!(opened.contains(&page), "page {page} was never opened");
            }
            assert!(opened.iter().all(|p| *p <= DISKS));
        });
    }
}
