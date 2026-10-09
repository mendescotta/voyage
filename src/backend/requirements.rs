//! System requirements shown on the Welcome page. `evaluate` is pure (facts in, checks out) so the
//! thresholds are unit-tested; `Facts::detect` reads the real machine.

use std::fs;
use std::path::Path;

use super::system_detect;

const GIB: u64 = 1024 * 1024 * 1024;
/// Below this the installer cannot work at all.
pub const MIN_RAM: u64 = GIB;
/// Below this a desktop will feel starved.
pub const RECOMMENDED_RAM: u64 = 2 * GIB;
/// The copied system plus swap and a little room.
pub const MIN_DISK: u64 = 8 * GIB;
pub const RECOMMENDED_DISK: u64 = 20 * GIB;
/// On battery below this percentage, ask for the charger.
const LOW_BATTERY: u8 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone)]
pub struct Check {
    pub label: String,
    pub detail: String,
    pub level: Level,
    /// A failed required check blocks the installation; any other result only informs.
    pub required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Battery {
    pub discharging: bool,
    pub percent: u8,
}

#[derive(Debug, Clone, Default)]
pub struct Facts {
    pub ram_bytes: u64,
    /// Size of the largest disk that may be installed to (the live medium is already excluded).
    pub largest_disk_bytes: u64,
    pub uefi: bool,
    pub battery: Option<Battery>,
    pub internet: bool,
    pub pkexec: bool,
}

pub fn format_bytes(bytes: u64) -> String {
    let gib = bytes as f64 / GIB as f64;
    if gib >= 1.0 {
        format!("{gib:.1} GiB")
    } else {
        format!("{:.0} MiB", bytes as f64 / (1024.0 * 1024.0))
    }
}

pub fn evaluate(f: &Facts) -> Vec<Check> {
    let mut checks = Vec::new();

    let (level, detail) = if f.ram_bytes < MIN_RAM {
        (
            Level::Fail,
            format!(
                "{} found, at least {} is needed",
                format_bytes(f.ram_bytes),
                format_bytes(MIN_RAM)
            ),
        )
    } else if f.ram_bytes < RECOMMENDED_RAM {
        (
            Level::Warn,
            format!(
                "{} found, {} or more is recommended",
                format_bytes(f.ram_bytes),
                format_bytes(RECOMMENDED_RAM)
            ),
        )
    } else {
        (Level::Ok, format_bytes(f.ram_bytes))
    };
    checks.push(Check {
        label: "Memory".into(),
        detail,
        level,
        required: true,
    });

    let (level, detail) = if f.largest_disk_bytes == 0 {
        (
            Level::Fail,
            "No disk to install to was found (the install medium itself is never offered)".into(),
        )
    } else if f.largest_disk_bytes < MIN_DISK {
        (
            Level::Fail,
            format!(
                "The largest disk is {}, at least {} is needed",
                format_bytes(f.largest_disk_bytes),
                format_bytes(MIN_DISK)
            ),
        )
    } else if f.largest_disk_bytes < RECOMMENDED_DISK {
        (
            Level::Warn,
            format!(
                "The largest disk is {}, {} or more is recommended",
                format_bytes(f.largest_disk_bytes),
                format_bytes(RECOMMENDED_DISK)
            ),
        )
    } else {
        (
            Level::Ok,
            format!("Largest disk: {}", format_bytes(f.largest_disk_bytes)),
        )
    };
    checks.push(Check {
        label: "Storage".into(),
        detail,
        level,
        required: true,
    });

    checks.push(Check {
        label: "Firmware".into(),
        detail: if f.uefi {
            "UEFI".into()
        } else {
            "Legacy BIOS".into()
        },
        level: Level::Ok,
        required: false,
    });

    let (level, detail) = match f.battery {
        Some(b) if b.discharging && b.percent < LOW_BATTERY => (
            Level::Warn,
            format!(
                "On battery at {}%: plug in the charger before installing",
                b.percent
            ),
        ),
        Some(b) if b.discharging => (Level::Ok, format!("On battery ({}%)", b.percent)),
        Some(_) => (Level::Ok, "Plugged in".into()),
        None => (Level::Ok, "No battery".into()),
    };
    checks.push(Check {
        label: "Power".into(),
        detail,
        level,
        required: false,
    });

    checks.push(Check {
        label: "Internet".into(),
        detail: if f.internet {
            "Connected: mirrors, updates and drivers are available".into()
        } else {
            "Offline: only what is on the live image can be installed".into()
        },
        level: if f.internet { Level::Ok } else { Level::Warn },
        required: false,
    });

    checks.push(Check {
        label: "Administrator access".into(),
        detail: if f.pkexec {
            "pkexec is available".into()
        } else {
            "pkexec was not found: the installer cannot get the rights it needs".into()
        },
        level: if f.pkexec { Level::Ok } else { Level::Fail },
        required: true,
    });

    checks
}

/// The failed required checks. Empty means the installation may go ahead.
pub fn blocking(checks: &[Check]) -> Vec<&Check> {
    checks
        .iter()
        .filter(|c| c.required && c.level == Level::Fail)
        .collect()
}

/// Why the installation cannot start, one line per failed required check (None when it can).
pub fn blocking_summary(checks: &[Check]) -> Option<String> {
    let lines: Vec<String> = blocking(checks)
        .iter()
        .map(|c| format!("{}: {}", c.label, c.detail))
        .collect();
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

/// MemTotal of /proc/meminfo, in bytes (0 when unreadable).
pub fn parse_meminfo(text: &str) -> u64 {
    text.lines()
        .find_map(|l| l.strip_prefix("MemTotal:"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|kib| kib.parse::<u64>().ok())
        .map(|kib| kib * 1024)
        .unwrap_or(0)
}

/// One power supply's `status` and `capacity` files.
pub fn parse_battery(status: &str, capacity: &str) -> Option<Battery> {
    let percent = capacity.trim().parse::<u8>().ok()?.min(100);
    Some(Battery {
        discharging: status.trim().eq_ignore_ascii_case("discharging"),
        percent,
    })
}

fn detect_battery() -> Option<Battery> {
    let entries = fs::read_dir("/sys/class/power_supply").ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let kind = fs::read_to_string(path.join("type")).unwrap_or_default();
        if kind.trim() != "Battery" {
            continue;
        }
        let status = fs::read_to_string(path.join("status")).unwrap_or_default();
        let capacity = fs::read_to_string(path.join("capacity")).unwrap_or_default();
        if let Some(b) = parse_battery(&status, &capacity) {
            return Some(b);
        }
    }
    None
}

fn in_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| Path::new(&dir).join(program).is_file())
        })
        .unwrap_or(false)
}

