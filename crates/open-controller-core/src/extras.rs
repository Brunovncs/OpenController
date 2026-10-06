//! The buttons a controller has beyond the Xbox set (back paddles, L4/R4, capture, mic, touchpad
//! click), what each is called on that controller, how the controller is drawn, and what the user
//! can do to get more out of it. Games never see these buttons through a virtual Xbox
//! controller, so they are what the user assigns.

use crate::device::PadType;
use crate::mapping::button;
use crate::models;
use serde::{Deserialize, Serialize};

pub const RIGHT_PADDLE1: u8 = 16;
pub const LEFT_PADDLE1: u8 = 17;
pub const RIGHT_PADDLE2: u8 = 18;
pub const LEFT_PADDLE2: u8 = 19;
pub const TOUCHPAD: u8 = button::TOUCHPAD as u8;
pub const MISC1: u8 = button::MISC1 as u8;
pub const MISC2: u8 = 21;
pub const MISC3: u8 = 22;
pub const MISC4: u8 = 23;
pub const MISC5: u8 = 24;
pub const MISC6: u8 = 25;

/// SDL button indices with no Xbox equivalent, in the order they are listed: back buttons
/// first, then the rest.
pub const ALL: [u8; 11] = [LEFT_PADDLE1, RIGHT_PADDLE1, LEFT_PADDLE2, RIGHT_PADDLE2, MISC1, TOUCHPAD, MISC2, MISC3, MISC4, MISC5, MISC6];

/// Controllers whose extra buttons, drawing or advice differ from the rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Family {
    DualShock3,
    DualShock4,
    DualSense,
    DualSenseEdge,
    /// PlayStation and PlayStation 2 controllers on a USB adapter.
    Ps2Adapter,
    SwitchPro,
    JoyCons,
    /// Nintendo Switch Online's SNES, N64 and Mega Drive controllers.
    NintendoClassic,
    /// Switch 2 controllers, which SDL reads only through libusb.
    Switch2,
    GameCube,
    Wii,
    Xbox,
    XboxElite,
    SteamController,
    /// Steam Deck and the controllers built like it (Steam Controller 2026, HORIPAD for Steam).
    SteamDeck,
    /// 8BitDo pads without buttons of their own: SN30 Pro, Lite, Zero, M30 and the retro range.
    EightBitDo,
    /// L4 and R4 above the back and PL and PR on it: Ultimate 2 Wireless, Ultimate 3, Pro 3.
    EightBitDoFour,
    /// Pro 2, with two back paddles.
    EightBitDoPro2,
    /// The first Ultimate, with two back buttons and the star (profile) button.
    EightBitDoUltimate,
    /// Ultimate 2C, with L4 and R4.
    EightBitDoUltimate2C,
    Flydigi,
    /// Flydigi Apex 5 and 6, whose two extra front buttons are LM and RM.
    FlydigiApex,
    Luna,
    Stadia,
    Shield,
    Other,
}

pub fn family(vendor: u16, product: u16, ty: PadType) -> Family {
    if let Some(row) = models::lookup(vendor, product) {
        return row.2;
    }
    match ty {
        PadType::Ps3 => Family::DualShock3,
        PadType::Ps4 => Family::DualShock4,
        PadType::Ps5 => Family::DualSense,
        PadType::SwitchPro => Family::SwitchPro,
        PadType::JoyConLeft | PadType::JoyConRight | PadType::JoyConPair => Family::JoyCons,
        PadType::Xbox360 | PadType::XboxOne => Family::Xbox,
        _ if vendor == 0x2DC8 => Family::EightBitDo,
        _ => Family::Other,
    }
}

