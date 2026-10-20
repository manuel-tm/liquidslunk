//! `iPod_Control/iTunes/Play Counts` (4_external_libraries.md §7.7): what the
//! iPod counted since the database was last written, an entry per track in
//! the database's order. liquidslunk adds them to the database's counts when
//! it writes it again, and then removes the file, as iTunes did.

use super::bytes::Reader;
use super::itunesdb::Stats;
use crate::Result;

/// What the iPod counted for one track since the last sync.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counted {
    pub plays: u32,
    /// Mac time, 0 when it wasn't played
    pub last_played: u32,
    /// Stars times 20, as the iPod has it now, when its entries have one
    pub rating: Option<u8>,
    pub skips: u32,
    pub last_skipped: u32,
}

impl Counted {
    /// Adds what was counted to a track's counts.
    pub fn add_to(&self, stats: &mut Stats) {
        stats.play_count += self.plays;
        if self.last_played != 0 {
            stats.last_played = self.last_played;
        }
        stats.skip_count += self.skips;
        if self.last_skipped != 0 {
            stats.last_skipped = self.last_skipped;
        }
        if let Some(rating) = self.rating {
            stats.rating = rating;
        }
    }
}

/// Reads the file: an `mhdp` header, then its entries, whose length grew
/// with the firmware: plays, last played and bookmark (12 bytes), the
/// rating (16), and skips (28).
pub fn read(bytes: &[u8]) -> Result<Vec<Counted>> {
    let r = Reader::new(bytes, "Play Counts");
    r.expect(0, b"mhdp")?;
    let header = r.header_length(0)?;
    let entry = r.u32(8)? as usize;
    let count = r.u32(12)? as usize;
    if entry < 12 {
        return Err(r.damaged(format!("its entries are {entry} bytes long")));
    }
    let mut counted = Vec::with_capacity(count.min(100_000));
    for index in 0..count {
        let at = header + index * entry;
        counted.push(Counted {
            plays: r.u32(at)?,
            last_played: r.u32(at + 4)?,
            rating: if entry >= 16 {
                Some(r.u32(at + 12)? as u8)
            } else {
                None
            },
            skips: if entry >= 28 { r.u32(at + 20)? } else { 0 },
            last_skipped: if entry >= 28 { r.u32(at + 24)? } else { 0 },
        });
    }
    Ok(counted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::external::ipod::bytes::Writer;

    #[test]
    fn counts_are_added_to_the_tracks() {
        let mut w = Writer::new();
        w.tag(b"mhdp");
        w.u32(0x60);
        w.u32(28);
        w.u32(2);
        w.pad_to(0, 0x60);
        for (plays, rating, skips) in [(2, 60, 1), (0, 60, 0)] {
            w.u32(plays);
            w.u32(if plays > 0 { 3_000_000_000 } else { 0 });
            w.u32(0);
            w.u32(rating);
            w.u32(0);
            w.u32(skips);
            w.u32(0);
        }
        let counted = read(&w.bytes).unwrap();
        assert_eq!(counted.len(), 2);
        let mut stats = Stats {
            play_count: 5,
            rating: 20,
            ..Stats::default()
        };
        counted[0].add_to(&mut stats);
        assert_eq!(
            stats,
            Stats {
                play_count: 7,
                last_played: 3_000_000_000,
                skip_count: 1,
                last_skipped: 0,
                rating: 60,
            }
        );
        // Nothing counted changes nothing
        let before = stats;
        counted[1].add_to(&mut stats);
        assert_eq!(stats, before);
        assert!(read(b"mhdp").is_err());
    }
}
