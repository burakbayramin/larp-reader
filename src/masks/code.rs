//! A documented source file (`masks/code.py`): paragraphs are doc comments, filler is real code.

use std::collections::VecDeque;

use super::base::{Line, Segment, crop, seg};
use super::palette::{COMMENT, GUTTER, highlight};
use super::{LANGS, Mask, Rng, section, snippets};
use crate::book::{Paragraph, Pos};
use crate::text::{ragged, ragged_count};
use unicode_width::UnicodeWidthStr;

/// "%4d " gutter.
const NUM_WIDTH: usize = 5;
/// Declarations a paragraph may use: its own plus filler, so neighbours never share one.
const ITEMS_PER_BLOCK: usize = 3;

fn file(lang: &str) -> &'static str {
    match lang {
        "typescript" => "src/indexer.ts",
        "go" => "indexer/indexer.go",
        "rust" => "src/indexer.rs",
        "java" => "src/main/java/Indexer.java",
        "csharp" => "src/Indexer.cs",
        _ => "src/indexer.py",
    }
}

/// (prefix for each text line, opening lines, closing lines) of a doc comment.
fn frame(
    lang: &str,
) -> (
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
) {
    match lang {
        "rust" => ("/// ", &[], &[]),
        "go" => ("// ", &[], &[]),
        "typescript" => (" * ", &["/**"], &[" */"]),
        "java" => ("     * ", &["    /**"], &["     */"]),
        "csharp" => ("    /// ", &[], &[]),
        _ => ("# ", &[], &[]),
    }
}

pub struct CodeMask {
    lang: &'static str,
    /// Number for the next filler line: continues the last rendered block.
    next_no: usize,
    /// Pending lines of `noise()`.
    stream: VecDeque<Vec<Segment>>,
    /// Where in the corpus the next filler comes from: walking through it keeps
    /// the same definition from turning up twice on one screen.
    cursor: usize,
}

impl CodeMask {
    pub fn new(lang: &str) -> Result<CodeMask, String> {
        let lang = LANGS
            .iter()
            .find(|l| **l == lang)
            .ok_or_else(|| format!("unknown language: {lang:?}"))?;
        Ok(CodeMask {
            lang,
            next_no: 1,
            stream: VecDeque::new(),
            cursor: 0,
        })
    }

    fn line(&self, n: usize, segments: Vec<Segment>, width: usize, pos: Option<Pos>) -> Line {
        let mut all = vec![seg(format!("{n:>4} "), GUTTER)];
        all.extend(segments);
        Line {
            segments: crop(all, width.max(1)),
            is_book: pos.is_some(),
            pos,
        }
    }

    fn numbered(&mut self, rows: Vec<Vec<Segment>>, width: usize) -> Vec<Line> {
        let start = self.next_no;
        self.next_no += rows.len();
        rows.into_iter()
            .enumerate()
            .map(|(i, r)| self.line(start + i, r, width, None))
            .collect()
    }

    fn body_width(&self, width: usize) -> usize {
        width
            .saturating_sub(NUM_WIDTH + frame(self.lang).0.width())
            .max(1)
    }
}

