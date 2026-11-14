//! The reading engine, UI-independent (`session.py`): a position in a book plus render settings.
//!
//! `pos` is the paragraph holding the top line of the manual-mode view and
//! `line_offset` is that line's index within `paragraph_lines(pos)`. Setting
//! `pos` puts the view at the top of that paragraph.

use crate::book::{Book, Pos};
use crate::masks::{Line, Mask, seeded, seeded_fmt};
use crate::noise::{noise_count, noise_lines};

/// Running line numbers: valid for one (mask, noise, width).
struct Numbering {
    key: (u64, u8, usize),
    /// Paragraphs in reading order as far as counted, and the number of each one's first line.
    order: Vec<Pos>,
    starts: Vec<usize>,
}

pub struct Session {
    pub book: Book,
    mask: Box<dyn Mask>,
    /// Bumped on every mask change, so cached numbering knows it is stale.
    mask_gen: u64,
    noise: u8,
    width: usize,
    pos: Pos,
    pub line_offset: usize,
    numbering: Option<Numbering>,
}

impl Session {
    pub fn new(book: Book, mask: Box<dyn Mask>, noise: u8, width: usize, pos: Pos) -> Session {
        Session {
            book,
            mask,
            mask_gen: 0,
            noise,
            width,
            pos,
            line_offset: 0,
            numbering: None,
        }
    }

    pub fn pos(&self) -> Pos {
        self.pos
    }

    pub fn set_pos(&mut self, pos: Pos) {
        self.pos = pos;
        self.line_offset = 0;
    }

    pub fn mask(&self) -> &dyn Mask {
        self.mask.as_ref()
    }

    pub fn mask_mut(&mut self) -> &mut dyn Mask {
        self.mask.as_mut()
    }

    pub fn set_mask(&mut self, mask: Box<dyn Mask>) {
        self.mask = mask;
        self.mask_gen += 1;
    }

    pub fn noise(&self) -> u8 {
        self.noise
    }

