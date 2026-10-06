//! Colours, type, corners and icons. A dark or light neutral base that follows Windows, one
//! accent taken from Windows, and colour for state only where something needs attention. Cards
//! sit a step above the page and controls a step above the cards, with hairline edges instead
//! of heavy shadows.

use gpui::{Hsla, Rgba, WindowAppearance, rgb, rgba};

pub const FONT: &str = "Segoe UI Variable Text";
pub const DISPLAY_FONT: &str = "Segoe UI Variable Display";

/// Type ramp, as (size, line height) in pixels.
pub mod text {
    pub const CAPTION: (f32, f32) = (12., 16.);
    pub const BODY: (f32, f32) = (14., 20.);
    pub const TITLE: (f32, f32) = (16., 22.);
    pub const DISPLAY: (f32, f32) = (28., 36.);
}

/// How wide the content may grow, and the gutter around it. The bar on top lines up with it.
pub mod layout {
    pub const MAX_W: f32 = 1180.;
    pub const SETTINGS_W: f32 = 820.;
    pub const GUTTER: f32 = 28.;
}

/// Corner radii: the larger the surface, the rounder.
pub mod radius {
    pub const TILE: f32 = 16.;
    pub const CARD: f32 = 12.;
    pub const CONTROL: f32 = 8.;
    pub const CHIP: f32 = 6.;
}

/// The icons, by name (see `crate::icons`).
pub mod icon {
    pub const BACK: &str = "back";
    pub const CHEVRON_RIGHT: &str = "chevron-right";
    pub const CHEVRON_DOWN: &str = "chevron-down";
    pub const WARNING: &str = "warning";
    pub const DELETE: &str = "delete";
    pub const CLOCK: &str = "clock";
    pub const INFO: &str = "info";
    pub const SETTINGS: &str = "settings";
    pub const ADD: &str = "add";
    pub const LIGHT: &str = "light";
    pub const BUTTONS: &str = "buttons";
    pub const CHECK: &str = "check";
    pub const BOLT: &str = "bolt";
    pub const CLOSE: &str = "close";
    pub const EDIT: &str = "edit";
    pub const OPEN: &str = "open";
    pub const MOTION: &str = "motion";
    pub const STICKS: &str = "sticks";
}

#[derive(Clone, Copy)]
pub struct Theme {
    /// The page.
    pub base: Hsla,
    /// A card or tile.
    pub layer: Hsla,
    pub layer_hover: Hsla,
    /// A control on a card: button, keycap, field.
    pub control: Hsla,
    pub control_hover: Hsla,
    pub stroke: Hsla,
    pub stroke_strong: Hsla,
    pub text: Hsla,
    pub text2: Hsla,
    pub text3: Hsla,
    pub accent: Hsla,
    pub accent_soft: Hsla,
    /// Text on an accent fill.
    pub on_accent: Hsla,
    pub critical: Hsla,
    pub success: Hsla,
    pub caution: Hsla,
    pub caution_layer: Hsla,
    /// The drawn controller: its body, lighter at the top, and the wells its controls sit in.
    pub art_body_top: Hsla,
    pub art_body_bottom: Hsla,
    pub art_well: Hsla,
    /// Behind a drawn controller: a trace of the accent fading into the card.
    pub stage_top: Hsla,
    pub stage_bottom: Hsla,
}

fn c(v: u32) -> Hsla {
    rgb(v).into()
}

fn ca(v: u32) -> Hsla {
    rgba(v).into()
}

impl Theme {
    pub fn new(appearance: WindowAppearance) -> Theme {
        let dark = matches!(appearance, WindowAppearance::Dark | WindowAppearance::VibrantDark);
        let accent = windows_accent(dark).unwrap_or_else(|| if dark { rgb(0x60cdff) } else { rgb(0x005fb8) });
        let accent: Hsla = accent.into();
        if dark {
            let layer = c(0x1a1b1f);
            Theme {
                base: c(0x111214),
                layer,
                layer_hover: c(0x202227),
                control: c(0x26282d),
                control_hover: c(0x2e3036),
                stroke: ca(0xffffff12),
                stroke_strong: ca(0xffffff24),
                text: c(0xf3f4f6),
                text2: ca(0xffffffb8),
                text3: ca(0xffffff7a),
                accent,
                accent_soft: accent.opacity(0.16),
                on_accent: c(0x000000),
                critical: c(0xff99a4),
                success: c(0x6ccb5f),
                caution: c(0xfce100),
                caution_layer: c(0x3a3220),
                art_body_top: c(0x3d4048),
                art_body_bottom: c(0x2a2c32),
                art_well: c(0x16171a),
                stage_top: accent.opacity(0.10),
                stage_bottom: layer.opacity(0.),
            }
        } else {
            let layer = c(0xffffff);
            Theme {
                base: c(0xf2f3f5),
                layer,
                layer_hover: c(0xf8f9fa),
                control: c(0xf1f2f4),
                control_hover: c(0xe8e9ec),
                stroke: ca(0x00000014),
                stroke_strong: ca(0x00000026),
                text: ca(0x000000e4),
                text2: ca(0x0000009e),
                text3: ca(0x00000072),
                accent,
                accent_soft: accent.opacity(0.12),
                on_accent: c(0xffffff),
                critical: c(0xc42b1c),
                success: c(0x0f7b0f),
                caution: c(0x9d5d00),
                caution_layer: c(0xfff4ce),
                art_body_top: c(0xfdfdfe),
                art_body_bottom: c(0xe4e6ea),
                art_well: c(0xd6d8dd),
                stage_top: accent.opacity(0.08),
                stage_bottom: layer.opacity(0.),
            }
        }
    }
}

/// The Windows accent colour in the shade Windows itself uses on this background (a lighter one
/// on dark, a darker one on light), or `None` when it is a grey that would not read as an
/// accent: the toggles and highlights would disappear.
#[cfg(not(windows))]
fn windows_accent(_: bool) -> Option<Rgba> {
    None
}

#[cfg(windows)]
fn windows_accent(dark: bool) -> Option<Rgba> {
    let palette = registry_binary(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Accent", "AccentPalette")?;
    // Eight RGBA entries, lightest to darkest; the accent itself is the fourth.
    let i = if dark { 1 } else { 4 };
    let px = palette.get(i * 4..i * 4 + 3)?;
    let (r, g, b) = (px[0], px[1], px[2]);
    let chroma = r.max(g).max(b) - r.min(g).min(b);
    (chroma >= 48).then(|| rgb(u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)))
}

#[cfg(windows)]
fn registry_binary(key: &str, value: &str) -> Option<Vec<u8>> {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_BINARY, RegGetValueW};
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let mut buf = vec![0u8; 64];
    let mut len = buf.len() as u32;
    let ok = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            wide(key).as_ptr(),
            wide(value).as_ptr(),
            RRF_RT_REG_BINARY,
            std::ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut len,
        )
    };
    (ok == 0).then(|| {
        buf.truncate(len as usize);
        buf
    })
}
