//! The disguises text is shown in (`masks/__init__.py`, `masks/base.py`).

use rand::SeedableRng;

use crate::book::{Paragraph, Pos};
use crate::errors::FccError;

pub mod agent;
pub mod base;
pub mod build;
pub mod code;
pub mod codegen;
pub mod diff;
pub mod docker;
pub mod git;
pub mod log;
pub mod palette;
pub mod snippets;
pub mod test;
pub mod time;

pub use base::{Line, Segment};

/// The random source masks draw filler from: seeded, so a paragraph draws the same every time.
pub type Rng = rand_pcg::Pcg32;

/// A generator seeded from text (FNV-1a), like Python's `Random("book:pos:mask:noise")`.
pub fn seeded(seed: &str) -> Rng {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in seed.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    Rng::seed_from_u64(h)
}

/// `random.choice`.
pub fn pick<'a, T>(rng: &mut Rng, items: &'a [T]) -> &'a T {
    &items[rand::Rng::gen_range(rng, 0..items.len())]
}

/// `random.randint` (both ends included).
pub fn randint(rng: &mut Rng, a: usize, b: usize) -> usize {
    rand::Rng::gen_range(rng, a..=b)
}

/// `random.random`: in [0, 1).
pub fn random(rng: &mut Rng) -> f64 {
    rand::Rng::r#gen(rng)
}

/// `random.choices(items, weights)[0]`.
pub fn weighted<'a, T>(rng: &mut Rng, items: &'a [T], weights: &[u32]) -> &'a T {
    let total: u32 = weights.iter().sum();
    let mut x = rand::Rng::gen_range(rng, 0..total);
    for (item, w) in items.iter().zip(weights) {
        if x < *w {
            return item;
        }
        x -= w;
    }
    items.last().expect("non-empty")
}

/// In the order `m` cycles through them.
pub const MASK_NAMES: &[&str] = &[
    "code", "log", "diff", "test", "build", "git", "docker", "agent",
];

/// Source languages of the code mask.
pub const LANGS: &[&str] = &["python", "typescript", "go", "rust", "java", "csharp"];

pub trait Mask {
    fn name(&self) -> &'static str;

    /// Lines that open the output, shown above the first paragraph.
    fn header(&mut self, rng: &mut Rng, width: usize) -> Vec<Line>;

    /// A paragraph in disguise. `line_no_start` is the number of its first line,
    /// counted from the top of the text, for masks that number or time lines.
    fn render(
        &mut self,
        para: &Paragraph,
        pos: Pos,
        width: usize,
        rng: &mut Rng,
        line_no_start: usize,
    ) -> Vec<Line>;

    /// How many lines `render` gives for this paragraph, without building them.
    fn line_count(&self, para: &Paragraph, width: usize) -> usize;

    /// One line of filler.
    fn noise(&mut self, rng: &mut Rng, width: usize) -> Line;

    /// Exactly `count` lines of filler in whole units, for masks whose filler
    /// only makes sense that way; None means one `noise` line at a time.
    fn noise_block(&mut self, _count: usize, _rng: &mut Rng, _width: usize) -> Option<Vec<Line>> {
        None
    }

    fn error(&self, err: &FccError, width: usize) -> Vec<Line> {
        base::error_lines(err, width)
    }

    fn progress_label(&self, done: usize, total: usize, width: usize) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng as _;

    #[test]
    fn seeded_generators_repeat_and_differ() {
        let a: Vec<u32> = (0..5)
            .map(|_| 0)
            .scan(seeded("bk:(0, 1):code:2"), |r, _| Some(r.r#gen()))
            .collect();
        let b: Vec<u32> = (0..5)
            .map(|_| 0)
            .scan(seeded("bk:(0, 1):code:2"), |r, _| Some(r.r#gen()))
            .collect();
        let c: Vec<u32> = (0..5)
            .map(|_| 0)
            .scan(seeded("bk:(0, 2):code:2"), |r, _| Some(r.r#gen()))
            .collect();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
