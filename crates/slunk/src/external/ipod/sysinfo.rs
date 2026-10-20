//! `iPod_Control/Device/SysInfo` (4_external_libraries.md §7.1): the iPod's
//! own description of itself, as `Key: value` lines, which liquidslunk only
//! reads. Its model number says which iPod it is.

use std::collections::HashMap;

/// An iPod with video: the only iPods liquidslunk syncs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Model {
    /// Like "A446", without the letter for its region
    pub number: &'static str,
    /// 5 for the 5th generation, 55 for the 5.5th
    pub generation: u8,
    pub gigabytes: u32,
    pub finish: &'static str,
}

impl Model {
    /// "iPod with video (5.5th generation), 30 GB, black"
    pub fn description(&self) -> String {
        let generation = match self.generation {
            5 => "5th",
            _ => "5.5th",
        };
        format!(
            "iPod with video ({generation} generation), {} GB, {}",
            self.gigabytes, self.finish
        )
    }
}

const MODELS: [Model; 10] = [
    model("A002", 5, 30, "white"),
    model("A146", 5, 30, "black"),
    model("A003", 5, 60, "white"),
    model("A147", 5, 60, "black"),
    model("A452", 5, 30, "U2 edition"),
    model("A444", 55, 30, "white"),
    model("A446", 55, 30, "black"),
    model("A664", 55, 30, "U2 edition"),
    model("A448", 55, 80, "white"),
    model("A450", 55, 80, "black"),
];

const fn model(
    number: &'static str,
    generation: u8,
    gigabytes: u32,
    finish: &'static str,
) -> Model {
    Model {
        number,
        generation,
        gigabytes,
        finish,
    }
}

/// What an iPod's SysInfo says of it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SysInfo {
    pub values: HashMap<String, String>,
}

impl SysInfo {
    pub fn parse(text: &str) -> SysInfo {
        SysInfo {
            values: text
                .lines()
                .filter_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    Some((key.trim().to_string(), value.trim().to_string()))
                })
                .collect(),
        }
    }

    /// Its model number, like "A446", from `ModelNumStr`, which starts with
    /// a letter for its region: "xA446", "MA446".
    pub fn model_number(&self) -> Option<&str> {
        let number = self.values.get("ModelNumStr")?;
        let mut chars = number.chars();
        match chars.next() {
            Some(first) if first.is_ascii_alphabetic() => Some(chars.as_str()),
            _ => Some(number),
        }
    }

    /// Its model, when it's an iPod with video.
    pub fn model(&self) -> Option<Model> {
        let number = self.model_number()?;
        MODELS.iter().copied().find(|model| model.number == number)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_model_is_read_from_its_number() {
        let info = SysInfo::parse(
            "BoardHwName: iPod M25\nModelNumStr: xA446\nvisibleBuildID: 0x01318000 (1.3)\n",
        );
        assert_eq!(info.model_number(), Some("A446"));
        assert_eq!(
            info.model().unwrap().description(),
            "iPod with video (5.5th generation), 30 GB, black"
        );
        // A Classic isn't one
        let classic = SysInfo::parse("ModelNumStr: MB029\n");
        assert_eq!(classic.model(), None);
        assert_eq!(SysInfo::parse("").model_number(), None);
    }
}
