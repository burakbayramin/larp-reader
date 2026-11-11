//! How much filler goes after a paragraph (`noise.py`).

use rand::Rng;

use crate::masks::{Line, Mask};

pub const MAX_NOISE: u8 = 5;

/// Filler lines after one paragraph: 0 to twice the level, so level 0 gives none.
pub fn noise_count(level: u8, rng: &mut impl Rng) -> usize {
    assert!(
        level <= MAX_NOISE,
        "noise level must be 0..={MAX_NOISE}, got {level}"
    );
    rng.gen_range(0..=2 * level as usize)
}

/// The filler after one paragraph: `count` lines, or a count drawn for `level`.
pub fn noise_lines(
    mask: &mut dyn Mask,
    level: u8,
    rng: &mut crate::masks::Rng,
    width: usize,
    count: Option<usize>,
) -> Vec<Line> {
    let count = count.unwrap_or_else(|| noise_count(level, rng));
    // masks whose filler only makes sense in whole units build the block themselves
    if let Some(block) = mask.noise_block(count, rng, width) {
        return block;
    }
    (0..count).map(|_| mask.noise(rng, width)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_pcg::Pcg32;
    use std::collections::BTreeSet;

    // -- test_noise.py (the parts that do not need a mask) --

    #[test]
    fn level_zero_gives_no_noise() {
        assert_eq!(noise_count(0, &mut Pcg32::seed_from_u64(1)), 0);
    }

    #[test]
    fn counts_stay_within_twice_the_level() {
        for level in 0..=MAX_NOISE {
            let most = (0..50)
                .map(|s| noise_count(level, &mut Pcg32::seed_from_u64(s)))
                .max()
                .unwrap();
            assert!(most <= 2 * level as usize);
        }
    }

    #[test]
    fn count_varies_and_reaches_max() {
        let counts: BTreeSet<usize> = (0..200)
            .map(|s| noise_count(2, &mut Pcg32::seed_from_u64(s)))
            .collect();
        assert_eq!(counts, BTreeSet::from([0, 1, 2, 3, 4]));
    }

    #[test]
    #[should_panic]
    fn level_above_max_is_a_bug() {
        noise_count(6, &mut Pcg32::seed_from_u64(1));
    }
}
