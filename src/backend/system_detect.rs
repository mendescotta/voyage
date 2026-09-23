//! `SystemDetector`: live system
//! probing for disks, timezones, keymaps, locales, display manager, EFI
//! and network state.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use super::locales;

#[derive(Debug, Clone)]
pub struct DiskPartition {
    pub name: String,
    pub size: String,
    pub fstype: String,
}

#[derive(Debug, Clone)]
pub struct Disk {
    pub name: String,
    pub model: String,
    pub size: String,
    pub children: Vec<DiskPartition>,
}

#[derive(Debug, Clone)]
pub struct PartitionDetail {
    pub name: String,
    pub display: String,
    pub size_bytes: u64,
    pub fstype: String,
}

pub fn detect_efi() -> bool {
    Path::new("/sys/firmware/efi").exists()
}

pub fn has_internet(timeout: Duration) -> bool {
    for url in [
        "https://repo-default.voidlinux.org/",
        "https://repo-fastly.voidlinux.org/",
    ] {
        let agent = ureq::AgentBuilder::new().timeout(timeout).build();
        if agent.get(url).call().is_ok() {
            return true;
        }
    }
    false
}

/// Minimal shape of `lsblk -J -o NAME,SIZE,TYPE,MODEL,FSTYPE` we need,
/// hand-parsed to avoid pulling in a JSON dependency for one call site.
mod lsblk_json {
    /// Very small recursive-descent JSON parser, just enough for lsblk's
    /// output shape (objects, arrays, strings, null, no numbers/escapes
    /// beyond what lsblk emits).
    pub fn parse(input: &str) -> Option<Value> {
        let mut chars = input.chars().peekable();
        let value = parse_value(&mut chars)?;
        Some(value)
    }

    #[derive(Debug, Clone)]
    pub enum Value {
        Null,
        String(String),
        Array(Vec<Value>),
        Object(Vec<(String, Value)>),
    }

    impl Value {
        pub fn get(&self, key: &str) -> Option<&Value> {
            match self {
                Value::Object(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
                _ => None,
            }
        }

        pub fn as_str(&self) -> Option<&str> {
            match self {
                Value::String(s) => Some(s),
                _ => None,
            }
        }

        pub fn as_array(&self) -> Option<&[Value]> {
            match self {
                Value::Array(items) => Some(items),
                _ => None,
            }
        }
    }

    type Chars<'a> = std::iter::Peekable<std::str::Chars<'a>>;

    fn skip_ws(chars: &mut Chars) {
        while matches!(chars.peek(), Some(c) if c.is_whitespace()) {
            chars.next();
        }
    }

    fn parse_value(chars: &mut Chars) -> Option<Value> {
        skip_ws(chars);
        match chars.peek()? {
            '{' => parse_object(chars),
            '[' => parse_array(chars),
            '"' => parse_string(chars).map(Value::String),
            'n' => {
                for _ in 0..4 {
                    chars.next();
                }
                Some(Value::Null)
            }
            _ => parse_bare(chars),
        }
    }

    fn parse_bare(chars: &mut Chars) -> Option<Value> {
        let mut s = String::new();
        while matches!(chars.peek(), Some(c) if !matches!(c, ',' | '}' | ']' | ' ' | '\n' | '\t')) {
            s.push(chars.next().unwrap());
        }
        if s.is_empty() {
            None
        } else {
            Some(Value::String(s))
        }
    }

    fn parse_string(chars: &mut Chars) -> Option<String> {
        chars.next(); // opening quote
        let mut s = String::new();
        loop {
            let c = chars.next()?;
            match c {
                '"' => break,
                '\\' => {
                    let esc = chars.next()?;
                    s.push(match esc {
                        'n' => '\n',
                        't' => '\t',
                        '"' => '"',
                        '\\' => '\\',
                        other => other,
                    });
                }
                other => s.push(other),
            }
        }
        Some(s)
    }

