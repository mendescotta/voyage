//! Port of `ui/pages/mirrors.py`: mirror choice and driver/nonfree toggles.

use adw::prelude::*;

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
    mirror_row: adw::ComboRow,
    nonfree_row: adw::SwitchRow,
    nvidia_row: adw::SwitchRow,
    intel_row: adw::SwitchRow,
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

        let mirror_group = adw::PreferencesGroup::builder().title("Package mirror").build();
        let model = gtk::StringList::new(&[]);
        for (label, _key) in MIRRORS {
            model.append(label);
        }
        let mirror_row = adw::ComboRow::builder().title("Mirror").model(&model).build();
        mirror_group.add(&mirror_row);
        widget.append(&mirror_group);

        let software_group = adw::PreferencesGroup::builder().title("Additional software").build();
        let nonfree_row = adw::SwitchRow::builder().title("Enable nonfree repository").build();
        let nvidia_row = adw::SwitchRow::builder().title("Install proprietary NVIDIA driver").build();
        let intel_row = adw::SwitchRow::builder().title("Install Intel graphics driver").build();
        software_group.add(&nonfree_row);
        software_group.add(&nvidia_row);
        software_group.add(&intel_row);
        widget.append(&software_group);

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
