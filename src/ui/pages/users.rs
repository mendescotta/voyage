use gtk::prelude::*;

use crate::ui::widgets::{EntryRow, Group, SwitchRow};
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
        widget.append(user_group.as_ref());

        let root_group = Group::new("Root account");
        let same_password_row = SwitchRow::new("Use my account password for root");
        same_password_row.set_subtitle("Skip setting a separate root password.");
        let rootpassword_row = EntryRow::password("Root password");
        let rootpassword_confirm_row = EntryRow::password("Confirm root password");
        root_group.add(&same_password_row);
        root_group.add(&rootpassword_row);
        root_group.add(&rootpassword_confirm_row);
        widget.append(root_group.as_ref());

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
        };
        (fields, errors)
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
}