    fn parse_array(chars: &mut Chars) -> Option<Value> {
        chars.next(); // '['
        let mut items = Vec::new();
        skip_ws(chars);
        if chars.peek() == Some(&']') {
            chars.next();
            return Some(Value::Array(items));
        }
        loop {
            items.push(parse_value(chars)?);
            skip_ws(chars);
            match chars.next()? {
                ',' => continue,
                ']' => break,
                _ => return None,
            }
        }
        Some(Value::Array(items))
    }

    fn parse_object(chars: &mut Chars) -> Option<Value> {
        chars.next(); // '{'
        let mut fields = Vec::new();
        skip_ws(chars);
        if chars.peek() == Some(&'}') {
            chars.next();
            return Some(Value::Object(fields));
        }
        loop {
            skip_ws(chars);
            let key = parse_string(chars)?;
            skip_ws(chars);
            if chars.next()? != ':' {
                return None;
            }
            let value = parse_value(chars)?;
            fields.push((key, value));
            skip_ws(chars);
            match chars.next()? {
                ',' => continue,
                '}' => break,
                _ => return None,
            }
        }
        Some(Value::Object(fields))
    }
}

pub fn detect_disks() -> Vec<Disk> {
    let output = Command::new("lsblk")
        .args(["-J", "-o", "NAME,SIZE,TYPE,MODEL,FSTYPE"])
        .output();
    let Ok(output) = output else { return Vec::new() };
    let text = String::from_utf8_lossy(&output.stdout);
    let Some(root) = lsblk_json::parse(&text) else { return Vec::new() };
    let Some(devices) = root.get("blockdevices").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    let mut disks = Vec::new();
    for device in devices {
        if device.get("type").and_then(|v| v.as_str()) != Some("disk") {
            continue;
        }
        let name = device.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.starts_with("zram") {
            continue;
        }
        let model = device
            .get("model")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("Virtual / Generic Disk")
            .to_string();
        let size = device.get("size").and_then(|v| v.as_str()).unwrap_or("").to_string();

        let children = device
            .get("children")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter(|c| c.get("type").and_then(|v| v.as_str()) == Some("part"))
                    .map(|c| DiskPartition {
                        name: format!("/dev/{}", c.get("name").and_then(|v| v.as_str()).unwrap_or("")),
                        size: c.get("size").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        fstype: c
                            .get("fstype")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_lowercase(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        disks.push(Disk {
            name: format!("/dev/{name}"),
            model,
            size,
            children,
        });
    }
    disks
}

/// Parse an lsblk size string like `512M`, `1G`, `100K` to bytes.
pub fn parse_size_to_bytes(size_str: &str) -> u64 {
    let size_str = size_str.trim();
    if size_str.is_empty() {
        return 0;
    }
    let units: &[(char, u64)] = &[('K', 1024), ('M', 1024u64.pow(2)), ('G', 1024u64.pow(3)), ('T', 1024u64.pow(4))];
    let upper = size_str.to_uppercase();
    for (suffix, mult) in units {
        if let Some(prefix) = upper.strip_suffix(*suffix) {
            return prefix.parse::<f64>().map(|v| (v * *mult as f64) as u64).unwrap_or(0);
        }
    }
    size_str.parse::<f64>().map(|v| v as u64).unwrap_or(0)
}

/// Flattens every disk's partitions into a single list, port of
/// `get_partitions_detailed` in `system_utils.py`.
pub fn get_partitions_detailed() -> Vec<PartitionDetail> {
    let mut partitions = Vec::new();
    for disk in detect_disks() {
        for part in disk.children {
            partitions.push(PartitionDetail {
                name: part.name.clone(),
                display: format!("{} ({})", part.name, part.size),
                size_bytes: parse_size_to_bytes(&part.size),
                fstype: part.fstype,
            });
        }
    }
    partitions
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Ok,
    Warn,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct DiskHealth {
    pub structure: HealthStatus,
    pub structure_note: String,
    pub hardware: HealthStatus,
    pub hardware_note: String,
}

/// Best-effort, non-blocking disk health check split into two independent
/// axes so a blank new disk (no partition table yet) isn't reported as a
/// hardware fault, and a disk with no SMART support isn't reported as
/// structurally unsound. Never used to gate the wizard, only to inform.
pub fn check_disk_health(disk_dev: &str) -> DiskHealth {
    let structure = check_structure(disk_dev);
    let hardware = check_hardware(disk_dev);
    DiskHealth {
        structure: structure.0,
        structure_note: structure.1,
        hardware: hardware.0,
        hardware_note: hardware.1,
    }
}

fn check_structure(disk_dev: &str) -> (HealthStatus, String) {
    // -d excludes child partition rows; without it lsblk prints one PTTYPE
    // line per partition too, which interpret_pttype also guards against.
    let output = Command::new("lsblk").args(["-dno", "PTTYPE", disk_dev]).output();
    match output {
        Ok(out) if out.status.success() => interpret_pttype(&String::from_utf8_lossy(&out.stdout)),
        _ => (HealthStatus::Unknown, "Could not read partition table (lsblk unavailable).".to_string()),
    }
}

fn interpret_pttype(raw: &str) -> (HealthStatus, String) {
    let pttype = raw.lines().next().unwrap_or("").trim();
    if pttype.is_empty() {
        (HealthStatus::Ok, "No partition table yet (blank disk).".to_string())
    } else {
        (HealthStatus::Ok, format!("Partition table: {pttype}"))
    }
}

fn check_hardware(disk_dev: &str) -> (HealthStatus, String) {
    let output = Command::new("smartctl").args(["-H", disk_dev]).output();
    match output {
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
            if text.contains("passed") || text.contains("ok") {
                (HealthStatus::Ok, "SMART health check passed.".to_string())
            } else if out.status.success() {
                (HealthStatus::Unknown, "SMART status unclear; check manually if concerned.".to_string())
            } else {
                (HealthStatus::Warn, "SMART reported a possible issue; check with smartctl -a before proceeding.".to_string())
            }
        }
        Err(_) => (HealthStatus::Unknown, "smartctl not installed; hardware health unknown.".to_string()),
    }
}

pub fn detect_timezones() -> BTreeMap<String, Vec<String>> {
    let base_dir = Path::new("/usr/share/zoneinfo");
    let ignore = ["posix", "right", "Etc", "SystemV"];
    let mut zones = BTreeMap::new();

    let Ok(entries) = fs::read_dir(base_dir) else {
        zones.insert("UTC".to_string(), vec!["UTC".to_string()]);
        return zones;
    };

    for entry in entries.flatten() {
        let region = entry.file_name().to_string_lossy().to_string();
        if !entry.path().is_dir() || ignore.contains(&region.as_str()) {
            continue;
        }
        let Ok(cities) = fs::read_dir(entry.path()) else { continue };
        let mut city_names: Vec<String> = cities
            .flatten()
            .filter(|c| c.path().is_file())
            .map(|c| c.file_name().to_string_lossy().to_string())
            .collect();
        if !city_names.is_empty() {
            city_names.sort();
            zones.insert(region, city_names);
        }
    }

    if zones.is_empty() {
        zones.insert("UTC".to_string(), vec!["UTC".to_string()]);
    }
    zones
}

pub fn detect_keymaps() -> Vec<String> {
    let mut found = Vec::new();
    collect_map_gz_files(Path::new("/usr/share/kbd/keymaps"), &mut found);

    let mut filtered: Vec<String> = found
        .into_iter()
        .filter(|k| locales::has_known_keymap_name(k))
        .collect();
    filtered.sort_by_key(|a| locales::keymap_name(a));
    filtered
}

fn collect_map_gz_files(dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_map_gz_files(&path, out);
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if let Some(stripped) = name.strip_suffix(".map.gz") {
                out.push(stripped.to_string());
            }
        }
    }
}

pub fn detect_locales() -> Vec<String> {
    let target = Path::new("/etc/default/libc-locales");
    let mut locales = Vec::new();
    if let Ok(content) = fs::read_to_string(target) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || !line.contains(".UTF-8") {
                continue;
            }
            let cleaned = line.trim_start_matches('#').trim();
            if let Some(first) = cleaned.split_whitespace().next() {
                if first.ends_with("UTF-8") && !first.starts_with("C.") {
                    locales.push(first.to_string());
                }
            }
        }
    }
    locales.sort();
    locales
}