    pub fn set_noise(&mut self, noise: u8) {
        self.noise = noise;
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn set_width(&mut self, width: usize) {
        self.width = width;
    }

    /// Each paragraph's own seed: the same filler every time it is drawn.
    fn seed(&self, pos: Pos) -> String {
        format!(
            "{}:({}, {}):{}:{}",
            self.book.id,
            pos.0,
            pos.1,
            self.mask.name(),
            self.noise
        )
    }

    /// Its own generator, so the count is known without rendering the paragraph.
    fn noise_count(&self, pos: Pos) -> usize {
        noise_count(
            self.noise,
            &mut seeded_fmt(format_args!(
                "{}:({}, {}):{}:{}:filler",
                self.book.id,
                pos.0,
                pos.1,
                self.mask.name(),
                self.noise
            )),
        )
    }

    /// Whether `pos` opens a chapter after the first one with text: such a
    /// paragraph starts with the mask's transition lines.
    fn opens_chapter(&self, pos: Pos) -> bool {
        pos.1 == 0 && self.book.prev_pos(pos).is_some()
    }

    fn render(&mut self, pos: Pos, start: usize) -> Vec<Line> {
        let mut rng = seeded(&self.seed(pos));
        let count = self.noise_count(pos);
        let mut lines = if self.opens_chapter(pos) {
            self.mask.transition(pos.0, self.width, start)
        } else {
            vec![]
        };
        let body_start = start + lines.len();
        let para = self.book.paragraph(pos).expect("a valid position");
        lines.extend(
            self.mask
                .render(para, pos, self.width, &mut rng, body_start),
        );
        lines.extend(noise_lines(
            self.mask.as_mut(),
            self.noise,
            &mut rng,
            self.width,
            Some(count),
        ));
        lines
    }

    fn line_count(&self, pos: Pos) -> usize {
        let para = self.book.paragraph(pos).expect("a valid position");
        let transition = if self.opens_chapter(pos) {
            self.mask.transition(pos.0, self.width, 0).len()
        } else {
            0
        };
        transition + self.mask.line_count(para, self.width) + self.noise_count(pos)
    }

    /// Number of the paragraph's first line, counted from the top of the text.
    ///
    /// Lines are numbered continuously like a real file, so a paragraph's start
    /// depends on every paragraph before it; the running totals are kept until
    /// the mask, noise level or width changes.
    fn line_no_start(&mut self, pos: Pos) -> usize {
        let key = (self.mask_gen, self.noise, self.width);
        if self.numbering.as_ref().is_none_or(|n| n.key != key) {
            let first = self
                .book
                .chapter_list()
                .first()
                .map(|(c, _)| (*c, 0))
                .unwrap_or((0, 0));
            let header = self
                .mask
                .header(&mut seeded(&self.book.id), self.width)
                .len();
            self.numbering = Some(Numbering {
                key,
                order: vec![first],
                starts: vec![header + 1],
            });
        }
        let index = self.book.index_of(pos);
        loop {
            let n = self.numbering.as_ref().expect("set above");
            if n.starts.len() > index {
                return n.starts[index];
            }
            let last = *n.order.last().expect("never empty");
            let next_start = n.starts.last().expect("never empty") + self.line_count(last);
            let Some(next) = self.book.next_pos(last) else {
                return next_start;
            };
            let n = self.numbering.as_mut().expect("set above");
            n.starts.push(next_start);
            n.order.push(next);
        }
    }

    pub fn paragraph_lines(&mut self, pos: Pos) -> Vec<Line> {
        let start = self.line_no_start(pos);
        self.render(pos, start)
    }

    /// Whole paragraphs from `pos` until at least `max_book_lines` text lines (or the end).
    pub fn lines_from(&mut self, pos: Pos, max_book_lines: usize) -> Vec<Line> {
        let mut out = vec![];
        let mut book_lines = 0;
        let mut cur = Some(pos);
        while let Some(p) = cur {
            if book_lines >= max_book_lines {
                break;
            }
            let para = self.paragraph_lines(p);
            book_lines += para.iter().filter(|l| l.is_book).count();
            out.extend(para);
            cur = self.book.next_pos(p);
        }
        out
    }

    /// Up to `rows` rendered lines starting at the view's top line (pos, line_offset).
    pub fn window(&mut self, rows: usize) -> Vec<Line> {
        let mut out = vec![];
        let first = self.paragraph_lines(self.pos);
        let skip = self.line_offset.min(first.len().saturating_sub(1));
        out.extend(first.into_iter().skip(skip));
        let mut cur = self.book.next_pos(self.pos);
        while let Some(p) = cur {
            if out.len() >= rows {
                break;
            }
            out.extend(self.paragraph_lines(p));
            cur = self.book.next_pos(p);
        }
        out.truncate(rows);
        out
    }

    /// Move the view's top line by `n` rendered lines, across paragraphs.
    ///
    /// Stops at the first line of the text; a forward move whose target line
    /// does not exist is not made. Returns whether the view moved.
    pub fn scroll(&mut self, n: isize) -> bool {
        let mut pos = self.pos;
        let here = self.paragraph_lines(pos).len();
        let mut off = self.line_offset.min(here.saturating_sub(1)) as isize + n;
        while off < 0 {
            match self.book.prev_pos(pos) {
                Some(prev) => {
                    pos = prev;
                    off += self.paragraph_lines(pos).len() as isize;
                }
                None => {
                    off = 0;
                    break;
                }
            }
        }
        loop {
            let count = self.paragraph_lines(pos).len() as isize;
            if off < count {
                break;
            }
            match self.book.next_pos(pos) {
                Some(next) => {
                    off -= count;
                    pos = next;
                }
                None => return false,
            }
        }
        let moved = (pos, off as usize) != (self.pos, self.line_offset);
        self.pos = pos;
        self.line_offset = off as usize;
        moved
    }

    pub fn advance(&mut self, n: usize) -> bool {
        self.step(n, true)
    }

    pub fn back(&mut self, n: usize) -> bool {
        self.step(n, false)
    }

    fn step(&mut self, n: usize, forward: bool) -> bool {
        let mut cur = self.pos;
        for _ in 0..n {
            let next = if forward {
                self.book.next_pos(cur)
            } else {
                self.book.prev_pos(cur)
            };
            match next {
                Some(p) => cur = p,
                None => {
                    self.set_pos(cur);
                    return false;
                }
            }
        }
        self.set_pos(cur);
        true
    }

    /// (paragraph number, paragraph count), counting from 1.
    pub fn progress(&self) -> (usize, usize) {
        (
            self.book.index_of(self.pos) + 1,
            self.book.paragraph_count(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::book::{Chapter, Paragraph};
    use crate::masks::get_mask;

    const TEXT: &str = "Bu bir deneme paragrafidir ve oldukca uzun bir cumle icerir. ";

    fn make_book(n: usize) -> Book {
        Book {
            id: "bk".into(),
            chapters: vec![Chapter {
                title: Some("1".into()),
                paragraphs: (0..n)
                    .map(|i| Paragraph::new(format!("{}{i}", TEXT.repeat(6))))
                    .collect(),
            }],
        }
    }

    fn mk(n: usize, noise: u8, width: usize) -> Session {
        Session::new(
            make_book(n),
            get_mask("code", "python").unwrap(),
            noise,
            width,
            (0, 0),
        )
    }

    fn gutter(lines: &[Line]) -> Vec<usize> {
        lines
            .iter()
            .map(|l| l.plain()[..4].trim().parse().unwrap())
            .collect()
    }

    // -- test_session.py --

    #[test]
    fn same_paragraph_renders_identically_after_back_and_forth() {
        let mut s = mk(3, 3, 80);
        let first = s.paragraph_lines((0, 0));
        assert!(!first.is_empty());
        assert!(s.advance(1) && s.back(1));
        assert_eq!(s.paragraph_lines((0, 0)), first);
        assert_eq!(s.pos(), (0, 0));
    }

    #[test]
    fn paragraph_lines_contains_book_lines_and_noise() {
        let lines = mk(3, 5, 80).paragraph_lines((0, 1));
        assert!(lines.iter().any(|l| l.is_book) && lines.iter().any(|l| !l.is_book));
    }

    #[test]
    fn noise_zero_adds_nothing_to_render() {
        let mut s = mk(3, 0, 80);
        let lines = s.paragraph_lines((0, 0));
        let mut fresh = get_mask("code", "python").unwrap();
        let first = fresh.header(&mut seeded("bk"), 80).len() + 1;
        let para = s.book.paragraph((0, 0)).unwrap().clone();
        let expected = fresh.render(&para, (0, 0), 80, &mut seeded("bk:(0, 0):code:0"), first);
        assert_eq!(lines, expected);
    }

    #[test]
    fn line_numbers_run_on_from_the_header_without_gaps() {
        for noise in [0, 3] {
            let mut s = mk(6, noise, 80);
            let header = get_mask("code", "python")
                .unwrap()
                .header(&mut seeded("bk"), 80);
            let mut numbers = gutter(&header);
            for i in 0..6 {
                numbers.extend(gutter(&s.paragraph_lines((0, i))));
            }
            assert_eq!(
                numbers,
                (1..=numbers.len()).collect::<Vec<_>>(),
                "noise {noise}"
            );
        }
    }

    #[test]
    fn line_numbers_do_not_depend_on_visiting_order() {
        let (mut a, mut b) = (mk(6, 3, 80), mk(6, 3, 80));
        let forward: Vec<Vec<Line>> = (0..6).map(|i| a.paragraph_lines((0, i))).collect();
        let mut backward: Vec<Vec<Line>> =
            (0..6).rev().map(|i| b.paragraph_lines((0, i))).collect();
        backward.reverse();
        assert_eq!(forward, backward);
    }

    #[test]
    fn line_numbers_follow_a_width_or_noise_change() {
        let mut s = mk(4, 3, 80);
        let wide = gutter(&s.paragraph_lines((0, 3)))[0];
        s.set_width(40);
        let narrow = gutter(&s.paragraph_lines((0, 3)))[0];
        assert!(narrow > wide); // narrower pane, more wrapped lines above
        assert_eq!(
            s.paragraph_lines((0, 3)),
            mk(4, 3, 40).paragraph_lines((0, 3))
        );
    }

    #[test]
    fn progress_counts_paragraphs_not_noise() {
        let mut s = mk(3, 5, 80);
        assert_eq!(s.progress(), (1, 3));
        s.advance(2);
        assert_eq!(s.progress(), (3, 3));
    }

    #[test]
    fn lines_from_fills_requested_book_lines_and_stops_at_the_end() {
        let mut s = mk(3, 3, 80);
        let lines = s.lines_from((0, 0), 8);
        assert!(lines.iter().filter(|l| l.is_book).count() >= 8);
        let first = s.paragraph_lines((0, 0));
        assert_eq!(&lines[..first.len()], &first[..]);
        assert_eq!(s.lines_from((0, 2), 10_000), s.paragraph_lines((0, 2)));
    }

    #[test]
    fn advance_and_back_stop_at_the_ends() {
        let mut s = mk(3, 3, 80);
        assert!(s.advance(2) && s.pos() == (0, 2));
        assert!(!s.advance(1) && s.pos() == (0, 2));
        assert!(!s.advance(5));
        let mut s = mk(3, 3, 80);
        assert!(!s.back(1) && s.pos() == (0, 0));
        s.advance(2);
        assert!(s.back(2) && s.pos() == (0, 0));
    }

    #[test]
    fn setters_change_rendering() {
        let mut s = mk(3, 0, 80);
        let a = s.paragraph_lines((0, 0));
        s.set_width(40);
        assert_ne!(s.paragraph_lines((0, 0)), a);
        s.set_mask(get_mask("log", "python").unwrap());
        s.set_noise(4);
        assert_eq!((s.noise(), s.mask().name()), (4, "log"));
    }

    #[test]
    fn window_spans_paragraphs_from_line_offset() {
        let mut s = mk(3, 2, 80);
        let stream: Vec<Line> = (0..3).flat_map(|i| s.paragraph_lines((0, i))).collect();
        assert_eq!(s.window(10_000), stream);
        s.line_offset = 3;
        assert_eq!(s.window(5), stream[3..8].to_vec());
    }

    #[test]
    fn scroll_crosses_paragraphs_both_ways_and_clamps() {
        let mut s = mk(3, 2, 80);
        let first = s.paragraph_lines((0, 0)).len();
        let stream = s.window(10_000);
        assert!(s.scroll(first as isize + 1));
        assert_eq!((s.pos(), s.line_offset), ((0, 1), 1));
        assert_eq!(s.window(1), vec![stream[first + 1].clone()]);
        assert!(s.scroll(-2));
        assert_eq!((s.pos(), s.line_offset), ((0, 0), first - 1));
        assert!(s.scroll(-10_000) && (s.pos(), s.line_offset) == ((0, 0), 0));
        assert!(!s.scroll(-1));
        assert!(!s.scroll(stream.len() as isize)); // no such line: stay put
        assert_eq!((s.pos(), s.line_offset), ((0, 0), 0));
        assert!(
            s.scroll(stream.len() as isize - 1)
                && s.window(10) == vec![stream.last().unwrap().clone()]
        );
    }

    #[test]
    fn setting_pos_resets_line_offset() {
        let mut s = mk(3, 3, 80);
        s.scroll(2);
        assert_eq!(s.line_offset, 2);
        s.set_pos((0, 1));
        assert_eq!(s.line_offset, 0);
    }

    #[test]
    fn deep_jumps_count_lines_without_drawing_them() {
        let mut s = mk(200, 3, 80);
        let lines = s.paragraph_lines((0, 199));
        let mut fresh = mk(200, 3, 80);
        let total: usize = (0..199).map(|i| fresh.paragraph_lines((0, i)).len()).sum();
        let header = get_mask("code", "python")
            .unwrap()
            .header(&mut seeded("bk"), 80)
            .len();
        assert_eq!(gutter(&lines)[0], header + total + 1);
    }

    // -- test_looks.py / test_code.py parts that need a session --

    fn sample_book(width_words: usize) -> Book {
        Book {
            id: "bk".into(),
            chapters: vec![Chapter {
                title: None,
                paragraphs: (0..8)
                    .map(|i| {
                        Paragraph::new(format!("Paragraf {i} {}", "kelime ".repeat(width_words)))
                    })
                    .collect(),
            }],
        }
    }

    const TITLE: &str = "Zehirli Elmanin Sirri";

    /// `chapters` chapters of `per` paragraphs each, titled so a leak would show,
    /// behind an empty chapter (front matter) that has no text.
    fn chaptered_book(chapters: usize, per: usize, width_words: usize) -> Book {
        let mut all = vec![Chapter {
            title: Some("Kapak".into()),
            paragraphs: vec![],
        }];
        all.extend((0..chapters).map(|c| {
            Chapter {
                title: Some(format!("{TITLE} {c}")),
                paragraphs: (0..per)
                    .map(|i| {
                        Paragraph::new(format!(
                            "Paragraf {c}.{i} {}",
                            "kelime ".repeat(width_words)
                        ))
                    })
                    .collect(),
            }
        }));
        Book {
            id: "bk".into(),
            chapters: all,
        }
    }

    /// Every position in reading order.
    fn positions(book: &Book) -> Vec<Pos> {
        let first = book.chapter_list()[0].0;
        std::iter::successors(Some((first, 0)), |p| book.next_pos(*p)).collect()
    }

    #[test]
    fn every_code_line_has_a_gutter_including_filler() {
        for lang in crate::masks::LANGS {
            let book = chaptered_book(3, 3, 40);
            let order = positions(&book);
            let mut s = Session::new(book, get_mask("code", lang).unwrap(), 3, 60, (1, 0));
            let mut all = vec![];
            for p in order {
                let block = s.paragraph_lines(p);
                assert!(block.iter().all(|l| l.plain().chars().count() <= 60));
                all.extend(gutter(&block));
            }
            // continuous across paragraphs, filler and chapter transitions alike
            assert!(all.windows(2).all(|w| w[1] == w[0] + 1), "{lang}: {all:?}");
        }
    }

    #[test]
    fn chapter_starts_after_the_first_open_with_the_masks_transition() {
        for name in crate::masks::MASK_NAMES {
            let langs: &[&str] = if *name == "code" {
                crate::masks::LANGS
            } else {
                &["python"]
            };
            for lang in langs {
                for width in [40, 60, 120] {
                    let book = chaptered_book(3, 3, 25);
                    let order = positions(&book);
                    let mut s = Session::new(book, get_mask(name, lang).unwrap(), 2, width, (1, 0));
                    for p in order {
                        let start = s.line_no_start(p);
                        let lines = s.paragraph_lines(p);
                        let tr = s.mask().transition(p.0, width, start);
                        let has = lines.len() >= tr.len() && lines[..tr.len()] == tr[..];
                        // chapter 1 is the first with text: the header opens it instead
                        let want = p.1 == 0 && p.0 > 1;
                        assert_eq!(has, want, "{name}/{lang} at {width}, {p:?}");
                        let para = s.book.paragraph(p).unwrap();
                        let bare = s.mask().line_count(para, width) + s.noise_count(p);
                        let extra = if want { tr.len() } else { 0 };
                        assert_eq!(lines.len(), bare + extra, "{name}/{lang} {p:?}");
                        assert_eq!(s.line_count(p), lines.len(), "{name}/{lang} {p:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn transitions_never_show_a_chapter_title() {
        for name in crate::masks::MASK_NAMES {
            let book = chaptered_book(4, 2, 10);
            let order = positions(&book);
            let mut s = Session::new(book, get_mask(name, "python").unwrap(), 3, 120, (1, 0));
            for p in order {
                for l in s.paragraph_lines(p).iter().filter(|l| !l.is_book) {
                    let low = l.plain().to_lowercase();
                    assert!(
                        !low.contains("zehirli") && !low.contains("elma"),
                        "{name}: {low}"
                    );
                }
            }
        }
    }

    #[test]
    fn code_does_not_repeat_a_definition_on_one_screen() {
        for lang in crate::masks::LANGS {
            let mut s = Session::new(
                sample_book(30),
                get_mask("code", lang).unwrap(),
                3,
                100,
                (0, 0),
            );
            for first in 0..6 {
                let screen: Vec<Line> = [first, first + 1]
                    .iter()
                    .flat_map(|i| s.paragraph_lines((0, *i)))
                    .collect();
                let code: Vec<String> = screen
                    .iter()
                    .filter(|l| !l.is_book && l.plain().chars().count() > 12)
                    .map(|l| l.plain().chars().skip(5).collect())
                    .collect();
                let sigs: Vec<&String> = code
                    .iter()
                    .filter(|c| {
                        !c.starts_with("    ") && !c.starts_with("  ") && !c.starts_with(['}', ')'])
                    })
                    .collect();
                let unique: std::collections::HashSet<&&String> = sigs.iter().collect();
                assert_eq!(sigs.len(), unique.len(), "{lang}: {sigs:?}");
            }
        }
    }

    /// Words of the text shown in `lines`, cut at `width` cells like the screen does.
    fn screen_words(lines: &[Line], width: usize) -> Vec<String> {
        let mut out = vec![];
        for l in lines.iter().filter(|l| l.is_book) {
            let mut col = 0;
            for (text, style) in &l.segments {
                if style.split_whitespace().any(|t| t == "book") {
                    let visible: String = text.chars().take(width.saturating_sub(col)).collect();
                    out.extend(visible.split_whitespace().map(String::from));
                }
                col += text.chars().count();
            }
        }
        out
    }

    #[test]
    fn paging_shows_every_word_once_in_order() {
        // as the UI pages: show `rows` lines, then move the top by exactly that many
        let paras: Vec<Paragraph> = (0..8)
            .map(|p| {
                Paragraph::new(
                    (0..60)
                        .map(|w| format!("p{p}w{w}"))
                        .collect::<Vec<_>>()
                        .join(" "),
                )
            })
            .collect();
        let words: Vec<String> = paras
            .iter()
            .flat_map(|p| p.text.split_whitespace().map(String::from))
            .collect();
        // three chapters, so pages cross chapter transitions too
        let mut paras = paras.into_iter();
        let book = Book {
            id: "bk".into(),
            chapters: [3, 2, 3]
                .iter()
                .map(|n| Chapter {
                    title: None,
                    paragraphs: paras.by_ref().take(*n).collect(),
                })
                .collect(),
        };
        let rows = 23;
        for name in crate::masks::MASK_NAMES {
            for width in [60, 40] {
                for noise in [0, 2] {
                    let mut s = Session::new(
                        book.clone(),
                        get_mask(name, "python").unwrap(),
                        noise,
                        width,
                        (0, 0),
                    );
                    let mut seen = vec![];
                    for _ in 0..600 {
                        let text = s.window(rows + 1);
                        let more_below = text.len() > rows;
                        let shown = &text[..text.len().min(rows)];
                        seen.extend(screen_words(shown, width));
                        if !more_below {
                            break;
                        }
                        s.scroll(shown.len() as isize);
                    }
                    assert_eq!(seen, words, "{name} width {width} noise {noise}");
                }
            }
        }
    }

    #[test]
    fn log_time_never_runs_backwards_across_text_and_filler() {
        // chapters, so the transition lines are checked between them too
        let book = chaptered_book(3, 3, 30);
        let order = positions(&book);
        let mut s = Session::new(book, get_mask("log", "python").unwrap(), 5, 120, (1, 0));
        let mut times = vec![];
        for pos in order {
            for l in s.paragraph_lines(pos) {
                let p = l.plain();
                assert_eq!(&p[23..25], "Z ", "{p}");
                times.push(p[..23].to_string());
            }
        }
        assert!(times.len() > 40);
        assert!(times.windows(2).all(|w| w[0] < w[1]));
    }

    // -- deep jumps do not wrap the whole book again and again --

    #[test]
    fn deep_jumps_wrap_each_paragraph_once_per_room() {
        use crate::text::WRAPS;
        let wraps = || WRAPS.with(|c| c.get());
        let n = 300;
        let last = (0, n - 1);
        let mut s = Session::new(
            make_book(n),
            get_mask("code", "python").unwrap(),
            3,
            130,
            last,
        );

        let before = wraps();
        let first = s.paragraph_lines(last);
        let cold = wraps() - before;
        assert!(
            cold <= n,
            "a deep jump wraps at most every paragraph: {cold}"
        );

        // the same jump, scrolling around the end: nothing is counted again
        let before = wraps();
        assert_eq!(s.paragraph_lines(last), first);
        s.set_pos(last);
        s.back(5);
        s.window(40);
        assert_eq!(wraps(), before, "a repeated jump recounts nothing");

        // numbering is rebuilt after these, but no paragraph is wrapped again
        // (the body is as wide as the code mask allows, so the room is the same)
        s.set_width(140);
        s.paragraph_lines(last);
        s.set_mask(get_mask("code", "python").unwrap());
        s.paragraph_lines(last);
        s.set_noise(1);
        s.paragraph_lines(last);
        assert_eq!(wraps(), before, "same room, same wrapping");

        // a narrower room really does change the wrapping
        s.set_width(60);
        s.paragraph_lines(last);
        let narrow = wraps() - before;
        assert!(
            (1..=n).contains(&narrow),
            "wrapped {narrow} paragraphs for a new room"
        );
    }
}
