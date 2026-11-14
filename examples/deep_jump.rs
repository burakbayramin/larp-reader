//! Times deep jumps in a long synthetic book: `cargo run --release --example deep_jump`.
use std::time::Instant;

use fcc::book::{Book, Chapter, Paragraph};
use fcc::masks::{MASK_NAMES, get_mask};
use fcc::session::Session;

const WORDS: &[&str] = &[
    "the",
    "river",
    "carried",
    "everything,",
    "slowly",
    "toward",
    "a",
    "sea",
    "nobody;",
    "had",
    "ever",
    "seen.",
    "Sözcük",
    "ağır",
    "ağır",
    "akıyordu",
    "ve",
    "kimse",
    "bilmiyordu.",
];

fn book(paragraphs: usize, chapters: usize) -> Book {
    let mut x: u64 = 12345;
    let mut next = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    let per = paragraphs / chapters;
    Book {
        id: "bench".into(),
        chapters: (0..chapters)
            .map(|c| Chapter {
                title: Some(format!("Chapter {c}")),
                paragraphs: (0..per)
                    .map(|_| {
                        let n = if next() % 50 == 0 {
                            600 + (next() % 600) as usize
                        } else {
                            80 + (next() % 121) as usize
                        };
                        let words: Vec<&str> = (0..n)
                            .map(|_| WORDS[(next() % WORDS.len() as u64) as usize])
                            .collect();
                        Paragraph::new(words.join(" "))
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn main() {
    let b = book(20_000, 20);
    let last = (
        b.chapters.len() - 1,
        b.chapters.last().unwrap().paragraphs.len() - 1,
    );
    println!(
        "{:<8} {:>10} {:>10} {:>10} {:>10}",
        "mask", "open ms", "resize ms", "again ms", "scroll ms"
    );
    let (mut t0, mut t1, mut t2, mut t3) = (0.0, 0.0, 0.0, 0.0);
    for name in MASK_NAMES {
        let mut s = Session::new(b.clone(), get_mask(name, "python").unwrap(), 3, 100, last);
        let t = Instant::now();
        let n = s.paragraph_lines(last).len();
        let open = t.elapsed().as_secs_f64() * 1e3;
        s.set_width(101);
        let t = Instant::now();
        let m = s.paragraph_lines(last).len();
        let resize = t.elapsed().as_secs_f64() * 1e3;
        let t = Instant::now();
        let m2 = s.paragraph_lines(last).len();
        let again = t.elapsed().as_secs_f64() * 1e3;
        let t = Instant::now();
        s.set_pos(last);
        for _ in 0..200 {
            s.back(5);
            s.window(40);
        }
        let scroll = t.elapsed().as_secs_f64() * 1e3;
        println!(
            "{name:<8} {open:>10.1} {resize:>10.1} {again:>10.1} {scroll:>10.1}   ({n} {m} {m2})"
        );
        t0 += open;
        t1 += resize;
        t2 += again;
        t3 += scroll;
    }
    println!("{:<8} {t0:>10.1} {t1:>10.1} {t2:>10.1} {t3:>10.1}", "total");
}
