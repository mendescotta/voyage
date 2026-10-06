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

/// A tab can be picked once its page has been reached; later ones stay locked so
/// validation on the pages in between cannot be skipped.
pub fn tab_enabled(tab: usize, current: usize, furthest: usize) -> bool {
    tabs_visible(current) && tab <= furthest
}

/// Track the furthest setup page reached (Review is the last tab).
pub fn advance_furthest(furthest: usize, current: usize) -> usize {
    furthest.max(current.min(REVIEW))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_locked_beyond_furthest() {
        assert!(tab_enabled(WELCOME, WELCOME, WELCOME));
        assert!(!tab_enabled(USERS, WELCOME, WELCOME));
        assert!(tab_enabled(USERS, WELCOME, USERS));
        assert!(tab_enabled(WELCOME, USERS, USERS));
    }

    #[test]
    fn tabs_inert_during_install() {
        assert!(!tabs_visible(INSTALLATION));
        assert!(!tabs_visible(COMPLETION));
        assert!(tabs_visible(REVIEW));
        assert!(!tab_enabled(WELCOME, INSTALLATION, REVIEW));
    }

    #[test]
    fn furthest_only_moves_forward_and_stops_at_review() {
        assert_eq!(advance_furthest(WELCOME, USERS), USERS);
        assert_eq!(advance_furthest(DISKS, WELCOME), DISKS);
        assert_eq!(advance_furthest(REVIEW, INSTALLATION), REVIEW);
    }
}
