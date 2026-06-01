use super::*;

#[test]
fn wrap_prefixed_keeps_every_line_within_width() {
    // The guarantee holds once the width clears the prefix plus the longest
    // word — the Why panel's real operating range.  (Below that, greedy
    // wrap can't split a word and the render pass clips as a safety net.)
    let why = "Dividing the width by the hops spaces the number-line stops out evenly.";
    for width in [30u16, 40, 50, 60] {
        let lines = wrap_prefixed("  Why it matters: ", "    ", why, width);
        assert!(lines.len() > 1, "text should wrap onto multiple lines at width {}", width);
        for line in &lines {
            assert!(
                line.chars().count() <= width as usize,
                "line {:?} ({} cols) exceeds width {}",
                line,
                line.chars().count(),
                width
            );
        }
        assert!(lines[0].contains("Why it matters:"));
    }
}

#[test]
fn scroll_window_keeps_selection_visible() {
    // 15 items in a 10-row window: the selection is always inside it.
    for sel in 0..15usize {
        let (scroll, visible) = scroll_window(15, sel, 0, 10);
        assert_eq!(visible, 10);
        assert!(sel >= scroll && sel < scroll + visible, "sel {} not in [{}, {})", sel, scroll, scroll + visible);
        assert!(scroll + visible <= 15);
    }
    // Everything fits: no scrolling.
    assert_eq!(scroll_window(5, 4, 0, 10), (0, 5));
    // Degenerate window.
    assert_eq!(scroll_window(15, 3, 5, 5), (0, 0));
}

#[test]
fn wrap_chars_stays_within_width_and_preserves_content() {
    let s = "When I built a deck I added up every board length precisely.";
    for width in [10u16, 24, 40] {
        let lines = wrap_chars(s, width);
        for line in &lines {
            assert!(line.chars().count() <= width as usize);
        }
        // No characters are lost or invented.
        assert_eq!(lines.concat(), s);
    }
    assert_eq!(wrap_chars("", 8), vec![String::new()]);
}

#[test]
fn wrap_text_handles_blank_and_single_word() {
    assert_eq!(wrap_text("", 10), vec![String::new()]);
    // A word longer than the width still survives as its own line.
    let lines = wrap_text("supercalifragilistic", 8);
    assert_eq!(lines.len(), 1);
}
