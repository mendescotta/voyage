use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;
use std::thread;

use gtk::glib;
use gtk::prelude::*;

use crate::backend::config_schema::{DiskChoices, RawPartitions, SwapStrategy};
use crate::backend::paths::auto_partition_script;
use crate::backend::system_detect::{self, check_disk_health, Disk, HealthStatus, PartitionDetail};
use crate::ui::widgets::{self, ActionRow, ComboRow, Group, SwitchRow};
use crate::ui::SysData;

pub const TITLE: &str = "Disks";

const FILESYSTEMS: &[&str] = &["ext4", "btrfs", "ext3", "ext2", "xfs"];
const BIOS_BOOTLOADERS: &[(&str, &str)] = &[("GRUB", "grub")];
const EFI_BOOTLOADERS: &[(&str, &str)] = &[("GRUB", "grub"), ("Limine", "limine"), ("rEFInd", "refind")];

struct State {
    disks: Vec<Disk>,
    partitions: Vec<PartitionDetail>,
}

pub struct DisksPage {
    pub widget: gtk::Box,
    disk_row: ComboRow,
    root_row: ComboRow,
    efi_row: ComboRow,
    swap_strategy_row: ComboRow,
    swap_row: ComboRow,
    home_row: ComboRow,
    filesys_row: ComboRow,
    btrfs_flat_row: SwitchRow,
    btrfs_snapshots_row: SwitchRow,
    bootloader_row: ComboRow,
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

        let auto_group = Group::new("Automatic partitioning");
        let disks = system_detect::detect_disks();
        let disk_labels: Vec<String> = if disks.is_empty() {
            vec!["No disk found".to_string()]
        } else {
            disks.iter().map(|d| format!("{} ({})", d.model, d.size)).collect()
        };
        let disk_row = ComboRow::new("Target disk", &disk_labels);
        let layout_row = ComboRow::new("Layout", &["Basic".to_string(), "Basic + swap".to_string()]);
        let shred_row = SwitchRow::new("Securely erase disk first");
        shred_row.set_subtitle("Overwrites the disk with zeros before partitioning. Adds time proportional to disk size.");
        let auto_button = gtk::Button::builder().label("Partition automatically").css_classes(["destructive-action"]).build();
        let auto_row = ActionRow::new("Erase disk and create partitions");
        auto_row.add_suffix(auto_button.upcast_ref());
        let health_label = gtk::Label::builder().css_classes(["dim-label", "caption"]).halign(gtk::Align::Start).build();
        auto_group.add(&disk_row);
        auto_group.add(&ActionRow::with_child(health_label.upcast_ref()));
        auto_group.add(&layout_row);
        auto_group.add(&shred_row);
        auto_group.add(&auto_row);
        widget.append(auto_group.as_ref());

        let manual_group = Group::new("Manual assignment");
        let root_row = ComboRow::new("Root (/)", &[]);
        let efi_row = ComboRow::new("EFI (/boot/efi)", &[]);
        let swap_strategy_row = ComboRow::new("Swap", &["None".to_string(), "Partition".to_string(), "Swap file".to_string()]);
        let swap_row = ComboRow::new("Swap partition", &[]);
        swap_row.set_visible(false);
        let home_row = ComboRow::new("Home (/home)", &[]);
        manual_group.add(&root_row);
        if is_efi {
            manual_group.add(&efi_row);
        }
        manual_group.add(&swap_strategy_row);
        manual_group.add(&swap_row);
        manual_group.add(&home_row);

        {
            let swap_row = swap_row.clone();
            swap_strategy_row.connect_selected(move |row| {
                swap_row.set_visible(row.selected() == 1);
            });
        }

        let gparted_button = gtk::Button::builder().label("Open GParted").build();
        let gparted_row = ActionRow::new("Need finer control?");
        gparted_row.add_suffix(gparted_button.upcast_ref());
        manual_group.add(&gparted_row);
        widget.append(manual_group.as_ref());

