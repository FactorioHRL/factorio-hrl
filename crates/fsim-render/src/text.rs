//! Text with a real font (DejaVu Sans, embedded in the binary), rasterised by `fontdue`.

use std::sync::OnceLock;

use fontdue::{Font, FontSettings};
use tiny_skia::Pixmap;

use crate::palette::Rgb;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Weight {
    Regular,
    Bold,
}

struct Fonts {
    regular: Font,
    bold: Font,
}

fn fonts() -> &'static Fonts {
    static FONTS: OnceLock<Fonts> = OnceLock::new();
    FONTS.get_or_init(|| {
        let load = |bytes: &[u8]| {
            Font::from_bytes(bytes, FontSettings::default()).expect("embedded font is valid")
        };
        Fonts {
            regular: load(include_bytes!("../assets/fonts/DejaVuSans.ttf")),
            bold: load(include_bytes!("../assets/fonts/DejaVuSans-Bold.ttf")),
        }
    })
}

fn font(weight: Weight) -> &'static Font {
    match weight {
        Weight::Regular => &fonts().regular,
        Weight::Bold => &fonts().bold,
    }
}

/// Text width in pixels at font height `px`.
pub(crate) fn width(text: &str, px: f32, weight: Weight) -> f32 {
    let font = font(weight);
    text.chars()
        .map(|c| font.metrics(c, px).advance_width)
        .sum()
}

/// Distance from the top of the line to the baseline.
fn ascent(px: f32, weight: Weight) -> f32 {
    font(weight)
        .horizontal_line_metrics(px)
        .map_or(px * 0.8, |m| m.ascent)
}

fn blend(canvas: &mut Pixmap, x: i32, y: i32, color: Rgb, coverage: u8) {
    if coverage == 0 || x < 0 || y < 0 || x >= canvas.width() as i32 || y >= canvas.height() as i32
    {
        return;
    }
    let index = y as usize * canvas.width() as usize + x as usize;
    let pixel = canvas.pixels_mut()[index];
    let alpha = u32::from(coverage);
    let mix =
        |src: u8, dst: u8| ((u32::from(src) * alpha + u32::from(dst) * (255 - alpha)) / 255) as u8;
    // the background is opaque, so premultiplied and straight channels coincide
    canvas.pixels_mut()[index] = tiny_skia::PremultipliedColorU8::from_rgba(
        mix(color.0, pixel.red()),
        mix(color.1, pixel.green()),
        mix(color.2, pixel.blue()),
        255,
    )
    .unwrap_or(pixel);
}

fn draw_run(
    canvas: &mut Pixmap,
    text: &str,
    left: f32,
    top: f32,
    px: f32,
    weight: Weight,
    color: Rgb,
) {
    let font = font(weight);
    let baseline = top + ascent(px, weight);
    let mut pen = left;
    for c in text.chars() {
        let (metrics, bitmap) = font.rasterize(c, px);
        let glyph_left = (pen + metrics.xmin as f32).round() as i32;
        let glyph_top = (baseline - metrics.ymin as f32 - metrics.height as f32).round() as i32;
        for row in 0..metrics.height {
            for col in 0..metrics.width {
                blend(
                    canvas,
                    glyph_left + col as i32,
                    glyph_top + row as i32,
                    color,
                    bitmap[row * metrics.width + col],
                );
            }
        }
        pen += metrics.advance_width;
    }
}

/// Draws text with the line's top left corner at `(left, top)` and a subtle dark shadow,
/// so it is readable on any background.
pub(crate) fn draw(
    canvas: &mut Pixmap,
    text: &str,
    left: f32,
    top: f32,
    px: f32,
    weight: Weight,
    color: Rgb,
) {
    let shadow = (px / 12.0).max(1.0);
    draw_run(
        canvas,
        text,
        left + shadow,
        top + shadow,
        px,
        weight,
        (0, 0, 0),
    );
    draw_run(canvas, text, left, top, px, weight, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_grows_with_text_and_size() {
        let w = |t, px| width(t, px, Weight::Regular);
        assert_eq!(w("", 16.0), 0.0);
        assert!(w("iron plate", 16.0) > w("iron", 16.0));
        assert!(w("iron", 32.0) > w("iron", 16.0) * 1.9);
        assert!(width("iron", 16.0, Weight::Bold) > w("iron", 16.0));
    }

    #[test]
    fn drawing_changes_pixels_only_near_the_text() {
        let mut canvas = Pixmap::new(80, 30).unwrap();
        canvas.fill(tiny_skia::Color::from_rgba8(0, 0, 0, 255));
        draw(
            &mut canvas,
            "Input:",
            4.0,
            4.0,
            16.0,
            Weight::Bold,
            (255, 255, 255),
        );
        let lit = canvas.pixels().iter().filter(|p| p.red() > 128).count();
        assert!(lit > 30, "text should light up pixels, got {lit}");
        assert_eq!(canvas.pixel(79, 29).unwrap().red(), 0);
    }

    #[test]
    fn drawing_outside_the_canvas_is_harmless() {
        let mut canvas = Pixmap::new(10, 10).unwrap();
        draw(
            &mut canvas,
            "wide text",
            -20.0,
            -5.0,
            16.0,
            Weight::Regular,
            (255, 255, 255),
        );
        draw(
            &mut canvas,
            "wide text",
            5.0,
            8.0,
            16.0,
            Weight::Regular,
            (255, 255, 255),
        );
    }
}
