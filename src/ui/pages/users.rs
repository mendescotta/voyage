use adw::prelude::*;

use crate::ui::SysData;

pub const TITLE: &str = "Users";

pub struct UsersPage {
    pub widget: gtk::Box,
    hostname_row: adw::EntryRow,
    fullname_row: adw::EntryRow,
    userlogin_row: adw::EntryRow,
    userpassword_row: adw::PasswordEntryRow,
    userpassword_confirm_row: adw::PasswordEntryRow,
    same_password_row: adw::SwitchRow,
    rootpassword_row: adw::PasswordEntryRow,
    rootpassword_confirm_row: adw::PasswordEntryRow,
    autologin_row: adw::SwitchRow,
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

        let system_group = adw::PreferencesGroup::builder().title("Computer").build();
        let hostname_row = adw::EntryRow::builder().title("Computer name").text("void").build();
        system_group.add(&hostname_row);
        widget.append(&system_group);

        let user_group = adw::PreferencesGroup::builder().title("Your account").build();
        let fullname_row = adw::EntryRow::builder().title("Full name").text("void").build();
        let userlogin_row = adw::EntryRow::builder().title("Username").text("void").build();
        let userpassword_row = adw::PasswordEntryRow::builder().title("Password").build();
        let userpassword_confirm_row = adw::PasswordEntryRow::builder().title("Confirm password").build();
        user_group.add(&fullname_row);
        user_group.add(&userlogin_row);
        user_group.add(&userpassword_row);
        user_group.add(&userpassword_confirm_row);
        widget.append(&user_group);

        let root_group = adw::PreferencesGroup::builder().title("Root account").build();
        let same_password_row = adw::SwitchRow::builder()
            .title("Use my account password for root")
            .subtitle("Skip setting a separate root password.")
            .build();
        let rootpassword_row = adw::PasswordEntryRow::builder().title("Root password").build();
        let rootpassword_confirm_row = adw::PasswordEntryRow::builder().title("Confirm root password").build();
        root_group.add(&same_password_row);
        root_group.add(&rootpassword_row);
        root_group.add(&rootpassword_confirm_row);
        widget.append(&root_group);

        {
            let rootpassword_row = rootpassword_row.clone();
            let rootpassword_confirm_row = rootpassword_confirm_row.clone();
            same_password_row.connect_notify_local(Some("active"), move |row, _| {
                let same_password = row.is_active();
                for entry in [&rootpassword_row, &rootpassword_confirm_row] {
                    entry.set_sensitive(!same_password);
                    entry.set_text("");
                }
            });
        }

        let autologin_row = adw::SwitchRow::builder().title("Log in automatically").build();
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
            errors.push(("userpassword".to_string(), "User passwords do not match.".to_string()));
        }
        if !same_password && rootpassword != self.rootpassword_confirm_row.text() {
            errors.push(("rootpassword".to_string(), "Root passwords do not match.".to_string()));
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

fn configure_autologin(row: &adw::SwitchRow, display_manager: Option<&str>) {
    let manager = display_manager.unwrap_or("").to_lowercase();
    match manager.as_str() {
        "sddm" | "lightdm" | "gdm" => {
            row.set_sensitive(true);
            row.set_visible(true);
            row.set_subtitle(&format!("Available via {display_manager}.", display_manager = display_manager.unwrap_or("")));
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