impl Facts {
    /// Reads the machine. `internet` comes from the caller, which has already asked the network.
    pub fn detect(internet: bool) -> Self {
        let largest_disk_bytes = system_detect::install_targets(system_detect::detect_disks())
            .iter()
            .map(|d| system_detect::parse_size_to_bytes(&d.size))
            .max()
            .unwrap_or(0);
        Self {
            ram_bytes: fs::read_to_string("/proc/meminfo")
                .map(|t| parse_meminfo(&t))
                .unwrap_or(0),
            largest_disk_bytes,
            uefi: system_detect::detect_efi(),
            battery: detect_battery(),
            internet,
            pkexec: in_path("pkexec"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn healthy() -> Facts {
        Facts {
            ram_bytes: 8 * GIB,
            largest_disk_bytes: 500 * GIB,
            uefi: true,
            battery: None,
            internet: true,
            pkexec: true,
        }
    }

    fn level_of(checks: &[Check], label: &str) -> Level {
        checks.iter().find(|c| c.label == label).unwrap().level
    }

    #[test]
    fn a_healthy_machine_passes_everything() {
        let checks = evaluate(&healthy());
        assert!(checks.iter().all(|c| c.level == Level::Ok));
        assert!(blocking(&checks).is_empty());
    }

    #[test]
    fn too_little_memory_blocks() {
        let checks = evaluate(&Facts {
            ram_bytes: GIB / 2,
            ..healthy()
        });
        assert_eq!(level_of(&checks, "Memory"), Level::Fail);
        assert!(!blocking(&checks).is_empty());
        assert_eq!(blocking(&checks)[0].label, "Memory");
    }

    #[test]
    fn modest_memory_only_warns() {
        let checks = evaluate(&Facts {
            ram_bytes: GIB + GIB / 2,
            ..healthy()
        });
        assert_eq!(level_of(&checks, "Memory"), Level::Warn);
        assert!(blocking(&checks).is_empty());
    }

    #[test]
    fn no_installable_disk_blocks_and_says_why() {
        let checks = evaluate(&Facts {
            largest_disk_bytes: 0,
            ..healthy()
        });
        assert_eq!(level_of(&checks, "Storage"), Level::Fail);
        assert!(checks
            .iter()
            .find(|c| c.label == "Storage")
            .unwrap()
            .detail
            .contains("install medium"));
        assert!(!blocking(&checks).is_empty());
    }

    #[test]
    fn a_small_disk_blocks_and_a_mid_sized_one_warns() {
        let small = evaluate(&Facts {
            largest_disk_bytes: 4 * GIB,
            ..healthy()
        });
        assert_eq!(level_of(&small, "Storage"), Level::Fail);
        let mid = evaluate(&Facts {
            largest_disk_bytes: 12 * GIB,
            ..healthy()
        });
        assert_eq!(level_of(&mid, "Storage"), Level::Warn);
        assert!(blocking(&mid).is_empty());
    }

    #[test]
    fn offline_and_low_battery_inform_but_never_block() {
        let checks = evaluate(&Facts {
            internet: false,
            battery: Some(Battery {
                discharging: true,
                percent: 12,
            }),
            ..healthy()
        });
        assert_eq!(level_of(&checks, "Internet"), Level::Warn);
        assert_eq!(level_of(&checks, "Power"), Level::Warn);
        assert!(blocking(&checks).is_empty());
    }

    #[test]
    fn a_charged_battery_or_ac_power_is_fine() {
        for battery in [
            Some(Battery {
                discharging: true,
                percent: 80,
            }),
            Some(Battery {
                discharging: false,
                percent: 5,
            }),
        ] {
            let checks = evaluate(&Facts {
                battery,
                ..healthy()
            });
            assert_eq!(level_of(&checks, "Power"), Level::Ok);
        }
    }

    #[test]
    fn missing_pkexec_blocks() {
        let checks = evaluate(&Facts {
            pkexec: false,
            ..healthy()
        });
        assert_eq!(level_of(&checks, "Administrator access"), Level::Fail);
        assert!(!blocking(&checks).is_empty());
    }

    #[test]
    fn meminfo_is_read_in_bytes() {
        assert_eq!(
            parse_meminfo("MemTotal:       16384000 kB\nMemFree: 1 kB\n"),
            16384000 * 1024
        );
        assert_eq!(parse_meminfo("garbage"), 0);
    }

    #[test]
    fn battery_files_are_understood() {
        assert_eq!(
            parse_battery("Discharging\n", "42\n"),
            Some(Battery {
                discharging: true,
                percent: 42
            })
        );
        assert_eq!(
            parse_battery("Charging\n", "100\n"),
            Some(Battery {
                discharging: false,
                percent: 100
            })
        );
        assert_eq!(parse_battery("Full", "n/a"), None);
    }

    #[test]
    fn sizes_are_shown_in_a_readable_unit() {
        assert_eq!(format_bytes(3 * GIB / 2), "1.5 GiB");
        assert_eq!(format_bytes(512 * 1024 * 1024), "512 MiB");
    }

    #[test]
    fn the_blocking_summary_lists_each_failed_required_check() {
        let checks = evaluate(&Facts {
            ram_bytes: GIB / 2,
            largest_disk_bytes: 0,
            ..healthy()
        });
        let text = blocking_summary(&checks).unwrap();
        assert!(text.contains("Memory"), "{text}");
        assert!(text.contains("Storage"), "{text}");
        assert!(!text.contains("Internet"), "{text}");
    }

    #[test]
    fn no_summary_when_nothing_blocks() {
        assert!(blocking_summary(&evaluate(&healthy())).is_none());
        let warn_only = evaluate(&Facts {
            internet: false,
            ..healthy()
        });
        assert!(blocking_summary(&warn_only).is_none());
    }
}
