//! The interface's icons, drawn as line art on a 24 by 24 grid, so they look the same on every
//! system (Windows' icon font exists only there). Each is turned into an SVG image in the colour
//! asked for, once, and kept.

use gpui::{Hsla, Image, ImageFormat};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write;
use std::sync::Arc;

fn gear() -> String {
    // Eight teeth around a ring.
    let mut d = String::new();
    let (inner, outer) = (7.2_f32, 9.6_f32);
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::FRAC_PI_4;
        let corners = [(a - 0.24, inner), (a - 0.16, outer), (a + 0.16, outer), (a + 0.24, inner), (a + 0.55, inner)];
        for (j, (b, r)) in corners.iter().enumerate() {
            let (x, y) = (12. + r * b.cos(), 12. + r * b.sin());
            let cmd = if i == 0 && j == 0 { 'M' } else { 'L' };
            let _ = write!(d, "{cmd}{x:.2} {y:.2}");
        }
    }
    d.push('Z');
    format!(r#"<path d="{d}"/><circle cx="12" cy="12" r="3"/>"#)
}

fn shapes(name: &str) -> String {
    let s = match name {
        "back" => r#"<path d="M19 12H5M11 6l-6 6 6 6"/>"#,
        "chevron-right" => r#"<path d="M9 6l6 6-6 6"/>"#,
        "chevron-down" => r#"<path d="M6 9l6 6 6-6"/>"#,
        "warning" => r#"<path d="M12 3.6 2.8 19.6h18.4Z"/><path d="M12 9.6v4.2M12 16.8v.1"/>"#,
        "delete" => r#"<path d="M4 7h16M9.5 7V4.5h5V7M6.5 7l1 13h9l1-13M10 11v6M14 11v6"/>"#,
        "clock" => r#"<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3.2 2"/>"#,
        "info" => r#"<circle cx="12" cy="12" r="9"/><path d="M12 11v5.6M12 7.6v.1"/>"#,
        "settings" => return gear(),
        "add" => r#"<path d="M12 5v14M5 12h14"/>"#,
        "light" => {
            r#"<path d="M9.5 17.5h5M10.5 20.5h3M12 3a6 6 0 0 0-3.6 10.8c.7.6 1.1 1.4 1.1 2.2h5c0-.8.4-1.6 1.1-2.2A6 6 0 0 0 12 3Z"/>"#
        }
        "buttons" => {
            r#"<path d="M7 8h10a5 5 0 0 1 5 5v1a3.5 3.5 0 0 1-6.3 2.1L14.8 15H9.2l-.9 1.1A3.5 3.5 0 0 1 2 14v-1a5 5 0 0 1 5-5Z"/><path d="M7.5 10.5v4M5.5 12.5h4"/><circle cx="16" cy="11.5" r=".6"/><circle cx="18" cy="13.5" r=".6"/>"#
        }
        "check" => r#"<path d="M5 12.5l4.5 4.5L19 7.5"/>"#,
        "bolt" => r#"<path d="M13.5 2.5 5 13.5h6.5l-1 8 8.5-11h-6.5Z"/>"#,
        "close" => r#"<path d="M6.5 6.5l11 11M17.5 6.5l-11 11"/>"#,
        "minimize" => r#"<path d="M6 12h12"/>"#,
        "maximize" => r#"<rect x="6" y="6" width="12" height="12" rx="1.5"/>"#,
        "restore" => r#"<rect x="5" y="9" width="10" height="10" rx="1.5"/><path d="M9 5h8.5A1.5 1.5 0 0 1 19 6.5V15"/>"#,
        "edit" => r#"<path d="M4 20h4L19 9l-4-4L4 16v4ZM13.5 6.5l4 4"/>"#,
        "open" => r#"<path d="M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5"/>"#,
        "motion" => {
            r#"<path d="M4.5 11a7.5 7.5 0 0 1 13.3-4.2M18 3.6v3.6h-3.6M19.5 13a7.5 7.5 0 0 1-13.3 4.2M6 20.4v-3.6h3.6"/><circle cx="12" cy="12" r="1.6"/>"#
        }
        "sticks" => r#"<circle cx="12" cy="8" r="4"/><path d="M12 12v5M5.5 17h13a2 2 0 0 1 0 4h-13a2 2 0 0 1 0-4Z"/>"#,
        _ => r#"<circle cx="12" cy="12" r="2"/>"#,
    };
    s.to_string()
}

fn svg(name: &str, color: Hsla) -> String {
    let c = color.to_rgb();
    let hex = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="48" height="48" fill="none" stroke="#{:02x}{:02x}{:02x}" stroke-opacity="{:.3}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{}</svg>"##,
        hex(c.r),
        hex(c.g),
        hex(c.b),
        c.a,
        shapes(name)
    )
}

thread_local! {
    static CACHE: RefCell<HashMap<(&'static str, u32), Arc<Image>>> = RefCell::new(HashMap::new());
}

/// The icon in that colour, as an image to draw at any size.
pub fn image(name: &'static str, color: Hsla) -> Arc<Image> {
    let c = color.to_rgb();
    let key = u32::from_be_bytes([c.r, c.g, c.b, c.a].map(|v| (v.clamp(0., 1.) * 255.).round() as u8));
    CACHE.with(|cache| {
        cache
            .borrow_mut()
            .entry((name, key))
            .or_insert_with(|| Arc::new(Image::from_bytes(ImageFormat::Svg, svg(name, color).into_bytes())))
            .clone()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_is_drawn() {
        for name in [
            "back",
            "chevron-right",
            "chevron-down",
            "warning",
            "delete",
            "clock",
            "info",
            "settings",
            "add",
            "light",
            "buttons",
            "check",
            "close",
            "minimize",
            "maximize",
            "restore",
            "edit",
            "open",
            "motion",
            "sticks",
        ] {
            assert!(!shapes(name).contains(r#"r="2"/>"#) || name == "settings", "{name} has no drawing");
        }
        assert!(svg("check", gpui::hsla(0., 0., 1., 1.)).contains("stroke=\"#ffffff\""));
    }
}
