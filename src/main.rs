mod backend;
mod ui;

use std::env;

use adw::prelude::*;
use gtk::gdk;
use gtk::glib;

use backend::system_detect;
use ui::SysData;

fn load_theme() {
    let css_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/style.css");
    let provider = gtk::CssProvider::new();
    provider.load_from_path(&css_path);
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
    let app = adw::Application::builder().application_id("org.voidlinux.Voyage").build();

    app.connect_activate(move |app| {
        gtk::Window::set_default_icon_name("voyage");
        load_theme();
        let sys_data = detect_system_data();
        let window = ui::window::build(app, sys_data, demo);
        window.present();
    });

    // We parse --demo ourselves above; run with no args so GApplication's
    // own option parser doesn't reject it as unknown.
    app.run_with_args::<&str>(&[])
}
