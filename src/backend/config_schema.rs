use std::process::Command;

const BIOS_BOOT_PARTTYPE_GUID: &str = "21686148-6449-6e6f-744e-656f656e7451";

pub type FieldError = (String, String);

#[derive(Debug, Default, Clone)]
pub struct RawPartitions {
    pub root: Option<String>,
    pub efi: Option<String>,
    pub swap: Option<String>,
    pub home: Option<String>,
}

/// Which drivers the installed system's initramfs carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DriverSet {
    /// Everything: the disk can be moved to other hardware (Debian's "generic").
    #[default]
    Generic,
    /// Only what this machine needs: smaller and faster to boot (Debian's "targeted").
    Targeted,
}

impl DriverSet {
    pub fn as_conf_str(self) -> &'static str {
        match self {
            DriverSet::Generic => "generic",
            DriverSet::Targeted => "targeted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Partition {
    pub dev: String,
    pub point: String,
    pub fs: String,
    pub format: bool,
}

#[derive(Debug, Default, Clone)]
pub struct InstallFields {
    pub locale: String,
    pub timezone_region: String,
    pub timezone_city: String,
    pub keymap: String,
    pub hostname: String,
    pub userlogin: String,
    pub username: String,
    pub userpassword: String,
    pub rootpassword: String,
    pub autologin: bool,
    pub mirror: String,
    pub net: bool,
    pub nonfree: bool,
    pub hw_drivers: bool,
    pub driver_set: DriverSet,
}

#[derive(Debug, Clone)]
pub struct InstallConfig {
    pub locale: String,
    pub timezone: String,
    pub keymap: String,
    pub hostname: String,
    pub userlogin: String,
    pub username: String,
    pub userpassword: String,
    pub rootpassword: String,
    pub usergroups: String,
    pub display_manager: String,
    pub autologin: bool,
    pub mirror: String,
    pub update: bool,
    pub nonfree: bool,
    pub hw_drivers: bool,
    pub driver_set: DriverSet,
    pub partitions: Vec<Partition>,
    pub bootloader_disk: String,
    pub bootloader_type: String,
    pub swap_strategy: String,
    pub btrfs_flat: bool,
    pub btrfs_snapshots: bool,
}

fn hostname_valid(hostname: &str) -> bool {
    if hostname.is_empty() || hostname != hostname.to_lowercase() {
        return false;
    }
    let bytes: Vec<char> = hostname.chars().collect();
    let is_alnum = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit();
    let is_alnum_or_hyphen = |c: char| is_alnum(c) || c == '-';

    if bytes.len() == 1 {
        return is_alnum(bytes[0]);
    }
    is_alnum(bytes[0])
        && is_alnum(*bytes.last().unwrap())
        && bytes[1..bytes.len() - 1]
            .iter()
            .all(|&c| is_alnum_or_hyphen(c))
}

fn username_valid(username: &str) -> bool {
    if username.is_empty() || username != username.to_lowercase() {
        return false;
    }
    let mut chars = username.chars();
    let first = chars.next().unwrap();
    if !(first.is_ascii_lowercase() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

fn gpt_without_bios_boot_from_lsblk(pttype: &str, parttypes: &[String]) -> bool {
    if pttype.lines().next().unwrap_or("").trim() != "gpt" {
        return false;
    }
    !parttypes
        .iter()
        .any(|t| t.trim().to_lowercase() == BIOS_BOOT_PARTTYPE_GUID)
}

fn is_gpt_without_bios_boot(disk_dev: &str) -> bool {
    let pttype_out = Command::new("lsblk")
        .args(["-dno", "PTTYPE", disk_dev])
        .output();
    let Ok(pttype_out) = pttype_out else {
        return false;
    };
    if !pttype_out.status.success() {
        return false;
    }
    let pttype = String::from_utf8_lossy(&pttype_out.stdout)
        .trim()
        .to_string();

    let parttypes_out = Command::new("lsblk")
        .args(["-rno", "PARTTYPE", disk_dev])
        .output();
    let Ok(parttypes_out) = parttypes_out else {
        return false;
    };
    if !parttypes_out.status.success() {
        return false;
    }
    let parttypes: Vec<String> = String::from_utf8_lossy(&parttypes_out.stdout)
        .lines()
        .map(str::to_string)
        .collect();

    gpt_without_bios_boot_from_lsblk(&pttype, &parttypes)
}

fn bootloader_disk(root_dev: &str) -> String {
    let trimmed = root_dev.trim_end_matches(|c: char| c.is_ascii_digit());
    if trimmed.contains("nvme") && trimmed.ends_with('p') {
        trimmed[..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SwapStrategy {
    #[default]
    None,
    Partition,
    Swapfile,
}

impl SwapStrategy {
    pub fn as_conf_str(self) -> &'static str {
        match self {
            SwapStrategy::None => "none",
            SwapStrategy::Partition => "partition",
            SwapStrategy::Swapfile => "swapfile",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DiskChoices {
    pub raw_parts: RawPartitions,
    pub filesystem: String,
    pub want_efi: bool,
    pub bootloader_type: String,
    pub swap_strategy: SwapStrategy,
    pub btrfs_flat: bool,
    pub btrfs_snapshots: bool,
}

pub fn build_partitions(
    raw_parts: &RawPartitions,
    filesystem: &str,
    want_efi: bool,
    swap_strategy: SwapStrategy,
) -> Result<Vec<Partition>, Vec<FieldError>> {
    let mut errors = Vec::new();
    let mut partitions = Vec::new();

    match &raw_parts.root {
        None => errors.push((
            "root".to_string(),
            "You must select a Root (/) partition.".to_string(),
        )),
        Some(dev) => partitions.push(Partition {
            dev: dev.clone(),
            point: "/".to_string(),
            fs: filesystem.to_string(),
            format: true,
        }),
    }

    if want_efi {
        match &raw_parts.efi {
            None => errors.push((
                "efi".to_string(),
                "You must select an EFI (/boot/efi) partition.".to_string(),
            )),
            Some(dev) => partitions.push(Partition {
                dev: dev.clone(),
                point: "/boot/efi".to_string(),
                fs: "vfat".to_string(),
                format: true,
            }),
        }
    }

    match swap_strategy {
        SwapStrategy::Partition => match &raw_parts.swap {
            None => errors.push((
                "swap".to_string(),
                "You must select a Swap partition, or choose a different swap strategy."
                    .to_string(),
            )),
            Some(dev) => partitions.push(Partition {
                dev: dev.clone(),
                point: "none".to_string(),
                fs: "swap".to_string(),
                format: true,
            }),
        },
        SwapStrategy::None | SwapStrategy::Swapfile => {}
    }

    if let Some(dev) = &raw_parts.home {
        partitions.push(Partition {
            dev: dev.clone(),
            point: "/home".to_string(),
            fs: filesystem.to_string(),
            format: false,
        });
    }

    if errors.is_empty() {
        Ok(partitions)
    } else {
        Err(errors)
    }
}

fn validate_bootloader_choice(disk: &DiskChoices) -> Result<(), FieldError> {
    if matches!(disk.bootloader_type.as_str(), "limine" | "refind") && !disk.want_efi {
        let name = if disk.bootloader_type == "limine" {
            "Limine"
        } else {
            "rEFInd"
        };
        return Err((
            "bootloader".to_string(),
            format!(
                "{name} requires an EFI system partition; this computer booted in \
                 BIOS mode. Choose GRUB instead."
            ),
        ));
    }

    if disk.bootloader_type == "grub" && !disk.want_efi {
        if let Some(root_dev) = &disk.raw_parts.root {
            let disk_dev = bootloader_disk(root_dev);
            if is_gpt_without_bios_boot(&disk_dev) {
                return Err((
                    "disk".to_string(),
                    format!(
                        "Disk {disk_dev} uses a GPT partition table, but the computer booted in \
                         BIOS mode (not UEFI). GRUB needs a small, unformatted \u{ab}BIOS Boot\u{bb} \
                         partition (1 MiB) on that disk to be able to install itself. Use automatic \
                         partitioning, or create that partition manually with GParted before continuing."
                    ),
                ));
            }
        }
    }

    Ok(())
}

pub fn build_config(
    fields: &InstallFields,
    disk: &DiskChoices,
    display_manager: &str,
) -> Result<InstallConfig, Vec<FieldError>> {
    let mut errors = Vec::new();

    let required: &[(&str, &str)] = &[
        ("hostname", "Computer name (hostname)"),
        ("userlogin", "Username (login)"),
        ("username", "Full name"),
        ("userpassword", "User password"),
        ("rootpassword", "Root password"),
    ];
    let field_value = |key: &str| -> &str {
        match key {
            "hostname" => &fields.hostname,
            "userlogin" => &fields.userlogin,
            "username" => &fields.username,
            "userpassword" => &fields.userpassword,
            "rootpassword" => &fields.rootpassword,
            _ => "",
        }
    };
    for (key, label) in required {
        if field_value(key).is_empty() {
            errors.push((key.to_string(), format!("{label} is required.")));
        }
    }

    if !fields.hostname.is_empty() && !hostname_valid(&fields.hostname) {
        errors.push((
            "hostname".to_string(),
            "The computer name can only contain lowercase letters, numbers \
             and hyphens, and cannot start or end with a hyphen."
                .to_string(),
        ));
    }

    if !fields.userlogin.is_empty() && !username_valid(&fields.userlogin) {
        errors.push((
            "userlogin".to_string(),
            "The username can only contain lowercase letters, numbers, \
             hyphens and underscores, and cannot contain spaces."
                .to_string(),
        ));
    }

    for (key, label, pwd) in [
        ("userpassword", "User password", &fields.userpassword),
        ("rootpassword", "Root password", &fields.rootpassword),
    ] {
        if !pwd.is_empty() && pwd.trim() != pwd.as_str() {
            errors.push((
                key.to_string(),
                format!("{label} cannot start or end with spaces."),
            ));
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    let partitions = build_partitions(
        &disk.raw_parts,
        &disk.filesystem,
        disk.want_efi,
        disk.swap_strategy,
    )?;

    let root_dev = disk.raw_parts.root.as_ref().unwrap();
    let disk_dev = bootloader_disk(root_dev);

    if let Err(e) = validate_bootloader_choice(disk) {
        return Err(vec![e]);
    }

    let mirror_key = if fields.mirror.is_empty() {
        "Local"
    } else {
        fields.mirror.as_str()
    };
    let btrfs_snapshots = disk.btrfs_snapshots && !disk.btrfs_flat;

    Ok(InstallConfig {
        locale: fields.locale.clone(),
        timezone: format!("{}/{}", fields.timezone_region, fields.timezone_city),
        keymap: fields.keymap.clone(),
        hostname: fields.hostname.clone(),
        userlogin: fields.userlogin.clone(),
        username: fields.username.clone(),
        userpassword: fields.userpassword.clone(),
        rootpassword: fields.rootpassword.clone(),
        usergroups: "wheel,audio,video,users,network,optical,cdrom".to_string(),
        display_manager: display_manager.to_string(),
        autologin: fields.autologin,
        mirror: mirror_key.to_string(),
        update: mirror_key != "Local" && fields.net,
        nonfree: fields.nonfree,
        hw_drivers: fields.hw_drivers,
        driver_set: fields.driver_set,
        partitions,
        bootloader_disk: disk_dev,
        bootloader_type: disk.bootloader_type.clone(),
        swap_strategy: disk.swap_strategy.as_conf_str().to_string(),
        btrfs_flat: disk.btrfs_flat,
        btrfs_snapshots,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_fields() -> InstallFields {
        InstallFields {
            locale: "en_US.UTF-8".to_string(),
            timezone_region: "America".to_string(),
            timezone_city: "New_York".to_string(),
            keymap: "us".to_string(),
            hostname: "void-box".to_string(),
            userlogin: "gui".to_string(),
            username: "Gui".to_string(),
            userpassword: "hunter2".to_string(),
            rootpassword: "hunter2root".to_string(),
            autologin: false,
            mirror: "Default".to_string(),
            net: true,
            nonfree: false,
            hw_drivers: false,
            driver_set: DriverSet::Generic,
        }
    }

    fn efi_parts() -> RawPartitions {
        RawPartitions {
            root: Some("/dev/sda2".to_string()),
            efi: Some("/dev/sda1".to_string()),
            swap: None,
            home: None,
        }
    }

    fn efi_disk_choices() -> DiskChoices {
        DiskChoices {
            raw_parts: efi_parts(),
            filesystem: "ext4".to_string(),
            want_efi: true,
            bootloader_type: "grub".to_string(),
            swap_strategy: SwapStrategy::None,
            btrfs_flat: false,
            btrfs_snapshots: false,
        }
    }

    #[test]
    fn hostname_rules_match_python_regex() {
        assert!(hostname_valid("void"));
        assert!(hostname_valid("a"));
        assert!(hostname_valid("void-box-1"));
        assert!(!hostname_valid(""));
        assert!(!hostname_valid("-void"));
        assert!(!hostname_valid("void-"));
        assert!(!hostname_valid("Void"));
        assert!(!hostname_valid("void_box"));
    }

    #[test]
    fn username_rules_match_python_regex() {
        assert!(username_valid("gui"));
        assert!(username_valid("_gui"));
        assert!(username_valid("gui-2"));
        assert!(!username_valid(""));
        assert!(!username_valid("Gui"));
        assert!(!username_valid("2gui"));
        assert!(!username_valid("gui name"));
    }

    #[test]
    fn bootloader_disk_strips_partition_number() {
        assert_eq!(bootloader_disk("/dev/sda1"), "/dev/sda");
        assert_eq!(bootloader_disk("/dev/sda12"), "/dev/sda");
        assert_eq!(bootloader_disk("/dev/nvme0n1p2"), "/dev/nvme0n1");
    }

    #[test]
    fn gpt_without_bios_boot_detects_missing_partition() {
        let parttypes = vec!["c12a7328-f81f-11d2-ba4b-00a0c93ec93b".to_string()];
        assert!(gpt_without_bios_boot_from_lsblk("gpt", &parttypes));
    }

    #[test]
    fn gpt_without_bios_boot_detects_missing_partition_with_multiline_pttype() {
        let parttypes = vec!["c12a7328-f81f-11d2-ba4b-00a0c93ec93b".to_string()];
        assert!(gpt_without_bios_boot_from_lsblk(
            "gpt\ngpt\ngpt\n",
            &parttypes
        ));
    }

    #[test]
    fn gpt_without_bios_boot_false_when_present() {
        let parttypes = vec![BIOS_BOOT_PARTTYPE_GUID.to_string()];
        assert!(!gpt_without_bios_boot_from_lsblk("gpt", &parttypes));
    }

    #[test]
    fn gpt_without_bios_boot_false_for_mbr() {
        let parttypes = vec![];
        assert!(!gpt_without_bios_boot_from_lsblk("dos", &parttypes));
    }

    #[test]
    fn build_partitions_requires_root() {
        let err = build_partitions(&RawPartitions::default(), "ext4", false, SwapStrategy::None)
            .unwrap_err();
        assert_eq!(err[0].0, "root");
    }

    #[test]
    fn build_partitions_requires_efi_when_wanted() {
        let raw = RawPartitions {
            root: Some("/dev/sda2".to_string()),
            ..Default::default()
        };
        let err = build_partitions(&raw, "ext4", true, SwapStrategy::None).unwrap_err();
        assert_eq!(err[0].0, "efi");
    }

    #[test]
    fn build_partitions_happy_path() {
        let raw = RawPartitions {
            root: Some("/dev/sda2".to_string()),
            efi: Some("/dev/sda1".to_string()),
            swap: Some("/dev/sda3".to_string()),
            home: Some("/dev/sda4".to_string()),
        };
        let parts = build_partitions(&raw, "ext4", true, SwapStrategy::Partition).unwrap();
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[0].point, "/");
        assert!(parts[0].format);
        assert_eq!(parts[3].point, "/home");
        assert!(!parts[3].format);
    }

    #[test]
    fn build_config_rejects_missing_required_fields() {
        let mut fields = valid_fields();
        fields.hostname = String::new();
        let err = build_config(&fields, &efi_disk_choices(), "").unwrap_err();
        assert!(err.iter().any(|(k, _)| k == "hostname"));
    }

    #[test]
    fn build_config_rejects_password_mismatch_whitespace() {
        let mut fields = valid_fields();
        fields.userpassword = " leading".to_string();
        let err = build_config(&fields, &efi_disk_choices(), "").unwrap_err();
        assert!(err.iter().any(|(k, _)| k == "userpassword"));
    }

    #[test]
    fn build_config_limine_requires_efi() {
        let fields = valid_fields();
        let disk = DiskChoices {
            raw_parts: RawPartitions {
                root: Some("/dev/sda1".to_string()),
                ..Default::default()
            },
            filesystem: "ext4".to_string(),
            want_efi: false,
            bootloader_type: "limine".to_string(),
            ..Default::default()
        };
        let err = build_config(&fields, &disk, "").unwrap_err();
        assert_eq!(err[0].0, "bootloader");
    }

    #[test]
    fn build_config_partition_strategy_requires_swap_partition() {
        let fields = valid_fields();
        let mut disk = efi_disk_choices();
        disk.swap_strategy = SwapStrategy::Partition;
        let err = build_config(&fields, &disk, "").unwrap_err();
        assert!(err.iter().any(|(k, _)| k == "swap"));
    }

    #[test]
    fn build_config_swapfile_strategy_ignores_swap_partition_selection() {
        let fields = valid_fields();
        let mut disk = efi_disk_choices();
        disk.raw_parts.swap = Some("/dev/sda3".to_string());
        disk.swap_strategy = SwapStrategy::Swapfile;
        let cfg = build_config(&fields, &disk, "").unwrap();
        assert_eq!(cfg.swap_strategy, "swapfile");
        assert!(!cfg.partitions.iter().any(|p| p.fs == "swap"));
    }

    #[test]
    fn build_config_carries_btrfs_layout_choice() {
        let fields = valid_fields();
        let mut disk = efi_disk_choices();
        disk.filesystem = "btrfs".to_string();
        disk.btrfs_flat = true;
        disk.btrfs_snapshots = false;
        let cfg = build_config(&fields, &disk, "").unwrap();
        assert!(cfg.btrfs_flat);
        assert!(!cfg.btrfs_snapshots);
    }

    #[test]
    fn build_config_flat_btrfs_ignores_snapshot_request() {
        let fields = valid_fields();
        let mut disk = efi_disk_choices();
        disk.filesystem = "btrfs".to_string();
        disk.btrfs_flat = true;
        disk.btrfs_snapshots = true;
        let cfg = build_config(&fields, &disk, "").unwrap();
        assert!(
            !cfg.btrfs_snapshots,
            "flat layout has no subvolume boundary for @snapshots"
        );
    }

    #[test]
    fn validate_bootloader_choice_refind_requires_efi() {
        let disk = DiskChoices {
            raw_parts: RawPartitions {
                root: Some("/dev/sda1".to_string()),
                ..Default::default()
            },
            filesystem: "ext4".to_string(),
            want_efi: false,
            bootloader_type: "refind".to_string(),
            ..Default::default()
        };
        let err = validate_bootloader_choice(&disk).unwrap_err();
        assert_eq!(err.0, "bootloader");
    }

    #[test]
    fn validate_bootloader_choice_grub_efi_never_needs_bios_boot_partition() {
        let disk = DiskChoices {
            raw_parts: RawPartitions {
                root: Some("/dev/sda2".to_string()),
                ..Default::default()
            },
            filesystem: "ext4".to_string(),
            want_efi: true,
            bootloader_type: "grub".to_string(),
            ..Default::default()
        };
        assert!(validate_bootloader_choice(&disk).is_ok());
    }

    #[test]
    fn build_config_happy_path_efi_grub() {
        let fields = valid_fields();
        let cfg = build_config(&fields, &efi_disk_choices(), "gdm").unwrap();
        assert_eq!(cfg.timezone, "America/New_York");
        assert_eq!(cfg.bootloader_disk, "/dev/sda");
        assert_eq!(cfg.mirror, "Default");
        assert!(cfg.update);
        assert_eq!(cfg.partitions.len(), 2);
        assert_eq!(cfg.swap_strategy, "none");
        assert!(!cfg.btrfs_flat);
        assert!(!cfg.btrfs_snapshots);
    }

    #[test]
    fn build_config_local_mirror_never_updates() {
        let mut fields = valid_fields();
        fields.mirror = "Local".to_string();
        let cfg = build_config(&fields, &efi_disk_choices(), "").unwrap();
        assert!(!cfg.update);
    }
}
