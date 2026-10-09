use gtk::prelude::*;

use gtk::glib;

use crate::backend::config_schema::DriverSet;
use crate::backend::hardware;
use crate::ui::widgets::{ComboRow, Group, SwitchRow};
use crate::ui::SysData;

pub const TITLE: &str = "Mirror and Software";

const MIRRORS: &[(&str, &str)] = &[
    ("Local ISO (I'll update later)", "Local"),
    ("Default", "Default"),
    ("Europe, Finland", "Finland"),
    ("Europe, Germany", "Germany"),
    ("Global, CDN", "Global"),
    ("North America, USA", "USA"),
];

pub struct MirrorsPage {
    pub widget: gtk::Box,
    mirror_row: ComboRow,
    nonfree_row: SwitchRow,
    hw_row: SwitchRow,
    vbox_row: SwitchRow,
    driver_set_row: ComboRow,
    net: bool,
}

impl MirrorsPage {
    pub fn new(sys_data: &SysData) -> Self {
        let widget = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(18)
            .margin_top(24)
            .margin_start(24)
            .margin_end(24)
            .build();

        let mirror_group = Group::new("Package mirror");
        let labels: Vec<String> = MIRRORS
            .iter()
            .map(|(label, _key)| label.to_string())
            .collect();
        let mirror_row = ComboRow::new("Mirror", &labels);
        mirror_group.add(&mirror_row);
        widget.append(mirror_group.as_ref());

        let software_group = Group::new("Additional software");
        let nonfree_row = SwitchRow::new("Enable nonfree repository");
        let hw_row = SwitchRow::new("Install drivers for this hardware");
        hw_row.set_subtitle("Detecting hardware...");
        hw_row.set_sensitive(false);
        let vbox_row = SwitchRow::new("Install the VirtualBox guest additions");
        vbox_row.set_active(sys_data.virtualbox);
        vbox_row.set_subtitle(&vbox_subtitle(sys_data.virtualbox, sys_data.net));
        software_group.add(&nonfree_row);
        software_group.add(&hw_row);
        software_group.add(&vbox_row);
        widget.append(software_group.as_ref());

        let boot_group = Group::new("Boot drivers");
        let driver_set_row = ComboRow::new(
            "Initramfs",
            &[
                "Generic: works on any hardware".to_string(),
                "Targeted: only this machine, smaller and faster".to_string(),
            ],
        );
        boot_group.add(&driver_set_row);
        widget.append(boot_group.as_ref());

        // voidhw reads sysfs only, but run it off the UI thread anyway.
        let (tx, rx) = async_channel::bounded::<Option<hardware::HardwarePlan>>(1);
        std::thread::spawn(move || {
            let _ = tx.send_blocking(hardware::detect());
        });
        {
            let hw_row = hw_row.clone();
            glib::spawn_future_local(async move {
                match rx.recv().await {
                    Ok(Some(plan)) => {
                        hw_row.set_subtitle(&hardware::summary(&plan));
                        hw_row.set_active(!plan.packages.is_empty());
                        hw_row.set_sensitive(!plan.packages.is_empty());
                    }
                    _ => hw_row.set_subtitle("Hardware detection (voidhw) is not available."),
                }
            });
        }

        Self {
            widget,
            mirror_row,
            nonfree_row,
            hw_row,
            vbox_row,
            driver_set_row,
            net: sys_data.net,
        }
    }

    pub fn collect(&self) -> (MirrorsFields, Vec<(String, String)>) {
        let key = MIRRORS[self.mirror_row.selected() as usize].1.to_string();
        let fields = MirrorsFields {
            mirror: key,
            net: self.net,
            nonfree: self.nonfree_row.is_active(),
            hw_drivers: self.hw_row.is_active(),
            vbox_guest: self.vbox_row.is_active(),
            driver_set: if self.driver_set_row.selected() == 1 {
                DriverSet::Targeted
            } else {
                DriverSet::Generic
            },
        };
        (fields, Vec::new())
    }
}

/// What the guest-additions switch says about itself.
fn vbox_subtitle(detected: bool, net: bool) -> String {
    let what = if detected {
        "VirtualBox detected. Clipboard, shared folders and display resizing."
    } else {
        "For a VirtualBox guest: clipboard, shared folders and display resizing."
    };
    if net {
        what.to_string()
    } else {
        format!("{what} Needs the internet: offline they are skipped.")
    }
}

#[derive(Debug, Clone)]
pub struct MirrorsFields {
    pub mirror: String,
    pub net: bool,
    pub nonfree: bool,
    pub hw_drivers: bool,
    pub vbox_guest: bool,
    pub driver_set: DriverSet,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_guest_additions_subtitle_tells_detection_and_network_apart() {
        assert!(vbox_subtitle(true, true).starts_with("VirtualBox detected"));
        assert!(vbox_subtitle(false, true).starts_with("For a VirtualBox guest"));
        assert!(!vbox_subtitle(true, true).contains("internet"));
        assert!(vbox_subtitle(true, false).contains("Needs the internet"));
    }
}
