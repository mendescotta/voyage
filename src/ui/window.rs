use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;

use crate::backend::config_schema::{build_config, InstallFields};
use crate::ui::nav::*;
use crate::ui::pages::completion::CompletionPage;
use crate::ui::pages::disks::DisksPage;
use crate::ui::pages::installation::InstallationPage;
use crate::ui::pages::mirrors::MirrorsPage;
use crate::ui::pages::review::ReviewPage;
use crate::ui::pages::users::UsersPage;
use crate::ui::pages::welcome::WelcomePage;
use crate::ui::widgets::{self, Window};
use crate::ui::{pages, SysData};

struct State {
    sys_data: SysData,
    demo: bool,
    welcome: WelcomePage,
    mirrors: MirrorsPage,
    users: UsersPage,
    disks: DisksPage,
    review: ReviewPage,
    installation: InstallationPage,
    completion: CompletionPage,
    stack: gtk::Stack,
    tabs: Vec<gtk::ToggleButton>,
    tab_bar: gtk::Box,
    back_button: gtk::Button,
    next_button: gtk::Button,
    current_index: usize,
    furthest: usize,
    window: Window,
}

/// Setup pages can be taller than the window, so each one scrolls vertically,
/// with a scrollbar that stays visible (overlay scrollbars hide until hovered).
fn scrollable(page: &gtk::Box) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .overlay_scrolling(false)
        .vexpand(true)
        .child(page)
        .build()
}

pub fn build(app: &widgets::App, sys_data: SysData, demo: bool) -> Window {
    let window = widgets::new_window(app, "Voyage", 880, 620);

    let welcome = WelcomePage::new(&sys_data);
    let mirrors = MirrorsPage::new(&sys_data);
    let users = UsersPage::new(&sys_data);
    let disks = DisksPage::new(&sys_data);
    let review = ReviewPage::new();
    let installation = InstallationPage::new();
    let completion = CompletionPage::new();

    let stack = gtk::Stack::new();
    stack.add_titled(
        &scrollable(&welcome.widget),
        Some(pages::welcome::TITLE),
        pages::welcome::TITLE,
    );
    stack.add_titled(
        &scrollable(&mirrors.widget),
        Some(pages::mirrors::TITLE),
        pages::mirrors::TITLE,
    );
    stack.add_titled(
        &scrollable(&users.widget),
        Some(pages::users::TITLE),
        pages::users::TITLE,
    );
    stack.add_titled(
        &scrollable(&disks.widget),
        Some(pages::disks::TITLE),
        pages::disks::TITLE,
    );
    stack.add_titled(
        &scrollable(&review.widget),
        Some(pages::review::TITLE),
        pages::review::TITLE,
    );
    stack.add_titled(
        &installation.widget,
        Some(pages::installation::TITLE),
        pages::installation::TITLE,
    );
    stack.add_titled(
        &completion.widget,
        Some(pages::completion::TITLE),
        pages::completion::TITLE,
    );

    let tab_titles = [
        pages::welcome::TITLE,
        pages::mirrors::TITLE,
        pages::users::TITLE,
        pages::disks::TITLE,
        pages::review::TITLE,
    ];
    let tab_bar = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .css_classes(["voyage-tabs"])
        .build();
    let mut tabs: Vec<gtk::ToggleButton> = Vec::new();
    for title in tab_titles {
        let tab = gtk::ToggleButton::builder()
            .label(title)
            .has_frame(false)
            .hexpand(true)
            .build();
        if let Some(first) = tabs.first() {
            tab.set_group(Some(first));
        }
        tab_bar.append(&tab);
        tabs.push(tab);
    }

    let header_bar = gtk::HeaderBar::new();
    let about_button = gtk::Button::builder()
        .icon_name("help-about-symbolic")
        .tooltip_text("About")
        .build();
    header_bar.pack_end(&about_button);

    let panel = gtk::Frame::builder()
        .css_classes(["voyage-panel"])
        .vexpand(true)
        .child(&stack)
        .build();

    let back_button = gtk::Button::builder().label("Back").build();
    let next_button = gtk::Button::builder()
        .label("Next")
        .css_classes(["suggested-action"])
        .build();

    let bottom_bar = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .margin_top(6)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    bottom_bar.append(&back_button);
    bottom_bar.append(&gtk::Box::builder().hexpand(true).build());
    bottom_bar.append(&next_button);

    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    body.append(&tab_bar);
    body.append(&panel);
    body.append(&bottom_bar);
    widgets::set_window_content(&window, &header_bar, body.upcast_ref());

    let state = Rc::new(RefCell::new(State {
        sys_data,
        demo,
        welcome,
        mirrors,
        users,
        disks,
        review,
        installation,
        completion,
        stack,
        tabs,
        tab_bar,
        back_button,
        next_button,
        current_index: 0,
        furthest: 0,
        window: window.clone(),
    }));

    update_nav(&state);

    for index in 0..TAB_COUNT {
        let tab = state.borrow().tabs[index].clone();
        let state = state.clone();
        tab.connect_clicked(move |_| {
            let allowed = {
                let s = state.borrow();
                s.current_index != index && tab_enabled(index, s.current_index, s.furthest)
            };
            if !allowed {
                update_nav(&state);
            } else if index == REVIEW {
                go_review(&state);
            } else {
                state.borrow_mut().current_index = index;
                update_nav(&state);
            }
        });
    }

    {
        let back_button = state.borrow().back_button.clone();
        let state = state.clone();
        back_button.connect_clicked(move |_| {
            let mut s = state.borrow_mut();
            if s.current_index > 0 {
                s.current_index -= 1;
            }
            drop(s);
            update_nav(&state);
        });
    }

    {
        let next_button = state.borrow().next_button.clone();
        let state = state.clone();
        next_button.connect_clicked(move |_| on_next(&state));
    }

    {
        let state = state.clone();
        about_button.connect_clicked(move |_| show_about(&state));
    }

    {
        let state = state.clone();
        let install_button = state.borrow().review.install_button.clone();
        install_button.connect_clicked(move |_| start_install(&state));
    }

    if let Ok(dir) = std::env::var("VOYAGE_SCREENSHOTS") {
        screenshot_tour(&state, dir.into());
    }

    window
}

