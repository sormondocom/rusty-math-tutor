//! Static image assets — decoded once at first use and cached for the
//! lifetime of the process.

use std::sync::OnceLock;

use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, PremultipliedColorU8, Transform};

use super::*;

static DUCK_COLOR: OnceLock<Option<Pixmap>> = OnceLock::new();
static DUCK_CHALK: OnceLock<Option<Pixmap>> = OnceLock::new();

// ---------------------------------------------------------------------------
// Public accessors
// ---------------------------------------------------------------------------

/// Full-colour mascot, decoded from the embedded PNG bytes.
pub fn duck_png() -> Option<&'static Pixmap> {
    DUCK_COLOR
        .get_or_init(|| {
            let bytes = include_bytes!("../../../assets/duck_with_tie_and_hat.png");
            Pixmap::decode_png(bytes).ok()
        })
        .as_ref()
}

/// Chalk-style variant: RGB of every opaque pixel run through [`chalkify`].
/// Computed once on first entry to a chalk theme and then cached.
pub fn chalk_duck_png() -> Option<&'static Pixmap> {
    DUCK_CHALK
        .get_or_init(|| chalkify_pixmap(duck_png()?))
        .as_ref()
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

/// Blit the duck centred on logical `(cx, y)`, scaled so the image is
/// `target_h` logical px tall.  Width follows the image's native aspect ratio
/// so the duck is never stretched.  Chalk mode uses the chalk variant.
pub fn draw_duck_png(pm: &mut Pixmap, cx: f32, y: f32, target_h: f32) {
    let sprite = if is_chalk() { chalk_duck_png() } else { duck_png() };
    let Some(duck) = sprite else { return };

    let scale     = (target_h * SSF) / duck.height() as f32;
    let display_w = duck.width() as f32 * scale / SSF;
    let x         = cx - display_w / 2.0;

    let paint = PixmapPaint { quality: FilterQuality::Bilinear, ..PixmapPaint::default() };
    pm.draw_pixmap(
        (x * SSF) as i32,
        (y * SSF) as i32,
        duck.as_ref(),
        &paint,
        Transform::from_scale(scale, scale),
        None,
    );
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn chalkify_pixmap(src: &Pixmap) -> Option<Pixmap> {
    let mut pm = Pixmap::new(src.width(), src.height())?;
    for (sp, dp) in src.pixels().iter().zip(pm.pixels_mut().iter_mut()) {
        let a = sp.alpha();
        if a == 0 { continue; }
        let r = (sp.red()   as u32 * 255 / a as u32).min(255) as u8;
        let g = (sp.green() as u32 * 255 / a as u32).min(255) as u8;
        let b = (sp.blue()  as u32 * 255 / a as u32).min(255) as u8;
        let c = chalkify([r, g, b]);
        let prem = |ch: u8| (ch as u32 * a as u32 / 255) as u8;
        if let Some(px) = PremultipliedColorU8::from_rgba(prem(c[0]), prem(c[1]), prem(c[2]), a) {
            *dp = px;
        }
    }
    Some(pm)
}
