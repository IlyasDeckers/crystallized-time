use crate::config::QuantizeConfig;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scale {
    Unquantized,
    Major,
    Minor,
    Pentatonic,
    Hirajoshi,
    Iwato,
}

impl Scale {
    pub fn intervals(self) -> &'static [i16] {
        match self {
            Scale::Unquantized => &[],
            Scale::Major => &[0, 2, 4, 5, 7, 9, 11],
            Scale::Minor => &[0, 2, 3, 5, 7, 8, 10],
            Scale::Pentatonic => &[0, 2, 4, 7, 9],
            Scale::Hirajoshi => &[0, 2, 3, 7, 8],
            Scale::Iwato => &[0, 1, 5, 6, 10],
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Scale::Unquantized => "Unquantized",
            Scale::Major => "Major",
            Scale::Minor => "Minor",
            Scale::Pentatonic => "Pentatonic",
            Scale::Hirajoshi => "Hirajoshi",
            Scale::Iwato => "Iwato",
        }
    }

    pub fn from_name(name: &str) -> Option<Scale> {
        match name.to_lowercase().as_str() {
            "unquantized" | "none" => Some(Scale::Unquantized),
            "major" => Some(Scale::Major),
            "minor" => Some(Scale::Minor),
            "pentatonic" => Some(Scale::Pentatonic),
            "hirajoshi" => Some(Scale::Hirajoshi),
            "iwato" => Some(Scale::Iwato),
            _ => None,
        }
    }

    pub fn all() -> &'static [Scale] {
        &[
            Scale::Unquantized,
            Scale::Major,
            Scale::Minor,
            Scale::Pentatonic,
            Scale::Hirajoshi,
            Scale::Iwato,
        ]
    }
}

#[derive(Clone, Debug)]
pub struct ScaleQuantizer {
    pub scale: Scale,
    pub root_note: u8,
}

impl ScaleQuantizer {
    pub fn from_config(config: &QuantizeConfig) -> Option<Self> {
        if config.scale == Scale::Unquantized {
            None
        } else {
            Some(Self {
                scale: config.scale,
                root_note: config.root_note,
            })
        }
    }

    pub fn quantize(&self, pitch: u8) -> u8 {
        let intervals = self.scale.intervals();
        if intervals.is_empty() {
            return pitch;
        }
        let root = (self.root_note % 12) as i16;
        let p = pitch as i16;

        let mut best_pitch = p;
        let mut best_dist = 256i16;

        for octave in -1i16..=1 {
            let base = (p / 12 + octave) * 12;
            for &interval in intervals {
                let candidate = base + root + interval;
                if candidate < 0 || candidate > 127 {
                    continue;
                }
                let dist = (p - candidate).abs();
                if dist < best_dist {
                    best_dist = dist;
                    best_pitch = candidate;
                }
            }
        }
        best_pitch as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unquantized_is_passthrough() {
        let q = ScaleQuantizer {
            scale: Scale::Unquantized,
            root_note: 60,
        };
        assert_eq!(q.quantize(61), 61);
        assert_eq!(q.quantize(0), 0);
        assert_eq!(q.quantize(127), 127);
    }

    #[test]
    fn major_snaps_to_scale() {
        let q = ScaleQuantizer {
            scale: Scale::Major,
            root_note: 60, // C4
        };
        assert_eq!(q.quantize(60), 60); // C
        assert_eq!(q.quantize(61), 60); // C# -> C
        assert_eq!(q.quantize(62), 62); // D
        assert_eq!(q.quantize(63), 62); // D# -> D
        assert_eq!(q.quantize(64), 64); // E
        assert_eq!(q.quantize(65), 65); // F
        assert_eq!(q.quantize(66), 65); // F# -> F
        assert_eq!(q.quantize(67), 67); // G
        assert_eq!(q.quantize(68), 67); // G# -> G (closer than A)
        assert_eq!(q.quantize(69), 69); // A
        assert_eq!(q.quantize(70), 69); // A# -> A (tie at 1 semitone, lower wins)
        assert_eq!(q.quantize(71), 71); // B
    }

    #[test]
    fn pentatonic_snaps() {
        let q = ScaleQuantizer {
            scale: Scale::Pentatonic,
            root_note: 60,
        };
        assert_eq!(q.quantize(60), 60); // C
        assert_eq!(q.quantize(62), 62); // D
        assert_eq!(q.quantize(64), 64); // E
        assert_eq!(q.quantize(67), 67); // G
        assert_eq!(q.quantize(69), 69); // A
        assert_eq!(q.quantize(61), 60); // C# -> C (tie: 1 semitone from both C and D, lower wins)
        assert_eq!(q.quantize(63), 62); // D# -> D
        assert_eq!(q.quantize(65), 64); // F -> E
        assert_eq!(q.quantize(68), 67); // G# -> G
    }

    #[test]
    fn hirajoshi_snaps() {
        let q = ScaleQuantizer {
            scale: Scale::Hirajoshi,
            root_note: 60,
        };
        assert_eq!(q.quantize(60), 60); // C
        assert_eq!(q.quantize(62), 62); // D
        assert_eq!(q.quantize(63), 63); // Eb
        assert_eq!(q.quantize(67), 67); // G
        assert_eq!(q.quantize(68), 68); // Ab
        assert_eq!(q.quantize(61), 60); // C# -> C
        assert_eq!(q.quantize(64), 63); // E -> Eb
        assert_eq!(q.quantize(65), 63); // F -> Eb (tie: 2 away from both Eb and G, goes to lower)
    }

    #[test]
    fn iwato_snaps() {
        let q = ScaleQuantizer {
            scale: Scale::Iwato,
            root_note: 60,
        };
        assert_eq!(q.quantize(60), 60); // C
        assert_eq!(q.quantize(61), 61); // Db
        assert_eq!(q.quantize(65), 65); // F
        assert_eq!(q.quantize(66), 66); // Gb
        assert_eq!(q.quantize(70), 70); // Bb
        assert_eq!(q.quantize(62), 61); // D -> Db
        assert_eq!(q.quantize(63), 61); // Eb -> Db (4 away from both Db and F)
    }

    #[test]
    fn different_root() {
        let q = ScaleQuantizer {
            scale: Scale::Major,
            root_note: 62, // D4 -> D major
        };
        assert_eq!(q.quantize(60), 59); // C -> B3 (1 semitone down in D major)
        assert_eq!(q.quantize(62), 62); // D
        assert_eq!(q.quantize(64), 64); // E
        assert_eq!(q.quantize(66), 66); // F#
    }

    #[test]
    fn clamps_to_range() {
        let q = ScaleQuantizer {
            scale: Scale::Major,
            root_note: 60,
        };
        assert!(q.quantize(0) <= 127);
        assert!(q.quantize(127) <= 127);
    }

    #[test]
    fn scale_names_roundtrip() {
        for &scale in Scale::all() {
            let name = scale.name();
            let parsed = Scale::from_name(name);
            assert_eq!(parsed, Some(scale));
        }
    }
}