/// What the button is called, as printed on the controller or as its maker names it. `None` for
/// buttons that have no printed name; the interface then describes where the button is.
pub fn printed_name(family: Family, b: u8) -> Option<&'static str> {
    use Family::*;
    Some(match (family, b) {
        (EightBitDoFour, RIGHT_PADDLE1) => "R4",
        (EightBitDoFour, LEFT_PADDLE1) => "L4",
        (EightBitDoFour, RIGHT_PADDLE2) => "PR",
        (EightBitDoFour, LEFT_PADDLE2) => "PL",
        (EightBitDoPro2, RIGHT_PADDLE1) => "PR",
        (EightBitDoPro2, LEFT_PADDLE1) => "PL",
        (EightBitDoUltimate2C, RIGHT_PADDLE1) => "R4",
        (EightBitDoUltimate2C, LEFT_PADDLE1) => "L4",
        (EightBitDoUltimate, MISC1) => "★",
        (SteamDeck, MISC1) => "···",
        (SteamDeck, RIGHT_PADDLE1) => "R4",
        (SteamDeck, LEFT_PADDLE1) => "L4",
        (SteamDeck, RIGHT_PADDLE2) => "R5",
        (SteamDeck, LEFT_PADDLE2) => "L5",
        (DualSenseEdge, RIGHT_PADDLE2) => "Fn R",
        (DualSenseEdge, LEFT_PADDLE2) => "Fn L",
        (JoyCons, RIGHT_PADDLE1) => "SR (R)",
        (JoyCons, LEFT_PADDLE1) => "SL (L)",
        (JoyCons, RIGHT_PADDLE2) => "SL (R)",
        (JoyCons, LEFT_PADDLE2) => "SR (L)",
        (Flydigi | FlydigiApex, RIGHT_PADDLE1) => "M1",
        (Flydigi | FlydigiApex, LEFT_PADDLE1) => "M2",
        (Flydigi | FlydigiApex, RIGHT_PADDLE2) => "M3",
        (Flydigi | FlydigiApex, LEFT_PADDLE2) => "M4",
        (Flydigi, MISC2) => "C",
        (Flydigi, MISC3) => "Z",
        (Flydigi, MISC4) | (FlydigiApex, MISC2) => "LM",
        (Flydigi, MISC5) | (FlydigiApex, MISC3) => "RM",
        (Flydigi, MISC6) => "◯",
        (Shield, MISC2) => "Vol −",
        (Shield, MISC3) => "Vol +",
        _ => return None,
    })
}

/// Where a button without a printed name is, or what it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    BackLeft,
    BackRight,
    BackLeft2,
    BackRight2,
    Touchpad,
    TrackpadLeft,
    TrackpadRight,
    StickTouchLeft,
    StickTouchRight,
    TriggerClickLeft,
    TriggerClickRight,
    Capture,
    Mic,
    Share,
    Assistant,
    Extra(u8),
}

pub fn kind(family: Family, b: u8) -> Kind {
    use Family::*;
    match (family, b) {
        (SteamDeck, TOUCHPAD) => Kind::TrackpadLeft,
        (SteamDeck, MISC2) => Kind::TrackpadRight,
        (SteamDeck, MISC3) => Kind::StickTouchLeft,
        (SteamDeck, MISC4) => Kind::StickTouchRight,
        (GameCube, MISC3) => Kind::TriggerClickLeft,
        (GameCube, MISC4) => Kind::TriggerClickRight,
        (Stadia, MISC2) => Kind::Assistant,
        (_, LEFT_PADDLE1) => Kind::BackLeft,
        (_, RIGHT_PADDLE1) => Kind::BackRight,
        (_, LEFT_PADDLE2) => Kind::BackLeft2,
        (_, RIGHT_PADDLE2) => Kind::BackRight2,
        (_, TOUCHPAD) => Kind::Touchpad,
        (SwitchPro | JoyCons | NintendoClassic | Switch2 | Stadia, MISC1) => Kind::Capture,
        (DualSense | DualSenseEdge | Luna, MISC1) => Kind::Mic,
        (_, MISC1) => Kind::Share,
        (_, other) => Kind::Extra(other - 20),
    }
}

/// How the controller is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Art {
    /// Left stick above the d-pad: Xbox, Switch Pro, 8BitDo Ultimate, most pads.
    Offset,
    /// D-pad and face buttons above two sticks side by side: DualShock 3, PlayStation 2, 8BitDo
    /// Pro 2 and Pro 3, Logitech F310.
    Symmetric,
    /// Symmetric, with a touchpad between the hands: DualShock 4, DualSense.
    PlayStation,
    /// No sticks: SNES, NES and Mega Drive style pads.
    Retro,
    JoyCons,
    /// A screen between the hands: Steam Deck.
    Handheld,
}

