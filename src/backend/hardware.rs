//! Hardware-aware driver selection, delegated to `voidhw` (https://github.com/ ... voidhw).
//! The installer asks it what the machine needs, shows a summary, and the backend script runs
//! `voidhw --apply` against the new system.

use serde::Deserialize;
use std::process::Command;

/// The parts of `voidhw --json` the installer shows.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct HardwarePlan {
    pub chassis: String,
    pub hypervisor: Option<String>,
    pub profiles: Vec<String>,
    pub repos: Vec<String>,
    pub packages: Vec<String>,
}

pub fn parse(json: &str) -> Result<HardwarePlan, String> {
    serde_json::from_str(json).map_err(|e| format!("cannot read voidhw output: {e}"))
}

/// Run `voidhw --json`. `None` when it is not installed or fails (the installer then offers no
/// automatic drivers rather than guessing).
pub fn detect() -> Option<HardwarePlan> {
    let output = Command::new("voidhw").arg("--json").output().ok()?;
    if !output.status.success() {
        return None;
    }
    parse(&String::from_utf8_lossy(&output.stdout)).ok()
}

/// One line for the Mirror and Software page: what will be installed and why.
pub fn summary(plan: &HardwarePlan) -> String {
    if plan.packages.is_empty() {
        return "Nothing extra is needed for this hardware.".to_string();
    }
    let mut parts = Vec::new();
    if let Some(hypervisor) = &plan.hypervisor {
        parts.push(format!("virtual machine ({hypervisor})"));
    }
    parts.push(format!(
        "{} profile(s): {}",
        plan.profiles.len(),
        plan.profiles.join(", ")
    ));
    let mut text = parts.join("; ");
    if plan.repos.iter().any(|r| r == "nonfree") {
        text.push_str(". Adds the nonfree repository");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTEBOOK: &str = include_str!("testdata/voidhw-notebook.json");

    #[test]
    fn parses_real_voidhw_output() {
        let plan = parse(NOTEBOOK).unwrap();
        assert_eq!(plan.chassis, "notebook");
        assert!(plan.profiles.contains(&"nvidia-current".to_string()));
        assert!(plan.packages.contains(&"nvidia".to_string()));
        assert_eq!(plan.hypervisor, None);
    }

    #[test]
    fn summary_names_profiles_and_the_nonfree_repo() {
        let text = summary(&parse(NOTEBOOK).unwrap());
        assert!(text.contains("nvidia-current"), "{text}");
        assert!(text.contains("nonfree"), "{text}");
    }

    #[test]
    fn summary_mentions_the_hypervisor() {
        let plan = HardwarePlan {
            hypervisor: Some("vmware".into()),
            profiles: vec!["vm-vmware".into()],
            packages: vec!["open-vm-tools".into()],
            ..HardwarePlan::default()
        };
        let text = summary(&plan);
        assert!(
            text.contains("vmware") && !text.contains("nonfree"),
            "{text}"
        );
    }

    #[test]
    fn empty_plan_says_nothing_is_needed() {
        assert_eq!(
            summary(&HardwarePlan::default()),
            "Nothing extra is needed for this hardware."
        );
    }

    #[test]
    fn extra_and_missing_fields_are_tolerated() {
        let plan = parse(r#"{"profiles": ["a"], "something_new": 1}"#).unwrap();
        assert_eq!(plan.profiles, vec!["a".to_string()]);
        assert!(parse("not json").is_err());
    }
}
