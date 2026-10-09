use gtk::prelude::*;

use crate::backend::distro::DistroConf;
use crate::ui::widgets::{ComboRow, EntryRow, Group, SwitchRow};
use crate::ui::SysData;

pub const TITLE: &str = "Users";

pub struct UsersPage {
    pub widget: gtk::Box,
    hostname_row: EntryRow,
    fullname_row: EntryRow,
    userlogin_row: EntryRow,
    userpassword_row: EntryRow,
    userpassword_confirm_row: EntryRow,
    same_password_row: SwitchRow,
    rootpassword_row: EntryRow,
    rootpassword_confirm_row: EntryRow,
    autologin_row: SwitchRow,
    user_shell_row: ComboRow,
    root_shell_row: ComboRow,
    shell_names: Vec<String>,
}

impl UsersPage {
    pub fn new(sys_data: &SysData) -> Self {
        let widget = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(18)
            .margin_top(24)
            .margin_start(24)
            .margin_end(24)
            .build();

        let system_group = Group::new("Computer");
        let hostname_row = EntryRow::new("Computer name", "void");
        system_group.add(&hostname_row);
        widget.append(system_group.as_ref());

        let user_group = Group::new("Your account");
        let fullname_row = EntryRow::new("Full name", "void");
        let userlogin_row = EntryRow::new("Username", "void");
        let userpassword_row = EntryRow::password("Password");
        let userpassword_confirm_row = EntryRow::password("Confirm password");
        user_group.add(&fullname_row);
        user_group.add(&userlogin_row);
        user_group.add(&userpassword_row);
        user_group.add(&userpassword_confirm_row);
        // the shells distro.conf offers; /bin/sh stays dash whichever is chosen
        let distro = DistroConf::load();
        let shell_labels: Vec<String> = distro.shells.iter().map(|s| s.label()).collect();
        let shell_names: Vec<String> = distro.shells.iter().map(|s| s.name.clone()).collect();
        let user_shell_row = ComboRow::new("Login shell", &shell_labels);
        user_shell_row.set_selected(distro.default_index() as u32);
        user_group.add(&user_shell_row);
        widget.append(user_group.as_ref());

        let root_group = Group::new("Root account");
        let same_password_row = SwitchRow::new("Use my account password for root");
        same_password_row.set_subtitle("Skip setting a separate root password.");
        let rootpassword_row = EntryRow::password("Root password");
        let rootpassword_confirm_row = EntryRow::password("Confirm root password");
        root_group.add(&same_password_row);
        root_group.add(&rootpassword_row);
        root_group.add(&rootpassword_confirm_row);
        let root_shell_row = ComboRow::new("Login shell", &shell_labels);
        root_shell_row.set_selected(distro.default_index() as u32);
        root_group.add(&root_shell_row);
        widget.append(root_group.as_ref());
        if let Some(hint) = shell_hint(&distro, sys_data.net) {
            let label = gtk::Label::builder()
                .label(hint)
                .css_classes(["dim-label"])
                .wrap(true)
                .halign(gtk::Align::Start)
                .build();
            widget.append(&label);
        }

        {
            let rootpassword_row = rootpassword_row.clone();
            let rootpassword_confirm_row = rootpassword_confirm_row.clone();
            same_password_row.connect_active(move |row| {
                let same_password = row.is_active();
                for entry in [&rootpassword_row, &rootpassword_confirm_row] {
                    entry.set_sensitive(!same_password);
                    entry.set_text("");
                }
            });
        }

        let autologin_row = SwitchRow::new("Log in automatically");
        user_group.add(&autologin_row);
        configure_autologin(&autologin_row, sys_data.display_manager.as_deref());

        Self {
            widget,
            hostname_row,
            fullname_row,
            userlogin_row,
            userpassword_row,
            userpassword_confirm_row,
            same_password_row,
            rootpassword_row,
            rootpassword_confirm_row,
            autologin_row,
            user_shell_row,
            root_shell_row,
            shell_names,
        }
    }

