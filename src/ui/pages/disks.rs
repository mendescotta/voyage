//! Port of `ui/pages/disks.py`: disk/partition selection, auto-partition,
//! GParted launch, filesystem and bootloader choice.

use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;

use adw::prelude::*;

use crate::backend::config_schema::{DiskChoices, RawPartitions, SwapStrategy};
use crate::backend::paths::auto_partition_script;
use crate::backend::system_detect::{self, Disk, PartitionDetail};
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
    swap_row: adw::ComboRow,
    home_row: adw::ComboRow,
    filesys_row: adw::ComboRow,
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
        let auto_button = gtk::Button::builder().label("Partition automatically").css_classes(["destructive-action"]).build();
        let auto_row = adw::ActionRow::builder().title("Erase disk and create partitions").build();
        auto_row.add_suffix(&auto_button);
        auto_group.add(&disk_row);
        auto_group.add(&auto_row);
        widget.append(&auto_group);

        let manual_group = adw::PreferencesGroup::builder().title("Manual assignment").build();
        let root_row = adw::ComboRow::builder().title("Root (/)").build();
        let efi_row = adw::ComboRow::builder().title("EFI (/boot/efi)").build();
        let swap_row = adw::ComboRow::builder().title("Swap").build();
        let home_row = adw::ComboRow::builder().title("Home (/home)").build();
        manual_group.add(&root_row);
        if is_efi {
            manual_group.add(&efi_row);
        }
        manual_group.add(&swap_row);
        manual_group.add(&home_row);

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
        widget.append(&fs_group);

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
            swap_row,
            home_row,
            filesys_row,
            bootloader_row,
            is_efi,
            bootloaders,
            state,
        };
        page.reload_partitions();

        {
            let disk_row = page.disk_row.clone();
            let state = page.state.clone();
            let (root_row_w, efi_row_w, swap_row_w, home_row_w) =
                (page.root_row.clone(), page.efi_row.clone(), page.swap_row.clone(), page.home_row.clone());
            auto_button.connect_clicked(move |button| {
                let state_ref = state.borrow();
                if state_ref.disks.is_empty() {
                    return;
                }
                let disk = state_ref.disks[disk_row.selected() as usize].name.clone();
                drop(state_ref);

                let dialog = adw::AlertDialog::builder()
                    .heading("Warning: automatic partitioning")
                    .body(format!(
                        "Disk {disk} will be formatted and all its data will be lost. \
                         This action cannot be undone."
                    ))
                    .build();
                dialog.add_response("cancel", "Cancel");
                dialog.add_response("continue", "Continue");
                dialog.set_response_appearance("continue", adw::ResponseAppearance::Destructive);

                let state = state.clone();
                let root = button.root();
                let (root_row_w, efi_row_w, swap_row_w, home_row_w) =
                    (root_row_w.clone(), efi_row_w.clone(), swap_row_w.clone(), home_row_w.clone());
                let error_root = root.clone();
                dialog.connect_response(None, move |_dialog, response| {
                    if response != "continue" {
                        return;
                    }
                    match Command::new("pkexec").arg("bash").arg(auto_partition_script()).arg(&disk).status() {
                        Ok(status) if status.success() => {
                            let mut state_mut = state.borrow_mut();
                            state_mut.partitions = system_detect::get_partitions_detailed();
                            let options = partition_options(&state_mut.partitions);
                            drop(state_mut);
                            for row in [&root_row_w, &efi_row_w, &swap_row_w, &home_row_w] {
                                row.set_model(Some(&string_list(&options)));
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

    /// Conservative auto-selection, matching `_auto_select` in disks.py:
    /// EFI -> smallest vfat, swap -> fstype swap, root -> largest
    /// remaining, home -> second largest.
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
                swap_strategy: SwapStrategy::None,
                btrfs_flat: false,
                btrfs_snapshots: false,
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