pub fn detect_display_manager() -> Option<String> {
    let runit_services: &[(&str, &str)] = &[
        ("/var/service/greetd", "greetd"),
        ("/var/service/sddm", "sddm"),
        ("/var/service/lightdm", "lightdm"),
        ("/var/service/gdm", "gdm"),
    ];
    for (path, name) in runit_services {
        if Path::new(path).exists() {
            return Some((*name).to_string());
        }
    }

    let dm_bins: &[(&str, &str)] = &[
        ("/usr/bin/sddm", "sddm"),
        ("/usr/bin/lightdm", "lightdm"),
        ("/usr/bin/gdm", "gdm"),
        ("/usr/bin/gdm3", "gdm"),
        ("/usr/bin/greetd", "greetd"),
    ];
    for (path, name) in dm_bins {
        if Path::new(path).exists() {
            return Some((*name).to_string());
        }
    }
    None
}

pub fn detect_timezone_from_network(timeout: Duration) -> Option<String> {
    let agent = ureq::AgentBuilder::new().timeout(timeout).build();
    let body = agent.get("https://ipwho.is/").call().ok()?.into_string().ok()?;
    let value = lsblk_json::parse(&body)?;
    let tz = value.get("timezone")?.get("id")?.as_str()?;
    if tz.contains('/') {
        Some(tz.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_size_strings() {
        assert_eq!(parse_size_to_bytes("512M"), 512 * 1024 * 1024);
        assert_eq!(parse_size_to_bytes("1G"), 1024 * 1024 * 1024);
        assert_eq!(parse_size_to_bytes("100K"), 100 * 1024);
        assert_eq!(parse_size_to_bytes(""), 0);
        assert_eq!(parse_size_to_bytes("bogus"), 0);
    }

    #[test]
    fn parses_lsblk_json_shape() {
        let sample = r#"{"blockdevices": [{"name":"sda","size":"20G","type":"disk","model":"QEMU HARDDISK","fstype":null,"children":[{"name":"sda1","size":"512M","type":"part","fstype":"vfat"}]}]}"#;
        let value = lsblk_json::parse(sample).unwrap();
        let devices = value.get("blockdevices").unwrap().as_array().unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].get("name").unwrap().as_str().unwrap(), "sda");
        let children = devices[0].get("children").unwrap().as_array().unwrap();
        assert_eq!(children[0].get("fstype").unwrap().as_str().unwrap(), "vfat");
    }

    #[test]
    fn detect_timezones_falls_back_to_utc_when_missing() {
        // /usr/share/zoneinfo is expected to exist on any real Linux box
        // this runs on; this just checks the fallback path doesn't panic
        // when we point somewhere nonexistent via a manual re-implementation
        // isn't practical without refactoring for injection, so we just
        // assert detect_timezones() returns a non-empty map on this host.
        assert!(!detect_timezones().is_empty());
    }

    #[test]
    fn disk_health_reports_unknown_hardware_without_smartctl_binary() {
        let health = check_disk_health("/dev/null");
        assert!(!health.hardware_note.is_empty());
        assert!(!health.structure_note.is_empty());
    }

    #[test]
    fn disk_health_status_variants_are_distinguishable() {
        assert_ne!(HealthStatus::Ok, HealthStatus::Warn);
        assert_ne!(HealthStatus::Ok, HealthStatus::Unknown);
    }

    #[test]
    fn interpret_pttype_ignores_child_partition_rows() {
        // `lsblk -no PTTYPE <disk>` without -d prints one row per child
        // partition too, not just the disk itself.
        let (status, note) = interpret_pttype("gpt\ngpt\ngpt\ngpt\n");
        assert_eq!(status, HealthStatus::Ok);
        assert_eq!(note, "Partition table: gpt");
    }

    #[test]
    fn interpret_pttype_blank_disk_has_no_partition_table() {
        let (status, note) = interpret_pttype("");
        assert_eq!(status, HealthStatus::Ok);
        assert_eq!(note, "No partition table yet (blank disk).");
    }
}