    pub fn collect(&self) -> (UsersFields, Vec<(String, String)>) {
        let mut errors = Vec::new();
        let userpassword = self.userpassword_row.text().to_string();
        let same_password = self.same_password_row.is_active();
        let rootpassword = if same_password {
            userpassword.clone()
        } else {
            self.rootpassword_row.text().to_string()
        };

        if userpassword != self.userpassword_confirm_row.text() {
            errors.push((
                "userpassword".to_string(),
                "User passwords do not match.".to_string(),
            ));
        }
        if !same_password && rootpassword != self.rootpassword_confirm_row.text() {
            errors.push((
                "rootpassword".to_string(),
                "Root passwords do not match.".to_string(),
            ));
        }

        let fields = UsersFields {
            hostname: self.hostname_row.text().trim().to_string(),
            username: self.fullname_row.text().trim().to_string(),
            userlogin: self.userlogin_row.text().trim().to_string(),
            userpassword,
            rootpassword,
            autologin: self.autologin_row.is_active(),
            user_shell: self.shell_names[self.user_shell_row.selected() as usize].clone(),
            root_shell: self.shell_names[self.root_shell_row.selected() as usize].clone(),
        };
        (fields, errors)
    }
}

/// A note under the shell choices: offline, shells that are not on the live image cannot be installed, and
/// the account then gets the default shell. None when there is nothing to warn about.
fn shell_hint(distro: &DistroConf, net: bool) -> Option<String> {
    if net {
        return None;
    }
    let missing: Vec<&str> = distro
        .shells
        .iter()
        .filter(|s| !s.available_offline())
        .map(|s| s.name.as_str())
        .collect();
    if missing.is_empty() {
        None
    } else {
        Some(format!(
            "Offline: {} would need the internet to be installed; without it the account gets {}.",
            missing.join(" and "),
            distro.default_shell
        ))
    }
}

fn configure_autologin(row: &SwitchRow, display_manager: Option<&str>) {
    let manager = display_manager.unwrap_or("").to_lowercase();
    match manager.as_str() {
        "sddm" | "lightdm" | "gdm" => {
            row.set_sensitive(true);
            row.set_visible(true);
            row.set_subtitle(&format!(
                "Available via {display_manager}.",
                display_manager = display_manager.unwrap_or("")
            ));
        }
        "greetd" => {
            row.set_active(false);
            row.set_sensitive(false);
            row.set_visible(true);
            row.set_subtitle("Not supported with greetd.");
        }
        _ => {
            row.set_active(false);
            row.set_visible(false);
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct UsersFields {
    pub hostname: String,
    pub username: String,
    pub userlogin: String,
    pub userpassword: String,
    pub rootpassword: String,
    pub autologin: bool,
    pub user_shell: String,
    pub root_shell: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conf() -> DistroConf {
        DistroConf::parse(
            "shell bash /bin/bash\nshell ghost /nonexistent/ghost ghost\ndefault-shell bash\n",
        )
    }

    #[test]
    fn no_shell_hint_while_online() {
        assert_eq!(shell_hint(&conf(), true), None);
    }

    #[test]
    fn offline_the_hint_names_the_shells_that_would_need_the_network_and_the_fallback() {
        let hint = shell_hint(&conf(), false).unwrap();
        assert!(hint.contains("ghost"), "{hint}");
        assert!(hint.contains("bash"), "{hint}");
        assert!(!hint.contains("bash would"), "{hint}");
    }

    #[test]
    fn no_hint_when_every_shell_is_on_the_live_image() {
        let all_here =
            DistroConf::parse("shell bash /bin/bash\nshell sh /bin/sh dash\ndefault-shell bash\n");
        assert_eq!(shell_hint(&all_here, false), None);
    }

    #[test]
    fn the_page_offers_the_configured_shells_and_defaults_to_bash() {
        if gtk::init().is_err() {
            eprintln!("no display: skipping the GTK part of the Users test");
            return;
        }
        let page = UsersPage::new(&SysData::default());
        assert_eq!(page.shell_names, ["bash", "zsh", "fish"]);
        let (fields, _) = page.collect();
        assert_eq!(fields.user_shell, "bash");
        assert_eq!(fields.root_shell, "bash");
    }

    #[test]
    fn the_user_and_root_shells_are_chosen_separately() {
        if gtk::init().is_err() {
            eprintln!("no display: skipping the GTK part of the Users test");
            return;
        }
        let page = UsersPage::new(&SysData::default());
        page.user_shell_row.set_selected(1);
        page.root_shell_row.set_selected(2);
        let (fields, _) = page.collect();
        assert_eq!(fields.user_shell, "zsh");
        assert_eq!(fields.root_shell, "fish");
        page.user_shell_row.set_selected(0);
        let (fields, _) = page.collect();
        assert_eq!(fields.user_shell, "bash");
        assert_eq!(fields.root_shell, "fish", "root keeps its own choice");
    }
}
