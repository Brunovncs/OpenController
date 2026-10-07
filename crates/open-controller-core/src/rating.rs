//! How sure OpenController is that a controller works, in the grades the window and the
//! website show next to it. Only a controller someone tested on real hardware is verified; the
//! rest is what its USB ids and family promise.

use crate::extras::{Family, Hint};
use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Rating {
    /// Tested with OpenController on the controller itself: everything it has works.
    Verified,
    /// A known model whose protocol SDL reads in full, not tested here.
    Compatible,
    /// Works, with something to know first: a mode to switch to, a program that takes it, buttons
    /// Windows keeps to itself.
    Caveats,
    /// Recognised, but not read yet.
    Unsupported,
    /// Not in the table: most such controllers still work through SDL's generic support.
    Unknown,
}

/// Controllers tested on hardware, by the ids they showed. A clone that copies one of these ids
/// is taken for the original, as SDL takes it.
pub static VERIFIED: &[(u16, u16, &str)] = &[
    (0x2dc8, 0x310b, "8BitDo Ultimate 2 Wireless, on its receiver"),
    (0x2dc8, 0x6012, "8BitDo Ultimate 2 Wireless, in D-input mode"),
    (0x054c, 0x05c4, "DualShock 4"),
    (0x054c, 0x09cc, "DualShock 4 (second model)"),
];

pub fn is_verified(vendor: u16, product: u16) -> bool {
    VERIFIED.iter().any(|v| v.0 == vendor && v.1 == product)
}

pub fn rating(vendor: u16, product: u16, family: Family, hint: Option<Hint>) -> Rating {
    if hint == Some(Hint::Switch2Unsupported) {
        Rating::Unsupported
    } else if is_verified(vendor, product) {
        Rating::Verified
    } else if hint.is_some() {
        Rating::Caveats
    } else if models::lookup(vendor, product).is_some() || family != Family::Other {
        Rating::Compatible
    } else {
        Rating::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extras::hint;

    fn of(vendor: u16, product: u16) -> Rating {
        let family = models::lookup(vendor, product).map_or(Family::Other, |r| r.2);
        rating(vendor, product, family, hint(vendor, product, family))
    }

    #[test]
    fn grades() {
        assert_eq!(of(0x054c, 0x05c4), Rating::Verified);
        assert_eq!(of(0x2dc8, 0x310b), Rating::Verified, "tested, though its extras need D-input");
        assert_eq!(of(0x054c, 0x0ce6), Rating::Compatible, "DualSense");
        assert_eq!(of(0x045e, 0x0b00), Rating::Caveats, "Elite Series 2: paddles hidden");
        assert_eq!(of(0x1234, 0x5678), Rating::Unknown);
    }

    #[test]
    fn verified_ids_are_in_the_table() {
        assert!(VERIFIED.iter().all(|v| models::lookup(v.0, v.1).is_some()));
    }
}