impl Mask for CodeMask {
    fn name(&self) -> &'static str {
        "code"
    }

    fn header(&mut self, _rng: &mut Rng, width: usize) -> Vec<Line> {
        let rows = highlight(self.lang, snippets::header(self.lang));
        rows.into_iter()
            .enumerate()
            .map(|(i, r)| self.line(i + 1, r, width, None))
            .collect()
    }

    /// Next line of an endless stream of complete definitions.
    fn noise(&mut self, _rng: &mut Rng, width: usize) -> Line {
        if self.stream.is_empty() {
            let blocks = snippets::blocks(self.lang);
            self.stream.push_back(vec![]);
            self.stream
                .extend(highlight(self.lang, blocks[self.cursor % blocks.len()]));
            self.cursor += 1;
        }
        let row = self.stream.pop_front().unwrap_or_default();
        self.numbered(vec![row], width).remove(0)
    }

    /// Exactly `count` filler lines made of whole definitions, never a cut-off one.
    fn noise_block(&mut self, count: usize, _rng: &mut Rng, width: usize) -> Option<Vec<Line>> {
        // each paragraph draws from its own third of the corpus (the first three
        // blocks are short ones, one per third), so three paragraphs in a row
        // never show the same definition
        let all: Vec<Vec<Vec<Segment>>> = snippets::blocks(self.lang)
            .iter()
            .map(|b| highlight(self.lang, b))
            .collect();
        let mut pool: Vec<&Vec<Vec<Segment>>> =
            all.iter().skip(self.cursor % 3).step_by(3).collect();
        let turn = self.cursor / 3 % pool.len();
        pool.rotate_left(turn);

        let mut chosen: Vec<&Vec<Vec<Segment>>> = vec![];
        let mut left = count;
        while let Some(b) = pool.iter().find(|b| b.len() < left && !chosen.contains(b)) {
            left -= b.len() + 1;
            chosen.push(b);
        }
        let items = snippets::items(self.lang);
        let mut rows: Vec<Vec<Segment>> = (0..left.min(ITEMS_PER_BLOCK - 1))
            .map(|i| {
                let item = items[(self.cursor * ITEMS_PER_BLOCK + 1 + i) % items.len()];
                highlight(self.lang, item).swap_remove(0)
            })
            .collect();
        rows.resize(left, vec![]);
        for b in chosen {
            rows.push(vec![]);
            rows.extend(b.iter().cloned());
        }
        Some(self.numbered(rows, width))
    }

    fn line_count(&self, para: &Paragraph, width: usize) -> usize {
        let (_, opening, closing) = frame(self.lang);
        ragged_count(&para.text, self.body_width(width)) + opening.len() + closing.len() + 2
    }

    fn render(
        &mut self,
        para: &Paragraph,
        pos: Pos,
        width: usize,
        _rng: &mut Rng,
        line_no_start: usize,
    ) -> Vec<Line> {
        let (prefix, opening, closing) = frame(self.lang);
        let mut n = line_no_start;
        let mut out = vec![];
        let mut emit = |out: &mut Vec<Line>, this: &Self, segments: Vec<Segment>, book: bool| {
            out.push(this.line(n, segments, width, book.then_some(pos)));
            n += 1;
        };
        emit(&mut out, self, vec![], false);
        for text in opening {
            emit(&mut out, self, vec![seg(*text, COMMENT)], false);
        }
        for text in ragged(&para.text, self.body_width(width)) {
            emit(
                &mut out,
                self,
                vec![seg(prefix, COMMENT), seg(text, COMMENT.book())],
                true,
            );
        }
        for text in closing {
            emit(&mut out, self, vec![seg(*text, COMMENT)], false);
        }
        self.cursor = pos.0 * 5 + pos.1;
        let items = snippets::items(self.lang);
        let item = highlight(
            self.lang,
            items[self.cursor * ITEMS_PER_BLOCK % items.len()],
        )
        .swap_remove(0);
        emit(&mut out, self, item, false);
        self.next_no = n;
        out
    }

    /// The next file of a concatenated listing: a gap, then the file's banner comment.
    fn transition(&self, chapter: usize, width: usize, line_no_start: usize) -> Vec<Line> {
        let name = section::name(chapter, 0);
        let pkg = section::name(chapter, 1);
        let banner = match self.lang {
            "typescript" => format!("// --- src/{pkg}/{name}.ts ---"),
            "go" => format!("// --- {pkg}/{name}.go ---"),
            "rust" => format!("// --- src/{pkg}/{name}.rs ---"),
            "java" => format!(
                "// --- src/main/java/{pkg}/{}.java ---",
                section::capitalised(name)
            ),
            "csharp" => format!("// --- src/{}.cs ---", section::capitalised(name)),
            _ => format!("# --- {pkg}/{name}.py ---"),
        };
        vec![
            self.line(line_no_start, vec![], width, None),
            self.line(line_no_start + 1, vec![seg(banner, COMMENT)], width, None),
        ]
    }

    /// An editor status line: file, position, how far down.
    fn progress_label(&self, done: usize, total: usize, _width: usize) -> String {
        let pct = (done * 100).checked_div(total).unwrap_or(0).min(100);
        format!(" {}  {done}/{total}  {pct}%", file(self.lang))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::masks::palette::COMMENT;
    use rand::SeedableRng;

    fn para() -> Paragraph {
        Paragraph::new("Oda dar ve basıktı, tavanı alçaktı. ".repeat(4))
    }

    fn plain(lang: &str, start: usize) -> Vec<String> {
        let mut m = CodeMask::new(lang).unwrap();
        m.render(&para(), (0, 0), 60, &mut Rng::seed_from_u64(1), start)
            .iter()
            .map(Line::plain)
            .collect()
    }

    // -- test_code.py --

    #[test]
    fn python_uses_hash_comments_and_rust_slashes() {
        assert!(plain("python", 1).iter().any(|l| l.contains("# ")));
        let mut m = CodeMask::new("rust").unwrap();
        let book: Vec<String> = m
            .render(&para(), (0, 0), 60, &mut Rng::seed_from_u64(1), 1)
            .iter()
            .filter(|l| l.is_book)
            .map(Line::plain)
            .collect();
        assert!(!book.is_empty() && book.iter().all(|t| t.contains("/// ")));
    }

    #[test]
    fn unknown_lang_is_an_error() {
        assert!(CodeMask::new("cobol").is_err());
    }

    #[test]
    fn line_numbers_start_at_given_value_and_increase() {
        let lines = plain("python", 40);
        let nums: Vec<usize> = lines
            .iter()
            .map(|t| t[..4].trim().parse().unwrap())
            .collect();
        assert_eq!(nums, (40..40 + lines.len()).collect::<Vec<_>>());
        assert_eq!(plain("python", 40), lines);
    }

    #[test]
    fn every_lang_renders_and_fits() {
        for lang in LANGS {
            let mut m = CodeMask::new(lang).unwrap();
            assert!(!m.header(&mut Rng::seed_from_u64(1), 60).is_empty());
            let out = m.render(&para(), (0, 0), 60, &mut Rng::seed_from_u64(1), 1);
            assert!(
                !out.is_empty() && out.iter().all(|l| l.plain().chars().count() <= 60),
                "{lang}"
            );
        }
    }

    #[test]
    fn progress_label_is_an_editor_status_line() {
        assert_eq!(
            CodeMask::new("rust").unwrap().progress_label(142, 380, 50),
            " src/indexer.rs  142/380  37%"
        );
        assert_eq!(
            CodeMask::new("python").unwrap().progress_label(0, 0, 50),
            " src/indexer.py  0/0  0%"
        );
    }

    #[test]
    fn standalone_noise_lines_have_gutter() {
        let mut m = CodeMask::new("python").unwrap();
        let mut rng = Rng::seed_from_u64(4);
        for _ in 0..20 {
            let l = m.noise(&mut rng, 30).plain();
            assert!(
                l[..4].trim().parse::<usize>().is_ok() && l.chars().count() <= 30,
                "{l:?}"
            );
        }
    }

    // -- test_looks.py (the parts that need no session) --

    #[test]
    fn code_text_is_a_comment_in_the_editor_comment_colour() {
        for lang in LANGS {
            let mut m = CodeMask::new(lang).unwrap();
            let lines = m.render(&para(), (0, 0), 80, &mut Rng::seed_from_u64(1), 1);
            let text: Vec<&Line> = lines.iter().filter(|l| l.is_book).collect();
            assert!(!text.is_empty());
            for l in text {
                let n = l.segments.len();
                assert_eq!(l.segments[n - 1].1, COMMENT.book());
                assert_eq!(l.segments[n - 2].1, COMMENT); // the comment marker
            }
        }
    }

    #[test]
    fn code_filler_block_has_exact_size_and_no_cut_off_definition() {
        for lang in ["rust", "typescript", "go", "java", "csharp"] {
            let mut m = CodeMask::new(lang).unwrap();
            for count in 0..=10 {
                for seed in 0..20 {
                    let block = m
                        .noise_block(count, &mut Rng::seed_from_u64(seed), 200)
                        .unwrap();
                    assert_eq!(block.len(), count);
                    let body: String = block
                        .iter()
                        .map(|l| l.plain().chars().skip(5).collect::<String>() + "\n")
                        .collect();
                    assert_eq!(
                        body.matches('{').count(),
                        body.matches('}').count(),
                        "{lang}\n{body}"
                    );
                    assert_eq!(
                        body.matches('(').count(),
                        body.matches(')').count(),
                        "{lang}\n{body}"
                    );
                }
            }
        }
    }

    #[test]
    fn code_filler_block_uses_whole_definitions_when_there_is_room() {
        let mut m = CodeMask::new("rust").unwrap();
        for seed in 0..20 {
            let block = m
                .noise_block(10, &mut Rng::seed_from_u64(seed), 200)
                .unwrap();
            assert!(block.iter().any(|l| l.plain().contains('{')));
        }
    }

    #[test]
    fn code_filler_is_syntax_coloured_not_one_colour() {
        for lang in LANGS {
            let mut m = CodeMask::new(lang).unwrap();
            let mut styles = std::collections::HashSet::new();
            let mut mixed = false;
            for i in 0..8 {
                m.render(&para(), (0, i), 100, &mut Rng::seed_from_u64(1), 1);
                for l in m
                    .noise_block(10, &mut Rng::seed_from_u64(i as u64), 100)
                    .unwrap()
                {
                    let own: std::collections::HashSet<&crate::masks::Look> =
                        l.segments[1..].iter().map(|(_, s)| s).collect();
                    mixed |= own.len() >= 3;
                    styles.extend(l.segments.iter().map(|(_, s)| *s));
                }
            }
            assert!(styles.len() >= 5 && mixed, "{lang}: {styles:?}");
        }
    }
}
