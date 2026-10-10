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
    /// The disk the live system booted from. Never offered as an install target.
    pub live_medium: bool,
}

/// Where a live system mounts the medium it booted from (dracut dmsquash-live, older void-mklive).
const LIVE_MOUNTS: &[&str] = &[
    "/run/initramfs/live",
    "/run/live/medium",
    "/run/initramfs/medium",
];

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

/// True for a VirtualBox guest, from the DMI strings it reports ("VirtualBox" / "innotek GmbH").
pub fn is_virtualbox(product_name: &str, sys_vendor: &str) -> bool {
    product_name.trim().eq_ignore_ascii_case("virtualbox")
        || sys_vendor.to_ascii_lowercase().contains("innotek")
}

pub fn detect_virtualbox() -> bool {
    let read =
        |name: &str| fs::read_to_string(format!("/sys/class/dmi/id/{name}")).unwrap_or_default();
    is_virtualbox(&read("product_name"), &read("sys_vendor"))
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

mod lsblk_json {
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
        chars.next();
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
        chars.next();
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
        chars.next();
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

/// Every mount point of a block device and of everything below it (partitions, crypt and LVM layers).
fn collect_mounts(device: &lsblk_json::Value) -> Vec<String> {
    let mut mounts = Vec::new();
    if let Some(list) = device.get("mountpoints").and_then(|v| v.as_array()) {
        mounts.extend(list.iter().filter_map(|m| m.as_str()).map(str::to_string));
    }
    if let Some(single) = device.get("mountpoint").and_then(|v| v.as_str()) {
        mounts.push(single.to_string());
    }
    if let Some(children) = device.get("children").and_then(|v| v.as_array()) {
        for child in children {
            mounts.extend(collect_mounts(child));
        }
    }
    mounts
}

/// The disks that may be installed to: everything except the medium the live system runs from.
pub fn install_targets(disks: Vec<Disk>) -> Vec<Disk> {
    disks.into_iter().filter(|d| !d.live_medium).collect()
}

pub fn detect_disks() -> Vec<Disk> {
    let output = Command::new("lsblk")
        .args(["-J", "-o", "NAME,SIZE,TYPE,MODEL,FSTYPE,MOUNTPOINTS"])
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    let mut disks = parse_disks(&String::from_utf8_lossy(&output.stdout));
    // Use the backend resolver too: lsblk's tree does not connect a loop-mounted
    // ISO to the filesystem holding its backing file. An unresolved live mount
    // must not leave writable targets available in the UI.
    let live = Command::new("bash")
        .args(["-c", ". \"$1\" && live_medium_disks", "voyage-live-guard"])
        .arg(super::paths::backend_dir().join("live_guard.sh"))
        .output();
    let Ok(live) = live else {
        return Vec::new();
    };
    if !live.status.success() {
        eprintln!("{}", String::from_utf8_lossy(&live.stderr));
        return Vec::new();
    }
    mark_live_devices(&mut disks, &String::from_utf8_lossy(&live.stdout));
    disks
}

fn mark_live_devices(disks: &mut [Disk], live: &str) {
    for disk in disks {
        disk.live_medium |= live.lines().any(|device| device == disk.name);
    }
}

pub fn parse_disks(text: &str) -> Vec<Disk> {
    let Some(root) = lsblk_json::parse(text) else {
        return Vec::new();
    };
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
        let size = device
            .get("size")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let children = device
            .get("children")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter(|c| c.get("type").and_then(|v| v.as_str()) == Some("part"))
                    .map(|c| DiskPartition {
                        name: format!(
                            "/dev/{}",
                            c.get("name").and_then(|v| v.as_str()).unwrap_or("")
                        ),
                        size: c
                            .get("size")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        fstype: c
                            .get("fstype")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_lowercase(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        let live_medium = collect_mounts(device)
            .iter()
            .any(|m| LIVE_MOUNTS.contains(&m.as_str()));
        disks.push(Disk {
            name: format!("/dev/{name}"),
            model,
            size,
            children,
            live_medium,
        });
    }
    disks
}

pub fn parse_size_to_bytes(size_str: &str) -> u64 {
    let size_str = size_str.trim();
    if size_str.is_empty() {
        return 0;
    }
    let units: &[(char, u64)] = &[
        ('K', 1024),
        ('M', 1024u64.pow(2)),
        ('G', 1024u64.pow(3)),
        ('T', 1024u64.pow(4)),
    ];
    let upper = size_str.to_uppercase();
    for (suffix, mult) in units {
        if let Some(prefix) = upper.strip_suffix(*suffix) {
            return prefix
                .parse::<f64>()
                .map(|v| (v * *mult as f64) as u64)
                .unwrap_or(0);
        }
    }
    size_str.parse::<f64>().map(|v| v as u64).unwrap_or(0)
}

pub fn get_partitions_detailed() -> Vec<PartitionDetail> {
    let mut partitions = Vec::new();
    for disk in install_targets(detect_disks()) {
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
    let output = Command::new("lsblk")
        .args(["-dno", "PTTYPE", disk_dev])
        .output();
    match output {
        Ok(out) if out.status.success() => interpret_pttype(&String::from_utf8_lossy(&out.stdout)),
        _ => (
            HealthStatus::Unknown,
            "Could not read partition table (lsblk unavailable).".to_string(),
        ),
    }
}

fn interpret_pttype(raw: &str) -> (HealthStatus, String) {
    let pttype = raw.lines().next().unwrap_or("").trim();
    if pttype.is_empty() {
        (
            HealthStatus::Ok,
            "No partition table yet (blank disk).".to_string(),
        )
    } else {
        (HealthStatus::Ok, format!("Partition table: {pttype}"))
    }
}

fn check_hardware(disk_dev: &str) -> (HealthStatus, String) {
    let output = Command::new("smartctl").args(["-H", disk_dev]).output();
    match output {
        Ok(out) => {
            let exit_code = out.status.code().unwrap_or(-1);
            interpret_smartctl(exit_code, &String::from_utf8_lossy(&out.stdout))
        }
        Err(_) => (
            HealthStatus::Unknown,
            "smartctl not installed; hardware health unknown.".to_string(),
        ),
    }
}

fn interpret_smartctl(exit_code: i32, stdout: &str) -> (HealthStatus, String) {
    if exit_code & 0x07 != 0 {
        return (
            HealthStatus::Unknown,
            "smartctl could not check this disk (missing permissions or unsupported device)."
                .to_string(),
        );
    }
    if exit_code & 0xf8 != 0 {
        return (
            HealthStatus::Warn,
            "SMART reported a possible issue; check with smartctl -a before proceeding."
                .to_string(),
        );
    }
    let passed = stdout.lines().any(|line| {
        let upper = line.to_uppercase();
        upper.contains("SMART OVERALL-HEALTH") && (upper.contains("PASSED") || upper.contains("OK"))
    });
    if passed {
        (HealthStatus::Ok, "SMART health check passed.".to_string())
    } else {
        (
            HealthStatus::Unknown,
            "SMART status unclear; check manually if concerned.".to_string(),
        )
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
        let Ok(cities) = fs::read_dir(entry.path()) else {
            continue;
        };
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
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
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
    let body = agent
        .get("https://ipwho.is/")
        .call()
        .ok()?
        .into_string()
        .ok()?;
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

    #[test]
    fn interpret_smartctl_permission_denied_is_unknown_not_warn() {
        let (status, _) = interpret_smartctl(2, "");
        assert_eq!(status, HealthStatus::Unknown);
    }

    #[test]
    fn interpret_smartctl_real_failure_is_warn() {
        let (status, _) = interpret_smartctl(
            8,
            "SMART overall-health self-assessment test result: FAILED",
        );
        assert_eq!(status, HealthStatus::Warn);
    }

    #[test]
    fn interpret_smartctl_passed_result_line_is_ok() {
        let (status, note) = interpret_smartctl(
            0,
            "SMART overall-health self-assessment test result: PASSED\n",
        );
        assert_eq!(status, HealthStatus::Ok);
        assert_eq!(note, "SMART health check passed.");
    }

    const LIVE_USB: &str = r#"{"blockdevices":[
      {"name":"sda","size":"14.9G","type":"disk","model":"USB DISK","fstype":null,"rm":true,"ro":false,"mountpoints":[null],
       "children":[
         {"name":"sda1","size":"2G","type":"part","fstype":"iso9660","mountpoints":["/run/initramfs/live"]},
         {"name":"sda2","size":"4M","type":"part","fstype":"vfat","mountpoints":[null]}]},
      {"name":"nvme0n1","size":"476.9G","type":"disk","model":"WDC","fstype":null,"rm":false,"ro":false,"mountpoints":[null],
       "children":[{"name":"nvme0n1p1","size":"1G","type":"part","fstype":"vfat","mountpoints":[null]}]}]}"#;

    #[test]
    fn the_disk_holding_the_live_mount_is_marked() {
        let disks = parse_disks(LIVE_USB);
        assert_eq!(disks.len(), 2);
        assert!(disks[0].live_medium, "sda carries /run/initramfs/live");
        assert!(!disks[1].live_medium);
    }

    #[test]
    fn install_targets_leave_out_the_live_medium() {
        let targets = install_targets(parse_disks(LIVE_USB));
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].name, "/dev/nvme0n1");
    }

    #[test]
    fn a_live_mount_below_a_nested_device_still_counts() {
        let json = r#"{"blockdevices":[{"name":"sdb","size":"8G","type":"disk","model":"X","fstype":null,"mountpoints":[null],
          "children":[{"name":"sdb1","size":"8G","type":"part","fstype":"crypto_LUKS","mountpoints":[null],
            "children":[{"name":"live","size":"8G","type":"crypt","fstype":"squashfs","mountpoints":["/run/live/medium"]}]}]}]}"#;
        assert!(parse_disks(json)[0].live_medium);
    }

    #[test]
    fn an_ordinary_mount_does_not_mark_the_disk() {
        let json = r#"{"blockdevices":[{"name":"sda","size":"100G","type":"disk","model":"X","fstype":null,"mountpoints":[null],
          "children":[{"name":"sda1","size":"100G","type":"part","fstype":"ext4","mountpoints":["/run/media/gui/data","/"]}]}]}"#;
        assert!(!parse_disks(json)[0].live_medium);
    }

    #[test]
    fn the_older_single_mountpoint_column_is_understood_too() {
        let json = r#"{"blockdevices":[{"name":"sdc","size":"8G","type":"disk","model":"X","fstype":null,"mountpoint":null,
          "children":[{"name":"sdc1","size":"8G","type":"part","fstype":"iso9660","mountpoint":"/run/initramfs/live"}]}]}"#;
        assert!(parse_disks(json)[0].live_medium);
    }

    #[test]
    fn virtualbox_is_recognised_from_the_dmi_strings() {
        assert!(is_virtualbox("VirtualBox\n", "innotek GmbH\n"));
        assert!(is_virtualbox("VirtualBox", ""));
        assert!(is_virtualbox("", "innotek GmbH"));
        assert!(is_virtualbox("virtualbox", "Oracle Corporation"));
    }

    #[test]
    fn other_machines_are_not_virtualbox() {
        assert!(!is_virtualbox("KVM", "QEMU"));
        assert!(!is_virtualbox("VMware Virtual Platform", "VMware, Inc."));
        assert!(!is_virtualbox("ThinkPad X1", "LENOVO"));
        assert!(!is_virtualbox("", ""));
    }
}

#[cfg(test)]
mod live_backing_tests {
    use super::*;

    #[test]
    fn loop_backing_disks_are_hidden_alongside_direct_live_mounts() {
        let json = r#"{"blockdevices":[
          {"name":"sda","type":"disk","children":[
            {"name":"sda1","type":"part","mountpoints":["/run/initramfs/live"]}]},
          {"name":"sdb","type":"disk"},
          {"name":"sdc","type":"disk"}]}"#;
        let mut disks = parse_disks(json);
        mark_live_devices(&mut disks, "/dev/sdb\n/dev/sr0\n");
        let targets = install_targets(disks);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].name, "/dev/sdc");
    }
}