        let fs_group = Group::new("Filesystem");
        let filesys_row =
            ComboRow::new("Format new partitions as", &FILESYSTEMS.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        fs_group.add(&filesys_row);

        let btrfs_flat_row = SwitchRow::new("Flat layout");
        btrfs_flat_row.set_subtitle("Single subvolume instead of @/@home/@log/@pkg. Disables snapshots.");
        btrfs_flat_row.set_visible(false);
        let btrfs_snapshots_row = SwitchRow::new("Enable @snapshots subvolume");
        btrfs_snapshots_row.set_subtitle("Mounted at /.snapshots.");
        btrfs_snapshots_row.set_visible(false);
        fs_group.add(&btrfs_flat_row);
        fs_group.add(&btrfs_snapshots_row);
        widget.append(fs_group.as_ref());

        {
            let btrfs_flat_row = btrfs_flat_row.clone();
            let btrfs_snapshots_row = btrfs_snapshots_row.clone();
            filesys_row.connect_selected(move |row| {
                let is_btrfs = FILESYSTEMS[row.selected() as usize] == "btrfs";
                btrfs_flat_row.set_visible(is_btrfs);
                btrfs_snapshots_row.set_visible(is_btrfs);
            });
        }
        {
            let btrfs_snapshots_row = btrfs_snapshots_row.clone();
            btrfs_flat_row.connect_active(move |row| {
                if row.is_active() {
                    btrfs_snapshots_row.set_active(false);
                }
                btrfs_snapshots_row.set_sensitive(!row.is_active());
            });
        }

        let bootloaders: &'static [(&'static str, &'static str)] = if is_efi { EFI_BOOTLOADERS } else { BIOS_BOOTLOADERS };
        let bootloader_group = Group::new("Bootloader");
        let bootloader_row =
            ComboRow::new("Install", &bootloaders.iter().map(|(label, _)| label.to_string()).collect::<Vec<_>>());
        bootloader_group.add(&bootloader_row);
        widget.append(bootloader_group.as_ref());

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
            disk_row_for_connect.connect_selected(move |row| update_health(row.selected() as usize));
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
                let state = state.clone();
                let (root_row_w, efi_row_w, swap_row_w, home_row_w) =
                    (root_row_w.clone(), efi_row_w.clone(), swap_row_w.clone(), home_row_w.clone());
                let swap_strategy_row = swap_strategy_row_outer.clone();
                let auto_button = button.clone();
                let error_anchor = button.clone();
                widgets::confirm(button, "Warning: automatic partitioning", &body, "Continue", move || {
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
                                    row.set_items(&options);
                                }
                                if let Some(swap_name) = swap_partition {
                                    swap_strategy_row.set_selected(1);
                                    let state_ref = state.borrow();
                                    select_partition(&swap_row_w, &state_ref.partitions, &swap_name);
                                }
                            }
                            _ => widgets::alert(&error_anchor, "Error", &format!("Failed to partition {disk}.")),
                        }
                    });
                });
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
                    widgets::alert(button, "Error", "gparted is not installed.");
                    return;
                }
                let mut state_mut = state.borrow_mut();
                state_mut.partitions = system_detect::get_partitions_detailed();
                let options = partition_options(&state_mut.partitions);
                drop(state_mut);
                for row in [&root_row, &efi_row, &swap_row, &home_row] {
                    row.set_items(&options);
                }
            });
        }

        page
    }

    fn reload_partitions(&self) {
        let mut state = self.state.borrow_mut();
        state.partitions = system_detect::get_partitions_detailed();
        let options = partition_options(&state.partitions);
        for row in [&self.root_row, &self.efi_row, &self.swap_row, &self.home_row] {
            row.set_items(&options);
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

    fn partition_device(&self, row: &ComboRow) -> Option<String> {
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

fn select_partition(row: &ComboRow, partitions: &[PartitionDetail], name: &str) {
    if let Some(index) = partitions.iter().position(|p| p.name == name) {
        row.set_selected((index + 1) as u32);
    }
}