/// Developer aid: with `VOYAGE_SCREENSHOTS=<dir>` the window visits each setup tab, saves a PNG
/// of it to `<dir>/<n>-<title>.png`, then quits. Used to review the UI without a screen grabber.
fn screenshot_tour(state: &Rc<RefCell<State>>, dir: std::path::PathBuf) {
    let titles = [
        pages::welcome::TITLE,
        pages::mirrors::TITLE,
        pages::users::TITLE,
        pages::disks::TITLE,
    ];
    let _ = std::fs::create_dir_all(&dir);
    let state = state.clone();
    let step = std::cell::Cell::new(0usize);
    gtk::glib::timeout_add_local(std::time::Duration::from_millis(1200), move || {
        let n = step.get();
        if n > 0 {
            let s = state.borrow();
            let paintable = gtk::WidgetPaintable::new(Some(&s.window));
            let snapshot = gtk::Snapshot::new();
            paintable.snapshot(
                &snapshot,
                f64::from(s.window.width()),
                f64::from(s.window.height()),
            );
            if let (Some(node), Some(renderer)) = (
                snapshot.to_node(),
                s.window.native().and_then(|n| n.renderer()),
            ) {
                let name = format!(
                    "{}-{}.png",
                    n - 1,
                    titles[n - 1].replace(' ', "-").to_lowercase()
                );
                let _ = renderer
                    .render_texture(&node, None)
                    .save_to_png(dir.join(name));
            }
        }
        if n >= titles.len() {
            if let Some(app) = state.borrow().window.application() {
                app.quit();
            }
            return gtk::glib::ControlFlow::Break;
        }
        state.borrow_mut().current_index = n;
        state.borrow_mut().furthest = REVIEW;
        update_nav(&state);
        step.set(n + 1);
        gtk::glib::ControlFlow::Continue
    });
}

fn update_nav(state: &Rc<RefCell<State>>) {
    {
        let mut s = state.borrow_mut();
        s.furthest = advance_furthest(s.furthest, s.current_index);
    }
    let s = state.borrow();
    let page = s.current_index;
    let visible_title = match page {
        WELCOME => pages::welcome::TITLE,
        MIRRORS => pages::mirrors::TITLE,
        USERS => pages::users::TITLE,
        DISKS => pages::disks::TITLE,
        REVIEW => pages::review::TITLE,
        INSTALLATION => pages::installation::TITLE,
        _ => pages::completion::TITLE,
    };
    s.stack.set_visible_child_name(visible_title);

    let is_install_step = page == INSTALLATION;
    let is_review_step = page == REVIEW;
    s.back_button
        .set_sensitive(page > 0 && page < PAGE_COUNT - 1 && !is_install_step);

    let is_last = page == PAGE_COUNT - 1;
    s.next_button
        .set_visible(!is_install_step && !is_review_step);
    s.next_button
        .set_label(if is_last { "Restart" } else { "Next" });

    s.tab_bar.set_visible(tabs_visible(page));
    for (i, tab) in s.tabs.iter().enumerate() {
        tab.set_sensitive(tab_enabled(i, page, s.furthest));
        if tab.is_active() != (i == page) {
            tab.set_active(i == page);
        }
    }
}