pub fn art(family: Family, vendor: u16, product: u16) -> Art {
    use Family::*;
    match family {
        DualShock4 | DualSense | DualSenseEdge => Art::PlayStation,
        DualShock3 | Ps2Adapter | EightBitDoPro2 => Art::Symmetric,
        JoyCons => Art::JoyCons,
        SteamDeck if product == 0x1205 => Art::Handheld,
        NintendoClassic if product != 0x2019 => Art::Retro,
        EightBitDoFour if product == 0x6009 || product == 0x3109 => Art::Symmetric,
        EightBitDo => match product {
            // SF30 Pro, SN30 Pro and SN30 Pro+, wherever they connect.
            0x6000 | 0x6100 | 0x6001 | 0x6101 | 0x6002 | 0x6102 | 0x2100 | 0x2101 | 0x2865 | 0x9015 | 0x3810 | 0x3820 | 0x9000 => {
                Art::Symmetric
            }
            0x3230 | 0x9018 | 0x9020 | 0x0651 | 0x5006 | 0x5111 | 0x5112 | 0x2862 | 0x9012 | 0xab20 | 0xab21 | 0xab12 | 0xab11 | 0x9025 => {
                Art::Retro
            }
            _ => Art::Offset,
        },
        // Logitech F310, F510 and F710 in either mode.
        Other if vendor == 0x046D && matches!(product, 0xC21D | 0xC216 | 0xC21F | 0xC219 | 0xC21E | 0xC218) => Art::Symmetric,
        _ => Art::Offset,
    }
}

/// What the controller can do besides buttons and sticks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Features {
    pub touchpad: bool,
    pub motion: bool,
    pub rumble: bool,
    pub trigger_rumble: bool,
    pub light_bar: bool,
    pub player_lights: bool,
}

/// Something the user can do to get more out of this controller.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hint {
    /// An 8BitDo pad in its XInput mode, where its extra buttons send nothing a program can read.
    /// Turned on while holding B, it uses D-input mode.
    EightBitDoDInput,
    /// An 8BitDo pad with a mode switch on its back (Pro 2) in its XInput position; D is D-input.
    EightBitDoSwitchD,
    /// An 8BitDo Ultimate 2C on its receiver, where L4 and R4 are hidden; over Bluetooth they work.
    EightBitDo2CBluetooth,
    /// An 8BitDo pad made for Xbox: its extra buttons only copy other buttons, set in 8BitDo's app.
    EightBitDoForXbox,
    /// An Xbox Elite controller: Windows does not pass its paddles on, except as copies of other
    /// buttons set up in the Xbox Accessories app.
    XboxPaddlesHidden,
    /// Steam takes Valve's controllers for itself while it runs.
    SteamInput,
    /// Flydigi's app must allow other programs to read the extra buttons.
    FlydigiThirdParty,
    /// Switch 2 controllers need libusb, which this build of SDL leaves out.
    Switch2Unsupported,
}

pub fn hint(vendor: u16, product: u16, family: Family) -> Option<Hint> {
    match (vendor, product, family) {
        (0x2DC8, 0x310B | 0x3109, _) => Some(Hint::EightBitDoDInput),
        (0x2DC8, 0x3106, _) => Some(Hint::EightBitDoSwitchD),
        (0x2DC8, 0x310A, _) => Some(Hint::EightBitDo2CBluetooth),
        (0x2DC8, 0x2002 | 0x200F, _) => Some(Hint::EightBitDoForXbox),
        (_, _, Family::XboxElite) => Some(Hint::XboxPaddlesHidden),
        (_, _, Family::SteamController | Family::SteamDeck) => Some(Hint::SteamInput),
        (_, _, Family::Flydigi | Family::FlydigiApex) => Some(Hint::FlydigiThirdParty),
        (_, _, Family::Switch2) => Some(Hint::Switch2Unsupported),
        _ => None,
    }
}

