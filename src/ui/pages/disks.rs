use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;
use std::thread;

use adw::prelude::*;
use gtk::glib;

use crate::backend::config_schema::{DiskChoices, RawPartitions, SwapStrategy};
use crate::backend::paths::auto_partition_script;
use crate::backend::system_detect::{self, check_disk_health, Disk, HealthStatus, PartitionDetail};
use crate::ui::SysData;

pub const TITLE: &str = "Disks";

const FILESYSTEMS: &[&str] = &["ext4", "btrfs", "ext3", "ext2", "xfs"];
const BIOS_BOOTLOADERS: &[(&str, &str)] = &[("GRUB", "grub")];
const EFI_BOOTLOADERS: &[(&str, &str)] = &[("GRUB", "grub"), ("Limine", "limine"), ("rEFInd", "refind")];

fn string_list(items: &[String]) -> gtk::StringList {
    let model = gtk::StringList::new(&[]);
    for item in items {
        model.append(item);
    }
    model
}

struct State {
    disks: Vec<Disk>,
    partitions: Vec<PartitionDetail>,
}

pub struct DisksPage {
    pub widget: gtk::Box,
    disk_row: adw::ComboRow,
    root_row: adw::ComboRow,
    efi_row: adw::ComboRow,
    swap_strategy_row: adw::ComboRow,
    swap_row: adw::ComboRow,
    home_row: adw::ComboRow,
    filesys_row: adw::ComboRow,
    btrfs_flat_row: adw::SwitchRow,
    btrfs_snapshots_row: adw::SwitchRow,
    bootloader_row: adw::ComboRow,
    is_efi: bool,
    bootloaders: &'static [(&'static str, &'static str)],
    state: Rc<RefCell<State>>,
}

