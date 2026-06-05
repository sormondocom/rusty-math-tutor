use super::*;

use tiny_skia::Pixmap;

#[test]
fn text_renders_pixels() {
    // Font-agnostic smoke test (TTF or 8×8 fallback): drawing text must light
    // *some* pixels, and `text_width` must grow with the string length.  `text`
    // scales coords by SS internally, so the pixmap is sized generously.
    let mut pm = Pixmap::new(400, 120).unwrap();
    text(&mut pm, 6.0, 6.0, 2.0, "Hi", WHITE);
    let any_lit = pm.pixels().iter().any(|p| p.red() > 30);
    assert!(any_lit, "text should light at least one pixel");
    assert!(text_width("WWWW", 2.0) > text_width("W", 2.0), "wider strings measure wider");
}

#[test]
fn fit_scale_is_bounded() {
    // Never returns above the cap, nor a useless sub-pixel scale.
    let s = fit_scale("a very long label that must shrink", 100.0, 6.0);
    assert!(s <= 6.0 && s >= 0.8, "fit_scale stays within [0.8, max]");
}