/// Mappings SDL 3.4 lacks for buttons its drivers do report: the joystick buttons each
/// controller's extra buttons arrive on, as the SDL gamepad mapping fields to add.
pub fn missing_mapping(vendor: u16, product: u16) -> Option<&'static str> {
    match (vendor, product) {
        // 8BitDo Ultimate 3: L4, R4, PL, PR and Share are joystick buttons 11 to 15.
        (0x2DC8, 0x202F) => Some("paddle1:b12,paddle2:b11,paddle3:b14,paddle4:b13,misc1:b15"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indices_are_sdls() {
        assert_eq!(button::MISC1, 15);
        assert_eq!(button::TOUCHPAD, 20);
        assert!(ALL.iter().all(|&b| (b as u32) >= button::MISC1 && (b as u32) < button::COUNT));
    }

    #[test]
    fn eightbitdo_names_follow_sdls_paddle_order() {
        // SDL: paddle1:b12 (R4), paddle2:b11 (L4), paddle3:b14 (PR), paddle4:b13 (PL).
        let f = family(0x2DC8, 0x6012, PadType::Standard);
        assert_eq!(f, Family::EightBitDoFour);
        let names: Vec<_> = [RIGHT_PADDLE1, LEFT_PADDLE1, RIGHT_PADDLE2, LEFT_PADDLE2].iter().map(|&b| printed_name(f, b)).collect();
        assert_eq!(names, [Some("R4"), Some("L4"), Some("PR"), Some("PL")]);
        assert_eq!(family(0x2DC8, 0x202F, PadType::Standard), Family::EightBitDoFour, "Ultimate 3");
        assert_eq!(family(0x2DC8, 0x9999, PadType::Standard), Family::EightBitDo, "an 8BitDo pad the table does not know");
    }

    #[test]
    fn misc_button_depends_on_the_maker() {
        assert_eq!(kind(family(0x057E, 0x2009, PadType::SwitchPro), MISC1), Kind::Capture);
        assert_eq!(kind(family(0x054C, 0x0CE6, PadType::Ps5), MISC1), Kind::Mic);
        assert_eq!(kind(Family::Other, MISC1), Kind::Share);
        assert_eq!(kind(Family::Other, 23), Kind::Extra(3));
        assert_eq!(kind(Family::SteamDeck, MISC2), Kind::TrackpadRight);
    }

    #[test]
    fn licensed_pads_follow_their_console() {
        assert_eq!(family(0x1532, 0x1000, PadType::Ps4), Family::DualShock4, "Razer Raiju");
        assert_eq!(family(0x1234, 0x5678, PadType::Ps5), Family::DualSense, "unknown, by SDL's type");
    }

    #[test]
    fn art() {
        use super::art;
        assert_eq!(art(Family::DualSense, 0x054C, 0x0CE6), Art::PlayStation);
        assert_eq!(art(Family::EightBitDoFour, 0x2DC8, 0x6012), Art::Offset);
        assert_eq!(art(Family::EightBitDoFour, 0x2DC8, 0x6009), Art::Symmetric, "Pro 3");
        assert_eq!(art(Family::EightBitDo, 0x2DC8, 0x5112), Art::Retro, "Lite 2");
        assert_eq!(art(Family::SteamDeck, 0x28DE, 0x1205), Art::Handheld);
    }

    #[test]
    fn hints() {
        assert_eq!(hint(0x2DC8, 0x310B, family(0x2DC8, 0x310B, PadType::XboxOne)), Some(Hint::EightBitDoDInput));
        assert_eq!(hint(0x2DC8, 0x6012, Family::EightBitDoFour), None);
        assert_eq!(hint(0x045E, 0x0B00, family(0x045E, 0x0B00, PadType::XboxOne)), Some(Hint::XboxPaddlesHidden));
        assert_eq!(hint(0x057E, 0x2069, family(0x057E, 0x2069, PadType::Standard)), Some(Hint::Switch2Unsupported));
    }
}