impl DisksPage {
    pub fn new(sys_data: &SysData) -> Self {
        let widget = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(18)
            .margin_top(24)
            .margin_start(24)
            .margin_end(24)
            .build();
        let is_efi = sys_data.efi;

        let auto_group = adw::PreferencesGroup::builder().title("Automatic partitioning").build();
        let disks = system_detect::detect_disks();
        let disk_labels: Vec<String> = if disks.is_empty() {
            vec!["No disk found".to_string()]
        } else {
            disks.iter().map(|d| format!("{} ({})", d.model, d.size)).collect()
        };
        let disk_row = adw::ComboRow::builder().title("Target disk").model(&string_list(&disk_labels)).build();
        let layout_row = adw::ComboRow::builder()
            .title("Layout")
            .model(&string_list(&["Basic".to_string(), "Basic + swap".to_string()]))
            .build();
        let shred_row = adw::SwitchRow::builder()
            .title("Securely erase disk first")
            .subtitle("Overwrites the disk with zeros before partitioning. Adds time proportional to disk size.")
            .build();
        let auto_button = gtk::Button::builder().label("Partition automatically").css_classes(["destructive-action"]).build();
        let auto_row = adw::ActionRow::builder().title("Erase disk and create partitions").build();
        auto_row.add_suffix(&auto_button);
        let health_label = gtk::Label::builder().css_classes(["dim-label", "caption"]).halign(gtk::Align::Start).build();
        auto_group.add(&disk_row);
        auto_group.add(&adw::ActionRow::builder().child(&health_label).build());
        auto_group.add(&layout_row);
        auto_group.add(&shred_row);
        auto_group.add(&auto_row);
        widget.append(&auto_group);

        let manual_group = adw::PreferencesGroup::builder().title("Manual assignment").build();
        let root_row = adw::ComboRow::builder().title("Root (/)").build();
        let efi_row = adw::ComboRow::builder().title("EFI (/boot/efi)").build();
        let swap_strategy_row = adw::ComboRow::builder()
            .title("Swap")
            .model(&string_list(&["None".to_string(), "Partition".to_string(), "Swap file".to_string()]))
            .build();
        let swap_row = adw::ComboRow::builder().title("Swap partition").visible(false).build();
        let home_row = adw::ComboRow::builder().title("Home (/home)").build();
        manual_group.add(&root_row);
        if is_efi {
            manual_group.add(&efi_row);
        }
        manual_group.add(&swap_strategy_row);
        manual_group.add(&swap_row);
        manual_group.add(&home_row);

        {
            let swap_row = swap_row.clone();
            swap_strategy_row.connect_selected_notify(move |row| {
                swap_row.set_visible(row.selected() == 1);
            });
        }

        let gparted_button = gtk::Button::builder().label("Open GParted").build();
        let gparted_row = adw::ActionRow::builder().title("Need finer control?").build();
        gparted_row.add_suffix(&gparted_button);
        manual_group.add(&gparted_row);
        widget.append(&manual_group);

        let fs_group = adw::PreferencesGroup::builder().title("Filesystem").build();
        let filesys_row = adw::ComboRow::builder()
            .title("Format new partitions as")
            .model(&string_list(&FILESYSTEMS.iter().map(|s| s.to_string()).collect::<Vec<_>>()))
            .build();
        fs_group.add(&filesys_row);

        let btrfs_flat_row = adw::SwitchRow::builder()
            .title("Flat layout")
            .subtitle("Single subvolume instead of @/@home/@log/@pkg. Disables snapshots.")
            .visible(false)
            .build();
        let btrfs_snapshots_row = adw::SwitchRow::builder()
            .title("Enable @snapshots subvolume")
            .subtitle("Mounted at /.snapshots.")
            .visible(false)
            .build();
        fs_group.add(&btrfs_flat_row);
        fs_group.add(&btrfs_snapshots_row);
        widget.append(&fs_group);

        {
            let btrfs_flat_row = btrfs_flat_row.clone();
            let btrfs_snapshots_row = btrfs_snapshots_row.clone();
            filesys_row.connect_selected_notify(move |row| {
                let is_btrfs = FILESYSTEMS[row.selected() as usize] == "btrfs";
                btrfs_flat_row.set_visible(is_btrfs);
                btrfs_snapshots_row.set_visible(is_btrfs);
            });
        }
        {
            let btrfs_snapshots_row = btrfs_snapshots_row.clone();
            btrfs_flat_row.connect_active_notify(move |row| {
                if row.is_active() {
                    btrfs_snapshots_row.set_active(false);
                }
                btrfs_snapshots_row.set_sensitive(!row.is_active());
            });
        }

        let bootloaders: &'static [(&'static str, &'static str)] = if is_efi { EFI_BOOTLOADERS } else { BIOS_BOOTLOADERS };
        let bootloader_group = adw::PreferencesGroup::builder().title("Bootloader").build();
        let bootloader_row = adw::ComboRow::builder()
            .title("Install")
            .model(&string_list(&bootloaders.iter().map(|(label, _)| label.to_string()).collect::<Vec<_>>()))
            .build();
        bootloader_group.add(&bootloader_row);
        widget.append(&bootloader_group);

        let state = Rc::new(RefCell::new(State { disks, partitions: Vec::new() }));

        let page = Self {
            widget,
            disk_row,
            root_row,
            efi_row,
            swap_strategy_row,
            swap_row,
            home_row,
            filesys_row,
            btrfs_flat_row,
            btrfs_snapshots_row,
            bootloader_row,
            is_efi,
            bootloaders,
            state,
        };
        page.reload_partitions();

        {
            let disk_row = page.disk_row.clone();
            let disks = page.state.borrow().disks.clone();
            let health_label = health_label.clone();
            let disk_row_for_connect = disk_row.clone();
            let update_health = move |index: usize| {
                let Some(disk) = disks.get(index) else { return };
                let disk_name = disk.name.clone();
                health_label.set_text("Checking disk health\u{2026}");

                let (tx, rx) = async_channel::bounded::<(String, system_detect::DiskHealth)>(1);
                let health_disk_name = disk_name.clone();
                thread::spawn(move || {
                    let health = check_disk_health(&health_disk_name);
                    let _ = tx.send_blocking((health_disk_name, health));
                });

                let health_label = health_label.clone();
                let disk_row = disk_row.clone();
                let disks = disks.clone();
                glib::spawn_future_local(async move {
                    let Ok((checked_name, health)) = rx.recv().await else { return };
                    let current = disks.get(disk_row.selected() as usize).map(|d| d.name.as_str());
                    if current != Some(checked_name.as_str()) {
                        return;
                    }
                    let fmt = |status: HealthStatus, note: &str| match status {
                        HealthStatus::Ok => format!("OK ({note})"),
                        HealthStatus::Warn => format!("Warning ({note})"),
                        HealthStatus::Unknown => format!("Unknown ({note})"),
                    };
                    health_label.set_text(&format!(
                        "Structure: {} \u{b7} Hardware: {}",
                        fmt(health.structure, &health.structure_note),
                        fmt(health.hardware, &health.hardware_note),
                    ));
                });
            };
            update_health(disk_row_for_connect.selected() as usize);
            disk_row_for_connect.connect_selected_notify(move |row| update_health(row.selected() as usize));
        }

        {
            let disk_row = page.disk_row.clone();
            let layout_row = layout_row.clone();
            let shred_row = shred_row.clone();
            let swap_strategy_row_outer = page.swap_strategy_row.clone();
            let state = page.state.clone();
            let (root_row_w, efi_row_w, swap_row_w, home_row_w) =
                (page.root_row.clone(), page.efi_row.clone(), page.swap_row.clone(), page.home_row.clone());
            auto_button.connect_clicked(move |button| {
                let state_ref = state.borrow();
                if state_ref.disks.is_empty() {
                    return;
                }
                let disk = state_ref.disks[disk_row.selected() as usize].name.clone();
                let layout = if layout_row.selected() == 1 { "with-swap" } else { "basic" };
                let shred_flag = if shred_row.is_active() { "--shred" } else { "" };
                drop(state_ref);

                let body = if shred_row.is_active() {
                    format!(
                        "Disk {disk} will be securely erased and then formatted; all its data will be lost. \
                         This action cannot be undone and may take a while depending on disk size."
                    )
                } else {
                    format!(
                        "Disk {disk} will be formatted and all its data will be lost. \
                         This action cannot be undone."
                    )
                };
                let dialog = adw::AlertDialog::builder()
                    .heading("Warning: automatic partitioning")
                    .body(body)
                    .build();
                dialog.add_response("cancel", "Cancel");
                dialog.add_response("continue", "Continue");
                dialog.set_response_appearance("continue", adw::ResponseAppearance::Destructive);

                let state = state.clone();
                let root = button.root();
                let (root_row_w, efi_row_w, swap_row_w, home_row_w) =
                    (root_row_w.clone(), efi_row_w.clone(), swap_row_w.clone(), home_row_w.clone());
                let swap_strategy_row = swap_strategy_row_outer.clone();
                let error_root = root.clone();
                let auto_button = button.clone();
                dialog.connect_response(None, move |_dialog, response| {
                    if response != "continue" {
                        return;
                    }

                    const AUTO_BUTTON_LABEL: &str = "Partition automatically";
                    auto_button.set_sensitive(false);
                    auto_button.set_label("Partitioning\u{2026}");

                    let (tx, rx) = async_channel::bounded::<std::io::Result<std::process::Output>>(1);
                    let disk_for_thread = disk.clone();
                    let layout = layout.to_string();
                    let shred_flag = shred_flag.to_string();
                    thread::spawn(move || {
                        let result = Command::new("pkexec")
                            .arg("bash")
                            .arg(auto_partition_script())
                            .arg(&disk_for_thread)
                            .arg(&layout)
                            .arg(&shred_flag)
                            .output();
                        let _ = tx.send_blocking(result);
                    });

                    let state = state.clone();
                    let disk = disk.clone();
                    let (root_row_w, efi_row_w, swap_row_w, home_row_w) =
                        (root_row_w.clone(), efi_row_w.clone(), swap_row_w.clone(), home_row_w.clone());
                    let swap_strategy_row = swap_strategy_row.clone();
                    let error_root = error_root.clone();
                    let auto_button = auto_button.clone();
                    glib::spawn_future_local(async move {
                        let result = rx.recv().await;
                        auto_button.set_sensitive(true);
                        auto_button.set_label(AUTO_BUTTON_LABEL);

                        match result {
                            Ok(Ok(output)) if output.status.success() => {
                                let swap_partition = String::from_utf8_lossy(&output.stdout)
                                    .lines()
                                    .find_map(|line| line.strip_prefix("SWAP_PARTITION=").map(str::to_string));

                                let mut state_mut = state.borrow_mut();
                                state_mut.partitions = system_detect::get_partitions_detailed();
                                let options = partition_options(&state_mut.partitions);
                                drop(state_mut);
                                for row in [&root_row_w, &efi_row_w, &swap_row_w, &home_row_w] {
                                    row.set_model(Some(&string_list(&options)));
                                }
                                if let Some(swap_name) = swap_partition {
                                    swap_strategy_row.set_selected(1);
                                    let state_ref = state.borrow();
                                    select_partition(&swap_row_w, &state_ref.partitions, &swap_name);
                                }
                            }
                            _ => {
                                let error = adw::AlertDialog::builder()
                                    .heading("Error")
                                    .body(format!("Failed to partition {disk}."))
                                    .build();
                                error.add_response("ok", "OK");
                                if let Some(root) = &error_root {
                                    error.present(Some(root));
                                }
                            }
                        }
                    });
                });
                if let Some(root) = root {
                    dialog.present(Some(&root));
                }
            });
        }

        {
            let state = page.state.clone();
            let root_row = page.root_row.clone();
            let efi_row = page.efi_row.clone();
            let swap_row = page.swap_row.clone();
            let home_row = page.home_row.clone();
            gparted_button.connect_clicked(move |button| {
                if Command::new("gparted").status().is_err() {
                    let error = adw::AlertDialog::builder().heading("Error").body("gparted is not installed.").build();
                    error.add_response("ok", "OK");
                    if let Some(root) = button.root() {
                        error.present(Some(&root));
                    }
                    return;
                }
                let mut state_mut = state.borrow_mut();
                state_mut.partitions = system_detect::get_partitions_detailed();
                let options = partition_options(&state_mut.partitions);
                drop(state_mut);
                for row in [&root_row, &efi_row, &swap_row, &home_row] {
                    row.set_model(Some(&string_list(&options)));
                }
            });
        }

        page
    }