fn on_next(state: &Rc<RefCell<State>>) {
    let current = state.borrow().current_index;

    if current == DISKS {
        go_review(state);
        return;
    }
    if current == COMPLETION {
        let (demo, app) = {
            let s = state.borrow();
            (s.demo, s.window.application())
        };
        if demo {
            if let Some(app) = app {
                app.quit();
            }
        } else {
            let _ = std::process::Command::new("pkexec").arg("reboot").spawn();
        }
        return;
    }

    let errors = match current {
        WELCOME => state.borrow().welcome.collect().1,
        MIRRORS => state.borrow().mirrors.collect().1,
        USERS => state.borrow().users.collect().1,
        _ => Vec::new(),
    };
    if !errors.is_empty() {
        show_errors(state, &errors);
        return;
    }

    state.borrow_mut().current_index += 1;
    update_nav(state);
}

/// Validate every page and refresh the summary before showing Review, so the summary is never stale.
fn go_review(state: &Rc<RefCell<State>>) {
    match collect_all(state) {
        Ok(config) => {
            state.borrow().review.set_config(&config);
            state.borrow_mut().current_index = REVIEW;
        }
        Err(errors) => show_errors(state, &errors),
    }
    update_nav(state);
}

fn collect_all(
    state: &Rc<RefCell<State>>,
) -> Result<crate::backend::config_schema::InstallConfig, Vec<(String, String)>> {
    let s = state.borrow();
    let (welcome_fields, mut errors) = s.welcome.collect();
    let (mirrors_fields, mirrors_errors) = s.mirrors.collect();
    let (users_fields, users_errors) = s.users.collect();
    errors.extend(mirrors_errors);
    errors.extend(users_errors);

    let (disk_choices, disks_errors) = s.disks.collect();
    errors.extend(disks_errors);
    if !errors.is_empty() {
        return Err(errors);
    }

    let fields = InstallFields {
        locale: welcome_fields.locale,
        timezone_region: welcome_fields.timezone_region,
        timezone_city: welcome_fields.timezone_city,
        keymap: welcome_fields.keymap,
        hostname: users_fields.hostname,
        userlogin: users_fields.userlogin,
        username: users_fields.username,
        userpassword: users_fields.userpassword,
        rootpassword: users_fields.rootpassword,
        autologin: users_fields.autologin,
        mirror: mirrors_fields.mirror,
        net: mirrors_fields.net,
        nonfree: mirrors_fields.nonfree,
        hw_drivers: mirrors_fields.hw_drivers,
        driver_set: mirrors_fields.driver_set,
    };

    build_config(
        &fields,
        &disk_choices,
        s.sys_data.display_manager.as_deref().unwrap_or(""),
    )
}

fn start_install(state: &Rc<RefCell<State>>) {
    let data = match collect_all(state) {
        Ok(data) => data,
        Err(errors) => {
            show_errors(state, &errors);
            return;
        }
    };

    {
        let mut s = state.borrow_mut();
        s.current_index = INSTALLATION;
    }
    update_nav(state);

    let demo = state.borrow().demo;
    let state_for_finish = state.clone();
    state
        .borrow()
        .installation
        .start(data, demo, move |success, message| {
            {
                let s = state_for_finish.borrow();
                s.completion.set_result(success, &message);
            }
            state_for_finish.borrow_mut().current_index = COMPLETION;
            update_nav(&state_for_finish);
        });
}

fn show_errors(state: &Rc<RefCell<State>>, errors: &[(String, String)]) {
    let body = errors
        .iter()
        .map(|(_, message)| message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let s = state.borrow();
    widgets::alert(&s.window, "Please check your input", &body);
}

fn show_about(state: &Rc<RefCell<State>>) {
    let s = state.borrow();
    widgets::about(&s.window);
}
