//! The phone inventory and the segmenter: a ruled IPA string to an ordered list of
//! phones with stress, for the consumer that turns phones into sound. The
//! inventory is data (`data/phonology/<language>.phones.tsv`); this module reads
//! it and owns one rule, that every symbol of the string is a phone the inventory
//! lists. A symbol it does not list is refused: no nearest phone, no spelling
//! rule, no guess. Tense/lax is a column of the inventory, so /i/ and /ɪ/ are two
//! phones here, not one phone and a flag on an entity.
//!
//! Stress marks are not phones. `ˈ` (primary) and `ˌ` (secondary) put their
//! stress on the next vowel phone, the syllable's nucleus; every other phone is
//! `none`. A mark with no vowel after it is refused. Durations, pitch and
//! acoustics are the consumer's, never read or written here.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Serialize;

const EN_US: &str = include_str!("../../../data/phonology/en-US.phones.tsv");

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Stress {
    None,
    Primary,
    Secondary,
}

/// One phone of a segmented string: its IPA symbol as the inventory spells it
/// and its stress.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Phone {
    pub ipa: String,
    pub stress: Stress,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tenseness {
    Tense,
    Lax,
    Reduced,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Class {
    Consonant,
    Vowel(Tenseness),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SegmentError {
    /// The symbol (or the text from it on) matches no inventory phone.
    UnknownPhone { at: usize, text: String },
    /// A stress mark (`at` is its byte offset) that finds no vowel before the
    /// string ends or another mark comes.
    DanglingStress { at: usize },
}

pub struct PhoneInventory {
    phones: BTreeMap<String, Class>,
    longest: usize,
}

impl PhoneInventory {
    /// The inventory for a language tag, if one is committed.
    pub fn for_language(language: &str) -> Option<&'static PhoneInventory> {
        static EN_US_INV: OnceLock<PhoneInventory> = OnceLock::new();
        match language {
            "en-US" => Some(EN_US_INV.get_or_init(|| {
                PhoneInventory::parse(EN_US).expect("the committed en-US phone inventory parses")
            })),
            _ => None,
        }
    }

    /// Parse the tab-separated inventory. Malformed lines are errors, as are
    /// repeated symbols.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut phones = BTreeMap::new();
        for (n, line) in text.lines().enumerate() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let cols: Vec<&str> = line.split('\t').collect();
            let [ipa, class, tenseness, gloss] = cols[..] else {
                return Err(format!("line {}: want 4 tab-separated columns", n + 1));
            };
            let class = match (class, tenseness) {
                ("consonant", "-") => Class::Consonant,
                ("vowel", "tense") => Class::Vowel(Tenseness::Tense),
                ("vowel", "lax") => Class::Vowel(Tenseness::Lax),
                ("vowel", "reduced") => Class::Vowel(Tenseness::Reduced),
                _ => return Err(format!("line {}: bad class/tenseness {class}/{tenseness}", n + 1)),
            };
            if ipa.is_empty() || gloss.is_empty() || ipa.chars().any(|c| matches!(c, 'ˈ' | 'ˌ')) {
                return Err(format!("line {}: empty or stress-marked entry", n + 1));
            }
            if phones.insert(ipa.to_string(), class).is_some() {
                return Err(format!("line {}: {ipa} repeats", n + 1));
            }
        }
        let longest = phones.keys().map(|k| k.chars().count()).max().unwrap_or(0);
        Ok(Self { phones, longest })
    }

    pub fn class(&self, ipa: &str) -> Option<&Class> {
        self.phones.get(ipa)
    }

    /// Segment `ipa` into phones: longest inventory match at each position.
    pub fn segment(&self, ipa: &str) -> Result<Vec<Phone>, SegmentError> {
        let chars: Vec<(usize, char)> = ipa.char_indices().collect();
        let mut out: Vec<Phone> = Vec::new();
        let mut pending: Option<(Stress, usize)> = None;
        let mut i = 0;
        while i < chars.len() {
            let at = chars[i].0;
            let mark = match chars[i].1 {
                'ˈ' => Some(Stress::Primary),
                'ˌ' => Some(Stress::Secondary),
                _ => None,
            };
            if let Some(m) = mark {
                if let Some((_, was)) = pending {
                    return Err(SegmentError::DanglingStress { at: was });
                }
                pending = Some((m, at));
                i += 1;
                continue;
            }
            let hit = (1..=self.longest.min(chars.len() - i)).rev().find_map(|len| {
                let end = chars.get(i + len).map_or(ipa.len(), |c| c.0);
                let s = &ipa[at..end];
                self.phones.get(s).map(|c| (len, s, c))
            });
            let Some((len, s, class)) = hit else {
                return Err(SegmentError::UnknownPhone { at, text: ipa[at..].to_string() });
            };
            let stress = if matches!(class, Class::Vowel(_)) {
                pending.take().map_or(Stress::None, |(m, _)| m)
            } else {
                Stress::None
            };
            out.push(Phone { ipa: s.to_string(), stress });
            i += len;
        }
        if let Some((_, at)) = pending {
            return Err(SegmentError::DanglingStress { at });
        }
        Ok(out)
    }
}
