use gtk::prelude::*;

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
    nvidia_row: SwitchRow,
    intel_row: SwitchRow,
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
        let labels: Vec<String> = MIRRORS.iter().map(|(label, _key)| label.to_string()).collect();
        let mirror_row = ComboRow::new("Mirror", &labels);
        mirror_group.add(&mirror_row);
        widget.append(mirror_group.as_ref());

        let software_group = Group::new("Additional software");
        let nonfree_row = SwitchRow::new("Enable nonfree repository");
        let nvidia_row = SwitchRow::new("Install proprietary NVIDIA driver");
        let intel_row = SwitchRow::new("Install Intel graphics driver");
        software_group.add(&nonfree_row);
        software_group.add(&nvidia_row);
        software_group.add(&intel_row);
        widget.append(software_group.as_ref());

        Self { widget, mirror_row, nonfree_row, nvidia_row, intel_row, net: sys_data.net }
    }

    pub fn collect(&self) -> (MirrorsFields, Vec<(String, String)>) {
        let key = MIRRORS[self.mirror_row.selected() as usize].1.to_string();
        let fields = MirrorsFields {
            mirror: key,
            net: self.net,
            nonfree: self.nonfree_row.is_active(),
            nvidia: self.nvidia_row.is_active(),
            intel: self.intel_row.is_active(),
        };
        (fields, Vec::new())
    }
}

#[derive(Debug, Clone, Default)]
pub struct MirrorsFields {
    pub mirror: String,
    pub net: bool,
    pub nonfree: bool,
    pub nvidia: bool,
    pub intel: bool,
}