    fn reload_partitions(&self) {
        let mut state = self.state.borrow_mut();
        state.partitions = system_detect::get_partitions_detailed();
        let options = partition_options(&state.partitions);
        let model = string_list(&options);
        for row in [&self.root_row, &self.efi_row, &self.swap_row, &self.home_row] {
            row.set_model(Some(&model));
        }
        drop(state);
        self.auto_select();
    }

    fn auto_select(&self) {
        let state = self.state.borrow();
        let efi_parts: Vec<&PartitionDetail> =
            state.partitions.iter().filter(|p| p.fstype.contains("vfat") || p.fstype.contains("fat")).collect();
        let swap_parts: Vec<&PartitionDetail> = state.partitions.iter().filter(|p| p.fstype.contains("swap")).collect();
        let claimed: std::collections::HashSet<&str> =
            efi_parts.iter().chain(swap_parts.iter()).map(|p| p.name.as_str()).collect();
        let mut other_sorted: Vec<&PartitionDetail> =
            state.partitions.iter().filter(|p| !claimed.contains(p.name.as_str())).collect();
        other_sorted.sort_by_key(|p| std::cmp::Reverse(p.size_bytes));

        if self.is_efi {
            if let Some(smallest) = efi_parts.iter().min_by_key(|p| p.size_bytes) {
                select_partition(&self.efi_row, &state.partitions, &smallest.name);
            }
        }
        if let Some(swap) = swap_parts.first() {
            self.swap_strategy_row.set_selected(1);
            select_partition(&self.swap_row, &state.partitions, &swap.name);
        }
        if let Some(root) = other_sorted.first() {
            select_partition(&self.root_row, &state.partitions, &root.name);
        }
        if let Some(home) = other_sorted.get(1) {
            select_partition(&self.home_row, &state.partitions, &home.name);
        }
    }

