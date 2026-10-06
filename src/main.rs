mod backend;
mod ui;

use std::env;

use gtk::prelude::*;
use gtk::gdk;
use gtk::glib;

use backend::system_detect;
use ui::SysData;

fn load_theme() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(ui::STYLE_CSS);
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }
}

fn detect_system_data() -> SysData {
    SysData {
        efi: system_detect::detect_efi(),
        net: system_detect::has_internet(std::time::Duration::from_secs(3)),
        display_manager: system_detect::detect_display_manager(),
    }
}

fn main() -> glib::ExitCode {
    let demo = env::args().any(|a| a == "--demo");

    glib::set_prgname(Some("voyage"));
    ui::widgets::init();
    let app = ui::widgets::new_app("org.voidlinux.Voyage");

    app.connect_activate(move |app| {
        gtk::Window::set_default_icon_name("voyage");
        load_theme();
        let sys_data = detect_system_data();
        let window = ui::window::build(app, sys_data, demo);
        window.present();
    });

    app.run_with_args::<&str>(&[])
}
