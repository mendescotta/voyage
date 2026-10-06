use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::thread;
use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;

use crate::backend::{locales, system_detect};
use crate::ui::widgets::{Banner, ComboRow, Group};
use crate::ui::SysData;

pub const TITLE: &str = "Welcome";

struct Inner {
    locale_codes: Vec<String>,
    regions: Vec<String>,
    zones: BTreeMap<String, Vec<String>>,
    keymap_codes: Vec<String>,
}

pub struct WelcomePage {
    pub widget: gtk::Box,
    locale_row: ComboRow,
    region_row: ComboRow,
    city_row: ComboRow,
    keymap_row: ComboRow,
    inner: Rc<RefCell<Inner>>,
}

impl WelcomePage {
    pub fn new(sys_data: &SysData) -> Self {
        let widget = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(18)
            .margin_top(24)
            .margin_start(24)
            .margin_end(24)
            .build();

        let heading = gtk::Label::builder()
            .label("Welcome! Let's set up your regional settings")
            .css_classes(["title-1"])
            .build();
        widget.append(&heading);

        let mut locales_list = system_detect::detect_locales();
        if locales_list.is_empty() {
            locales_list.push("en_US.UTF-8".to_string());
        }
        let locale_group = Group::new("Locale");
        let language_names: Vec<String> = locales_list
            .iter()
            .map(|loc| {
                let lang_code = loc.split('.').next().unwrap_or(loc).split('_').next().unwrap_or(loc);
                let name = locales::language_name(lang_code);
                if name.is_empty() { loc.clone() } else { name }
            })
            .collect();
        let locale_row = ComboRow::new("System language", &language_names);
        locale_group.add(&locale_row);
        widget.append(locale_group.as_ref());
        let default_locale_idx = locales_list.iter().position(|l| l.starts_with("en_US")).unwrap_or(0);
        locale_row.set_selected(default_locale_idx as u32);

        let mut zones = system_detect::detect_timezones();
        if zones.is_empty() {
            zones.insert("UTC".to_string(), vec!["UTC".to_string()]);
        }
        let regions: Vec<String> = zones.keys().cloned().collect();
        let tz_group = Group::new("Timezone");
        let region_row = ComboRow::new("Region", &regions);
        let first_region_cities = zones.get(&regions[0]).cloned().unwrap_or_default();
        let city_row = ComboRow::new("City", &first_region_cities);
        tz_group.add(&region_row);
        tz_group.add(&city_row);
        widget.append(tz_group.as_ref());

        let mut keymaps = system_detect::detect_keymaps();
        if keymaps.is_empty() {
            keymaps.push("us".to_string());
        }
        let keymap_names: Vec<String> = keymaps.iter().map(|k| locales::keymap_name(k)).collect();
        let keymap_group = Group::new("Keyboard");
        let keymap_row = ComboRow::new("Layout", &keymap_names);
        keymap_group.add(&keymap_row);
        widget.append(keymap_group.as_ref());
        let default_keymap_idx = keymaps.iter().position(|k| k == "us").unwrap_or(0);
        keymap_row.set_selected(default_keymap_idx as u32);

        let has_net = sys_data.net;
        let status = if has_net {
            "Internet connection detected. Installer in online mode."
        } else {
            "No internet connection detected. Installer in offline mode."
        };
        let banner = Banner::new(status);
        widget.append(banner.as_ref());

        let inner = Rc::new(RefCell::new(Inner {
            locale_codes: locales_list,
            regions,
            zones,
            keymap_codes: keymaps,
        }));

        {
            let inner = inner.clone();
            let city_row = city_row.clone();
            region_row.connect_selected(move |row| {
                let inner = inner.borrow();
                let selected = row.selected() as usize;
                let Some(region) = inner.regions.get(selected) else { return };
                let cities = inner.zones.get(region).cloned().unwrap_or_default();
                city_row.set_items(&cities);
            });
        }

        if has_net {
            let (tx, rx) = async_channel::bounded::<Option<String>>(1);
            thread::spawn(move || {
                let tz = system_detect::detect_timezone_from_network(Duration::from_secs(3));
                let _ = tx.send_blocking(tz);
            });
            let inner = inner.clone();
            let region_row = region_row.clone();
            let city_row = city_row.clone();
            glib::spawn_future_local(async move {
                if let Ok(Some(tz)) = rx.recv().await {
                    apply_network_timezone(&inner, &region_row, &city_row, &tz);
                }
            });
        }

        Self { widget, locale_row, region_row, city_row, keymap_row, inner }
    }

    pub fn collect(&self) -> (WelcomeFields, Vec<(String, String)>) {
        let inner = self.inner.borrow();
        let region = inner.regions[self.region_row.selected() as usize].clone();
        let city = inner.zones[&region][self.city_row.selected() as usize].clone();
        let fields = WelcomeFields {
            locale: inner.locale_codes[self.locale_row.selected() as usize].clone(),
            timezone_region: region,
            timezone_city: city,
            keymap: inner.keymap_codes[self.keymap_row.selected() as usize].clone(),
        };
        (fields, Vec::new())
    }
}

fn apply_network_timezone(inner: &Rc<RefCell<Inner>>, region_row: &ComboRow, city_row: &ComboRow, tz: &str) {
    let Some((region, city)) = tz.split_once('/') else { return };
    let inner_ref = inner.borrow();
    let Some(region_idx) = inner_ref.regions.iter().position(|r| r == region) else { return };
    let Some(cities) = inner_ref.zones.get(region) else { return };
    let Some(city_idx) = cities.iter().position(|c| c == city) else { return };
    let cities = cities.clone();
    drop(inner_ref);

    region_row.set_selected(region_idx as u32);
    city_row.set_items(&cities);
    city_row.set_selected(city_idx as u32);
}

#[derive(Debug, Clone, Default)]
pub struct WelcomeFields {
    pub locale: String,
    pub timezone_region: String,
    pub timezone_city: String,
    pub keymap: String,
}