    fn partition_device(&self, row: &adw::ComboRow) -> Option<String> {
        let index = row.selected() as usize;
        if index == 0 {
            return None;
        }
        self.state.borrow().partitions.get(index - 1).map(|p| p.name.clone())
    }

    pub fn collect(&self) -> (DiskChoices, Vec<(String, String)>) {
        let raw_parts = RawPartitions {
            root: self.partition_device(&self.root_row),
            efi: if self.is_efi { self.partition_device(&self.efi_row) } else { None },
            swap: self.partition_device(&self.swap_row),
            home: self.partition_device(&self.home_row),
        };
        let filesystem = FILESYSTEMS[self.filesys_row.selected() as usize].to_string();
        let bootloader_type = self.bootloaders[self.bootloader_row.selected() as usize].1.to_string();
        (
            DiskChoices {
                raw_parts,
                filesystem,
                want_efi: self.is_efi,
                bootloader_type,
                swap_strategy: match self.swap_strategy_row.selected() {
                    1 => SwapStrategy::Partition,
                    2 => SwapStrategy::Swapfile,
                    _ => SwapStrategy::None,
                },
                btrfs_flat: self.btrfs_flat_row.is_active(),
                btrfs_snapshots: self.btrfs_snapshots_row.is_active(),
            },
            Vec::new(),
        )
    }
}

fn partition_options(partitions: &[PartitionDetail]) -> Vec<String> {
    let mut options = vec!["(none)".to_string()];
    options.extend(partitions.iter().map(|p| p.display.clone()));
    options
}

fn select_partition(row: &adw::ComboRow, partitions: &[PartitionDetail], name: &str) {
    if let Some(index) = partitions.iter().position(|p| p.name == name) {
        row.set_selected((index + 1) as u32);
    }
}
