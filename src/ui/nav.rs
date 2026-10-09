pub const PAGE_COUNT: usize = 7;
pub const WELCOME: usize = 0;
pub const MIRRORS: usize = 1;
pub const USERS: usize = 2;
pub const DISKS: usize = 3;
pub const REVIEW: usize = 4;
pub const INSTALLATION: usize = 5;
pub const COMPLETION: usize = 6;
/// Welcome, Mirrors, Users, Disks, Review.
pub const TAB_COUNT: usize = 5;

/// The tab row is only shown while the user is still setting things up.
pub fn tabs_visible(current: usize) -> bool {
    current < INSTALLATION
}

/// Cancel policy: once the installation is running (partitioning, formatting, copying as root) nothing can
/// interrupt it safely, so the window cannot be closed until it ends in success or failure. Before that the
/// user can go anywhere and close freely. `--demo` touches no disk, so it may always be closed.
pub fn close_allowed(current: usize, demo: bool) -> bool {
    demo || current != INSTALLATION
}

/// Any setup tab can be picked at any time. Nothing is skipped by jumping around: Review and the install
/// button validate every page again before they let anything happen.
pub fn tab_enabled(_tab: usize, current: usize) -> bool {
    tabs_visible(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_setup_tab_is_available_from_the_start() {
        for tab in 0..TAB_COUNT {
            assert!(tab_enabled(tab, WELCOME), "tab {tab} from Welcome");
            assert!(tab_enabled(tab, DISKS), "tab {tab} from Disks");
            assert!(tab_enabled(tab, REVIEW), "tab {tab} from Review");
        }
    }

    #[test]
    fn tabs_inert_during_install() {
        assert!(!tabs_visible(INSTALLATION));
        assert!(!tabs_visible(COMPLETION));
        assert!(tabs_visible(REVIEW));
        assert!(!tab_enabled(WELCOME, INSTALLATION));
        assert!(!tab_enabled(REVIEW, COMPLETION));
    }

    #[test]
    fn the_window_cannot_be_closed_while_installing() {
        assert!(!close_allowed(INSTALLATION, false));
    }

    #[test]
    fn it_can_be_closed_before_the_install_and_after_it_ends() {
        for page in [WELCOME, MIRRORS, USERS, DISKS, REVIEW, COMPLETION] {
            assert!(close_allowed(page, false), "page {page}");
        }
    }

    #[test]
    fn the_demo_never_blocks_closing() {
        assert!(close_allowed(INSTALLATION, true));
    }
}
