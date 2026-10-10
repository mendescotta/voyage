pub struct CompletionView {
    pub heading: &'static str,
    pub css_class: &'static str,
}

pub fn completion_view(success: bool) -> CompletionView {
    if success {
        CompletionView {
            heading: "Installation complete",
            css_class: "success",
        }
    } else {
        CompletionView {
            heading: "Installation failed",
            css_class: "error",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_view_distinguishes_success_and_failure() {
        let ok = completion_view(true);
        let fail = completion_view(false);
        assert_ne!(ok.css_class, fail.css_class);
        assert_ne!(ok.heading, fail.heading);
    }

    #[test]
    fn completion_view_failure_uses_error_styling() {
        assert_eq!(completion_view(false).css_class, "error");
    }
}
