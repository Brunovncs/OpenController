//! DualSense's documented 11-byte trigger commands, sent through SDL's transport/CRC handling.
//! Only Feedback (0x21), Weapon (0x25), Vibration (0x26) and Off (0x05) are emitted.
//! Layout references: SDL testcontroller.c and Nielk1's public trigger protocol documentation:
//! https://gist.github.com/Nielk1/6d54cc2c00d2201ccb8c2720ad7538db
use crate::lab::{EffectMode, TriggerEffect};

pub fn encode(effect: TriggerEffect) -> [u8; 11] {
    let e = effect.clean();
    let mut data = [0; 11];
    data[0] = 0x05;
    if e.mode == EffectMode::Off || e.strength == 0 {
        return data;
    }
    let force = u32::from(e.strength - 1);
    if e.mode == EffectMode::Click {
        data[0] = 0x25;
        data[1..3].copy_from_slice(&((1u16 << e.start) | (1u16 << e.end)).to_le_bytes());
        data[3] = force as u8;
    } else {
        data[0] = if e.mode == EffectMode::Vibration { 0x26 } else { 0x21 };
        let zones = (0x03ffu16 << e.start) & 0x03ff;
        data[1..3].copy_from_slice(&zones.to_le_bytes());
        let packed = (e.start..10).fold(0u32, |bits, zone| bits | (force << (3 * zone)));
        data[3..7].copy_from_slice(&packed.to_le_bytes());
        if e.mode == EffectMode::Vibration {
            data[9] = e.frequency;
        }
    }
    data
}

/// SDL DS5EffectsState payload: flags, ten audio/rumble bytes, R2 then L2, remaining fields zero.
pub fn report(left_right: [TriggerEffect; 2]) -> [u8; 47] {
    let mut data = [0; 47];
    data[0] = 0x0c;
    data[10..21].copy_from_slice(&encode(left_right[1]));
    data[21..32].copy_from_slice(&encode(left_right[0]));
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn standard_modes_pack_zones_and_never_touch_audio_or_lights() {
        let rigid = TriggerEffect { mode: EffectMode::Resistance, start: 3, strength: 8, ..Default::default() };
        let bytes = encode(rigid);
        assert_eq!(&bytes[..3], &[0x21, 0xf8, 3]);
        let packed = u32::from_le_bytes(bytes[3..7].try_into().unwrap());
        for zone in 0..10 {
            assert_eq!((packed >> (zone * 3)) & 7, if zone >= 3 { 7 } else { 0 });
        }
        let pulse = TriggerEffect { mode: EffectMode::Vibration, frequency: 35, ..rigid };
        assert_eq!(encode(pulse)[9], 35);
        let r = report([rigid, pulse]);
        assert_eq!(r[0], 12);
        assert!(r[1..10].iter().all(|v| *v == 0));
        assert_eq!(r[10], 0x26);
        assert_eq!(r[21], 0x21);
        assert!(r[32..].iter().all(|v| *v == 0));
        assert_eq!(encode(TriggerEffect::default())[0], 5);
    }
    #[test]
    fn click_bounds_and_zero_strength_are_safe() {
        let e = TriggerEffect { mode: EffectMode::Click, start: 255, end: 0, strength: 255, ..Default::default() };
        let data = encode(e);
        assert_eq!(data[0], 0x25);
        assert_eq!(u16::from_le_bytes([data[1], data[2]]), (1 << 7) | (1 << 8));
        assert_eq!(encode(TriggerEffect { strength: 0, ..e })[0], 5);
    }
}
