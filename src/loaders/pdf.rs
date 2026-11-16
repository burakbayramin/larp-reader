//! PDF (`loaders/mupdf.py`), through the pure-Rust `pdf_oxide`.
//!
//! A page is read in reading order: it is cut into horizontal strips at tall
//! blank bands, each strip is split into columns at a vertical gutter (lines
//! crossing the gutter, such as titles, stay in place between the column
//! stretches), and each column is read top to bottom, left to right. A column's
//! lines become paragraphs at vertical gaps, at first-line indents and after
//! lines that stop short of the column's edge on a sentence end. Images sit
//! among the blocks by position, and the rest follows the Python version:
//! - a block that opens or closes at least half the pages (digits ignored) is a
//!   running header or footer and is dropped, once there are 3 pages or more;
//! - a word split by a soft hyphen at a line end is joined;
//! - outline entries split the text into chapters;
//! - no text at all means a scan: "no text layer".

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use pdf_oxide::document::PdfDocument;
use pdf_oxide::layout::TextLine;
use pdf_oxide::outline::{Destination, OutlineItem};

use super::{Loader, has_suffix};
use crate::book::{Book, Chapter, Paragraph, book_id};
use crate::errors::FccError;
use crate::text::normalize_paragraphs;

const MIN_PAGES_FOR_HEADER_STRIP: usize = 3;
/// Lines further apart than this share of a line's height start a new block.
const BLOCK_GAP: f32 = 0.5;
/// A blank band this many line heights tall cuts the page across its whole
/// width (title / body / footer) before columns are looked for.
const BAND_GAP: f32 = 2.5;
/// A gutter is at least this many points wide, and this many line heights.
const MIN_GUTTER_PT: f32 = 8.0;
const MIN_GUTTER_H: f32 = 0.8;
/// At most this share of the lines may cross a gutter (titles spanning the columns).
const MAX_SPAN_SHARE: f32 = 0.15;
/// A column holds at least this many lines and this share of the text width.
const MIN_COLUMN_LINES: usize = 3;
const MIN_COLUMN_SHARE: f32 = 0.2;
/// Columns inside columns are looked for down to this depth.
const MAX_DEPTH: usize = 4;
/// A line starting this many line heights right of its column's edge opens a paragraph.
const INDENT: f32 = 0.8;
/// A line ending this share of the column width short of its edge, on sentence
/// punctuation, closes a paragraph.
const SHORT_LINE: f32 = 0.15;
/// Slack for comparing coordinates, in points.
const EPS: f32 = 0.5;

pub struct PdfLoader;

/// What is on a page, in reading order.
#[derive(Debug, Clone, PartialEq)]
enum Block {
    Text(String),
    Image,
}

/// A block with where it sits on the page: its top and horizontal extent.
#[derive(Debug)]
struct Placed {
    top: f32,
    x0: f32,
    x1: f32,
    block: Block,
}

/// The words of one line within one column, in PDF space (y grows upward).
#[derive(Debug, Clone)]
struct Piece {
    /// (left, right, text) of each word, in line order
    words: Vec<(f32, f32, String)>,
    top: f32,
    bottom: f32,
}

impl Piece {
    fn from_line(line: &TextLine) -> Self {
        let words = if line.words.is_empty() {
            let b = &line.bbox;
            vec![(b.x, b.x + b.width, line.text.clone())]
        } else {
            line.words
                .iter()
                .map(|w| (w.bbox.x, w.bbox.x + w.bbox.width, w.text.clone()))
                .collect()
        };
        Piece {
            words,
            top: line.bbox.y + line.bbox.height,
            bottom: line.bbox.y,
        }
    }

    fn with_words(&self, words: Vec<(f32, f32, String)>) -> Self {
        Piece {
            words,
            top: self.top,
            bottom: self.bottom,
        }
    }

    fn x0(&self) -> f32 {
        self.words.iter().map(|w| w.0).fold(f32::MAX, f32::min)
    }

    fn x1(&self) -> f32 {
        self.words.iter().map(|w| w.1).fold(f32::MIN, f32::max)
    }

    fn height(&self) -> f32 {
        self.top - self.bottom
    }

    fn text(&self) -> String {
        let words: Vec<&str> = self.words.iter().map(|w| w.2.as_str()).collect();
        words.join(" ")
    }
}

fn unsupported(e: impl std::fmt::Display) -> FccError {
    FccError::UnsupportedFormat(format!("dosya açılamadı: {e}"))
}

/// The text with whitespace squashed and digits blanked: page numbers do not make a footer differ.
fn mask(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .map(|c| if c.is_ascii_digit() { '#' } else { c })
        .collect()
}

fn sort_top_down(pieces: &mut [Piece]) {
    pieces.sort_by(|a, b| b.top.total_cmp(&a.top).then(a.x0().total_cmp(&b.x0())));
}

fn median_height(pieces: &[Piece]) -> f32 {
    let mut hs: Vec<f32> = pieces.iter().map(Piece::height).collect();
    hs.sort_by(f32::total_cmp);
    hs.get(hs.len() / 2).copied().unwrap_or(0.0)
}

/// The value at share `q` of the way through `xs` once sorted.
fn quantile(mut xs: Vec<f32>, q: f32) -> f32 {
    xs.sort_by(f32::total_cmp);
    let i = (xs.len().saturating_sub(1) as f32 * q) as usize;
    xs.get(i).copied().unwrap_or(0.0)
}

/// Horizontal strips, top to bottom, cut where a tall blank band runs across
/// the whole width; each strip comes back sorted top to bottom.
fn bands(mut pieces: Vec<Piece>) -> Vec<Vec<Piece>> {
    sort_top_down(&mut pieces);
    let h = median_height(&pieces);
    let mut out: Vec<Vec<Piece>> = vec![];
    let mut floor = f32::MAX; // lowest bottom of the current strip
    for p in pieces {
        if let Some(band) = out.last_mut()
            && floor - p.top <= BAND_GAP * h
        {
            floor = floor.min(p.bottom);
            band.push(p);
            continue;
        }
        floor = p.bottom;
        out.push(vec![p]);
    }
    out
}

/// Where a piece goes once a gutter divides its strip.
enum Side {
    Left(Piece),
    Right(Piece),
    Span(Piece),
}

/// Puts a piece on one side of the gutter `g0..g1`, cuts in two a line the text
/// extractor ran across it, or keeps whole a line that really crosses it (a title).
fn split(p: Piece, g0: f32, g1: f32, min_gap: f32) -> Vec<Side> {
    let (x0, x1) = (p.x0(), p.x1());
    if x0 >= g0 - EPS && x1 <= g1 + EPS {
        return vec![Side::Span(p)];
    }
    if x1 <= g1 + EPS {
        return vec![Side::Left(p)];
    }
    if x0 >= g0 - EPS {
        return vec![Side::Right(p)];
    }
    let mid = (g0 + g1) / 2.0;
    let (l, r): (Vec<_>, Vec<_>) = p
        .words
        .iter()
        .cloned()
        .partition(|w| (w.0 + w.1) / 2.0 < mid);
    let (Some(l_end), Some(r_start)) = (
        l.iter().map(|w| w.1).reduce(f32::max),
        r.iter().map(|w| w.0).reduce(f32::min),
    ) else {
        return vec![Side::Span(p)];
    };
    if r_start - l_end >= min_gap.max(MIN_GUTTER_H * p.height()) {
        vec![Side::Left(p.with_words(l)), Side::Right(p.with_words(r))]
    } else {
        vec![Side::Span(p)]
    }
}

/// The widest blank vertical strip (crossed by few lines) with a column on
/// each side: `(g0, g1, min_gap)`.
fn find_gutter(pieces: &[Piece]) -> Option<(f32, f32, f32)> {
    if pieces.len() < 2 * MIN_COLUMN_LINES {
        return None;
    }
    let xmin = pieces.iter().map(Piece::x0).fold(f32::MAX, f32::min);
    let xmax = pieces.iter().map(Piece::x1).fold(f32::MIN, f32::max);
    let width = xmax - xmin;
    if width.is_nan() || width <= 0.0 || width > 100_000.0 {
        return None;
    }
    let origin = xmin.floor();
    let n_bins = (xmax.ceil() - origin) as usize;
    // how many lines have a word over each point of the width
    let mut cover = vec![0usize; n_bins];
    let mut seen = vec![usize::MAX; n_bins];
    for (i, p) in pieces.iter().enumerate() {
        for w in &p.words {
            let a = ((w.0.floor() - origin).max(0.0) as usize).min(n_bins);
            let b = ((w.1.ceil() - origin).max(0.0) as usize).min(n_bins);
            for k in a..b {
                if seen[k] != i {
                    seen[k] = i;
                    cover[k] += 1;
                }
            }
        }
    }
    let tolerance = (pieces.len() as f32 * MAX_SPAN_SHARE) as usize;
    let min_gap = MIN_GUTTER_PT.max(MIN_GUTTER_H * median_height(pieces));
    let mut best: Option<(f32, f32)> = None;
    let mut k = 0;
    while k < n_bins {
        if cover[k] > tolerance {
            k += 1;
            continue;
        }
        let start = k;
        while k < n_bins && cover[k] <= tolerance {
            k += 1;
        }
        let (g0, g1) = (origin + start as f32, origin + k as f32);
        if best.is_none_or(|(b0, b1)| g1 - g0 > b1 - b0)
            && g1 - g0 >= min_gap
            && g0 - xmin >= MIN_COLUMN_SHARE * width
            && xmax - g1 >= MIN_COLUMN_SHARE * width
            && has_columns(pieces, g0, g1, min_gap)
        {
            best = Some((g0, g1));
        }
    }
    best.map(|(g0, g1)| (g0, g1, min_gap))
}

/// Whether enough lines sit on both sides of the gutter, side by side.
fn has_columns(pieces: &[Piece], g0: f32, g1: f32, min_gap: f32) -> bool {
    let (mut left, mut right) = (vec![], vec![]);
    for p in pieces {
        for s in split(p.clone(), g0, g1, min_gap) {
            match s {
                Side::Left(p) => left.push((p.bottom, p.top)),
                Side::Right(p) => right.push((p.bottom, p.top)),
                Side::Span(_) => {}
            }
        }
    }
    let extent = |v: &[(f32, f32)]| {
        v.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &(b, t)| {
            (lo.min(b), hi.max(t))
        })
    };
    let ((l_lo, l_hi), (r_lo, r_hi)) = (extent(&left), extent(&right));
    left.len() >= MIN_COLUMN_LINES && right.len() >= MIN_COLUMN_LINES && l_lo < r_hi && r_lo < l_hi
}

/// The page's lines as single columns in reading order: strips top to bottom,
/// within a strip the columns left to right, and lines crossing the gutter
/// (titles) where they stand between the column stretches.
fn layout(pieces: Vec<Piece>, depth: usize) -> Vec<Vec<Piece>> {
    let mut out = vec![];
    for band in bands(pieces) {
        let gutter = if depth < MAX_DEPTH {
            find_gutter(&band)
        } else {
            None
        };
        let Some((g0, g1, min_gap)) = gutter else {
            out.push(band);
            continue;
        };
        let mut sides: Vec<Side> = band
            .into_iter()
            .flat_map(|p| split(p, g0, g1, min_gap))
            .collect();
        let top = |s: &Side| match s {
            Side::Left(p) | Side::Right(p) | Side::Span(p) => p.top,
        };
        sides.sort_by(|a, b| top(b).total_cmp(&top(a)));
        let (mut left, mut right, mut span) = (vec![], vec![], vec![]);
        for s in sides {
            let (p, to_left) = match s {
                Side::Span(p) => {
                    out.extend(layout(std::mem::take(&mut left), depth + 1));
                    out.extend(layout(std::mem::take(&mut right), depth + 1));
                    span.push(p);
                    continue;
                }
                Side::Left(p) => (p, true),
                Side::Right(p) => (p, false),
            };
            out.extend(layout(std::mem::take(&mut span), depth + 1));
            if to_left {
                left.push(p);
            } else {
                right.push(p);
            }
        }
        out.extend(layout(span, depth + 1));
        out.extend(layout(left, depth + 1));
        out.extend(layout(right, depth + 1));
    }
    out
}

/// Whether a line closes a sentence (closing quotes and brackets aside).
fn ends_sentence(text: &str) -> bool {
    text.trim_end()
        .trim_end_matches(['"', '\'', '”', '’', '»', ')', ']'])
        .ends_with(['.', '!', '?', '…', ':'])
}

/// Whether a line ends on a hyphen, i.e. its last word goes on in the next line.
fn ends_hyphen(text: &str) -> bool {
    text.trim_end().ends_with(['-', '\u{ad}'])
}

/// Whether a text opens with a lowercase letter (a sentence going on).
fn starts_lowercase(text: &str) -> bool {
    text.trim_start()
        .chars()
        .next()
        .is_some_and(char::is_lowercase)
}

/// One column's lines (top to bottom) as blocks: a new one starts at a vertical
/// gap, at a first-line indent, and after a line that stops well short of the
/// column's right edge on a sentence end; never after a hyphenated line end.
fn column_blocks(lines: &[Piece]) -> Vec<Placed> {
    // quantiles, so a stray wide or outdented line does not move the edges
    let left = quantile(lines.iter().map(Piece::x0).collect(), 0.25);
    let right = quantile(lines.iter().map(Piece::x1).collect(), 0.75);
    let indented = |p: &Piece| p.x0() - left >= INDENT * p.height();
    let mut out = vec![];
    let mut cur: Option<(Placed, Vec<String>)> = None;
    let mut prev: Option<(&Piece, String)> = None;
    for line in lines {
        let text = line.text();
        if let Some((p, p_text)) = &prev {
            let h = p.height().max(line.height());
            let gap = p.bottom - line.top > BLOCK_GAP * h;
            // a piece beside the previous one (same row) goes on with it
            let new_row = line.top - p.bottom < BLOCK_GAP * h;
            let same_indent = indented(p) && (line.x0() - p.x0()).abs() < INDENT * h / 2.0;
            let indent = indented(line) && !same_indent;
            let short_end = right - p.x1() >= SHORT_LINE * (right - left) && ends_sentence(p_text);
            if (gap || (new_row && !ends_hyphen(p_text) && (indent || short_end)))
                && let Some((placed, texts)) = cur.take()
            {
                out.push(Placed {
                    block: Block::Text(texts.join("\n")),
                    ..placed
                });
            }
        }
        match &mut cur {
            Some((placed, texts)) => {
                placed.x0 = placed.x0.min(line.x0());
                placed.x1 = placed.x1.max(line.x1());
                texts.push(text.clone());
            }
            None => {
                let placed = Placed {
                    top: line.top,
                    x0: line.x0(),
                    x1: line.x1(),
                    block: Block::Image, // replaced by the text once the block is done
                };
                cur = Some((placed, vec![text.clone()]));
            }
        }
        prev = Some((line, text));
    }
    if let Some((placed, texts)) = cur {
        out.push(Placed {
            block: Block::Text(texts.join("\n")),
            ..placed
        });
    }
    out
}

fn page_blocks(doc: &PdfDocument, page: usize) -> Result<Vec<Block>, FccError> {
    let read_err =
        |e: pdf_oxide::error::Error| FccError::UnsupportedFormat(format!("dosya okunamadı: {e}"));
    let lines = doc.extract_text_lines(page).map_err(read_err)?;
    let pieces = lines.iter().map(Piece::from_line).collect();

    let mut placed: Vec<Placed> = vec![];
    for column in layout(pieces, 0) {
        let mut blocks = column_blocks(&column);
        // a word hyphenated across a column break stays one paragraph
        if let Some(Placed {
            block: Block::Text(prev),
            ..
        }) = placed.last_mut()
            && let Some(Placed {
                block: Block::Text(first),
                ..
            }) = blocks.first()
            && (ends_hyphen(prev) || (!ends_sentence(prev) && starts_lowercase(first)))
            && let Block::Text(next) = blocks.remove(0).block
        {
            prev.push('\n');
            prev.push_str(&next);
        }
        placed.extend(blocks);
    }
    for img in doc.extract_images(page).unwrap_or_default() {
        let Some(r) = img.bbox() else {
            // unknown position: last
            placed.push(Placed {
                top: f32::MIN,
                x0: 0.0,
                x1: 0.0,
                block: Block::Image,
            });
            continue;
        };
        let item = Placed {
            top: r.y + r.height,
            x0: r.x,
            x1: r.x + r.width,
            block: Block::Image,
        };
        // before the first block below it in its column, else after the last
        // one above it there, else by height alone
        let same_column = |p: &Placed| p.x0 < item.x1 && item.x0 < p.x1;
        let at = placed
            .iter()
            .position(|p| same_column(p) && p.top < item.top)
            .or_else(|| {
                placed
                    .iter()
                    .rposition(|p| same_column(p) && p.top >= item.top)
                    .map(|i| i + 1)
            })
            .or_else(|| placed.iter().position(|p| p.top < item.top))
            .unwrap_or(placed.len());
        placed.insert(at, item);
    }
    Ok(placed.into_iter().map(|p| p.block).collect())
}

fn text_blocks(blocks: &[Block]) -> Vec<&str> {
    blocks
        .iter()
        .filter_map(|b| match b {
            Block::Text(t) if !t.trim().is_empty() => Some(t.as_str()),
            _ => None,
        })
        .collect()
}

/// Masked texts that open or close at least half the pages.
fn repeated_keys(pages: &[Vec<Block>]) -> HashSet<String> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for blocks in pages {
        let texts = text_blocks(blocks);
        let (Some(first), Some(last)) = (texts.first(), texts.last()) else {
            continue;
        };
        let keys: HashSet<String> = [mask(first), mask(last)].into();
        for k in keys {
            *counts.entry(k).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .filter(|(_, n)| n * 2 >= pages.len())
        .map(|(k, _)| k)
        .collect()
}

/// A page's paragraphs and how many of them are real text (not image markers).
fn page_paragraphs(
    blocks: &[Block],
    page_no: usize,
    drop: &HashSet<String>,
) -> (Vec<String>, usize) {
    let texts = text_blocks(blocks);
    let (first, last) = (texts.first().copied(), texts.last().copied());
    let mut out = vec![];
    let mut text_count = 0;
    let mut img_no = 0;
    for b in blocks {
        match b {
            Block::Image => {
                img_no += 1;
                out.push(format!("[asset: page{page_no}_img{img_no}]"));
            }
            Block::Text(t) if !t.trim().is_empty() => {
                let edge = Some(t.as_str()) == first || Some(t.as_str()) == last;
                if edge && drop.contains(&mask(t)) {
                    continue;
                }
                let norm = normalize_paragraphs(&[t.replace("\u{ad}\n", "")]);
                text_count += norm.len();
                out.extend(norm);
            }
            Block::Text(_) => {}
        }
    }
    (out, text_count)
}

/// (page index, title) of every outline entry that points at a page, in outline order.
fn outline_starts(items: &[OutlineItem], out: &mut Vec<(usize, String)>) {
    for it in items {
        if let Some(Destination::PageIndex(p)) = it.dest {
            out.push((p, it.title.trim().to_string()));
        }
        outline_starts(&it.children, out);
    }
}

impl Loader for PdfLoader {
    fn name(&self) -> &'static str {
        "pdf"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".pdf"]
    }

    fn can_load(&self, path: &Path, head: &[u8]) -> bool {
        has_suffix(path, self.extensions()) || head.starts_with(b"%PDF")
    }

    fn load(&self, path: &Path) -> Result<Book, FccError> {
        let data = std::fs::read(path).map_err(|e| FccError::from_io(&e))?;
        let id = book_id(&data);
        let doc = PdfDocument::from_bytes(data).map_err(unsupported)?;
        if doc.is_encrypted() && !doc.is_authenticated() {
            return Err(FccError::DrmProtected("DRM korumalı, açılamaz".into()));
        }
        let n_pages = doc.page_count().map_err(unsupported)?;
        let pages = (0..n_pages)
            .map(|i| page_blocks(&doc, i))
            .collect::<Result<Vec<_>, _>>()?;

        let drop = if pages.len() >= MIN_PAGES_FOR_HEADER_STRIP {
            repeated_keys(&pages)
        } else {
            HashSet::new()
        };
        let mut per_page = vec![];
        let mut total_text = 0;
        for (i, blocks) in pages.iter().enumerate() {
            let (paras, n) = page_paragraphs(blocks, i + 1, &drop);
            per_page.push(paras);
            total_text += n;
        }
        if total_text == 0 {
            return Err(FccError::NoTextLayer(
                "Metin katmanı yok, OCR desteklenmiyor".into(),
            ));
        }

        let chapter = |title: Option<String>, start: usize, end: usize| Chapter {
            title,
            paragraphs: per_page[start..end]
                .iter()
                .flatten()
                .cloned()
                .map(Paragraph::new)
                .collect(),
        };
        let mut entries = vec![];
        if let Ok(Some(items)) = doc.get_outline() {
            outline_starts(&items, &mut entries);
        }
        // the first entry for a page names its chapter
        let mut starts: BTreeMap<usize, Option<String>> = BTreeMap::new();
        for (page, title) in entries {
            let page = page.min(n_pages.saturating_sub(1));
            starts
                .entry(page)
                .or_insert((!title.is_empty()).then_some(title));
        }
        let mut chapters = vec![];
        if starts.is_empty() {
            chapters.push(chapter(None, 0, n_pages));
        } else {
            let bounds: Vec<usize> = starts.keys().copied().collect();
            if bounds[0] > 0 {
                chapters.push(chapter(None, 0, bounds[0]));
            }
            for (k, &s) in bounds.iter().enumerate() {
                let e = bounds.get(k + 1).copied().unwrap_or(n_pages);
                chapters.push(chapter(starts[&s].clone(), s, e));
            }
        }
        chapters.retain(|c| !c.paragraphs.is_empty());
        Ok(Book { id, chapters })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loaders::load_book;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/pdf")
            .join(name)
    }

    fn texts(book: &Book) -> Vec<String> {
        book.chapters
            .iter()
            .flat_map(|c| c.paragraphs.iter().map(|p| p.text.clone()))
            .collect()
    }

    // -- test_mupdf.py (pdf) --

    #[test]
    fn pdf_paragraphs_turkish_text() {
        let path = fixture("three_pages.pdf");
        let book = PdfLoader.load(&path).unwrap();
        assert!(texts(&book).contains(&"Türkçe karakterler: ığşçöü İstanbul.".to_string()));
        assert_eq!(book.id, book_id(&std::fs::read(&path).unwrap()));
    }

    #[test]
    fn pdf_footer_removed() {
        let t = texts(&PdfLoader.load(&fixture("three_pages.pdf")).unwrap());
        assert!(!t.iter().any(|x| x.starts_with("Sayfa")), "{t:?}");
        assert!(t.iter().any(|x| x.contains("Ağır ağır")));
    }

    #[test]
    fn pdf_footer_kept_when_not_repeated() {
        // with only 2 pages header/footer stripping must not run
        let t = texts(&PdfLoader.load(&fixture("two_pages.pdf")).unwrap());
        assert!(
            t.contains(&"Sayfa 1".to_string()) && t.contains(&"Sayfa 2".to_string()),
            "{t:?}"
        );
    }

    #[test]
    fn one_page_pdf_single_block_survives() {
        assert_eq!(
            texts(&PdfLoader.load(&fixture("one_page.pdf")).unwrap()),
            vec!["Tek blok metin."]
        );
    }

    #[test]
    fn pdf_hyphen_joined() {
        let t = texts(&PdfLoader.load(&fixture("three_pages.pdf")).unwrap());
        assert!(t.contains(&"bölümlendirme tamam.".to_string()), "{t:?}");
    }

    #[test]
    fn encrypted_pdf_raises_drm() {
        assert!(matches!(
            PdfLoader.load(&fixture("encrypted.pdf")),
            Err(FccError::DrmProtected(_))
        ));
    }

    #[test]
    fn image_only_pdf_raises_no_text_layer() {
        assert!(matches!(
            PdfLoader.load(&fixture("image_only.pdf")),
            Err(FccError::NoTextLayer(_))
        ));
    }

    #[test]
    fn load_book_routes_pdf_here() {
        assert!(
            load_book(&fixture("three_pages.pdf"))
                .unwrap()
                .paragraph_count()
                > 0
        );
    }

    #[test]
    fn corrupt_pdf_is_unsupported_format() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("bad.pdf");
        std::fs::write(&p, (0..=255u8).cycle().take(2048).collect::<Vec<u8>>()).unwrap();
        assert!(matches!(load_book(&p), Err(FccError::UnsupportedFormat(_))));
    }

    #[test]
    fn can_load_magic_and_extension() {
        assert!(PdfLoader.can_load(Path::new("a.pdf"), b""));
        assert!(PdfLoader.can_load(Path::new("noext"), b"%PDF-1.7"));
        assert!(!PdfLoader.can_load(Path::new("a.txt"), b"hello"));
    }

    #[test]
    fn pdf_image_block_becomes_asset() {
        assert_eq!(
            texts(&PdfLoader.load(&fixture("text_image.pdf")).unwrap()),
            vec!["Resimli sayfa metni.", "[asset: page1_img1]"]
        );
    }

    #[test]
    fn pdf_toc_splits_chapters() {
        let book = PdfLoader.load(&fixture("toc.pdf")).unwrap();
        let titles: Vec<Option<&str>> = book.chapters.iter().map(|c| c.title.as_deref()).collect();
        assert_eq!(titles, vec![Some("Bir"), Some("Iki")]);
        let per: Vec<Vec<&str>> = book
            .chapters
            .iter()
            .map(|c| c.paragraphs.iter().map(|p| p.text.as_str()).collect())
            .collect();
        assert_eq!(
            per,
            vec![
                vec!["Sayfa metni alfa.", "Sayfa metni beta."],
                vec!["Sayfa metni gama.", "Sayfa metni delta."]
            ]
        );
    }

    /// A minimal uncompressed PDF: US Letter pages, Helvetica, one `(x, y, size, text)`
    /// per line (PDF space, y up). Text is Latin-1 (WinAnsi) bytes; `(`, `)` and `\` are escaped.
    fn make_pdf(pages: &[Vec<Line>]) -> Vec<u8> {
        let n = pages.len();
        // objects: 1 catalog, 2 pages, 3 font, then (page, content) per page
        let mut objs: Vec<Vec<u8>> =
            vec![
            b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
            format!(
                "<< /Type /Pages /Count {n} /Kids [{}] >>",
                (0..n).map(|i| format!("{} 0 R", 4 + 2 * i)).collect::<Vec<_>>().join(" ")
            )
            .into_bytes(),
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
                .to_vec(),
        ];
        for (i, lines) in pages.iter().enumerate() {
            let mut content = vec![];
            for (x, y, size, text) in lines {
                content.extend(format!("BT /F1 {size} Tf {x} {y} Td (").into_bytes());
                for c in text.chars() {
                    if matches!(c, '(' | ')' | '\\') {
                        content.push(b'\\');
                    }
                    content.push(u32::from(c) as u8);
                }
                content.extend(b") Tj ET\n");
            }
            objs.push(
                format!(
                    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
                     /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
                    5 + 2 * i
                )
                .into_bytes(),
            );
            let mut stream = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
            stream.extend(content);
            stream.extend(b"\nendstream");
            objs.push(stream);
        }
        let mut out = b"%PDF-1.4\n".to_vec();
        let mut offsets = vec![];
        for (i, o) in objs.iter().enumerate() {
            offsets.push(out.len());
            out.extend(format!("{} 0 obj\n", i + 1).into_bytes());
            out.extend(o);
            out.extend(b"\nendobj\n");
        }
        let xref = out.len();
        out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).into_bytes());
        for off in offsets {
            out.extend(format!("{off:010} 00000 n \n").into_bytes());
        }
        out.extend(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                objs.len() + 1
            )
            .into_bytes(),
        );
        out
    }

    /// `(x, y, size, text)` of one line of a generated page.
    type Line = (f32, f32, f32, String);

    fn line(x: f32, y: f32, size: f32, text: &str) -> Line {
        (x, y, size, text.to_string())
    }

    /// 10pt lines starting at `x`, the first at `y`, 14pt apart.
    fn rows(x: f32, y: f32, texts: &[&str]) -> Vec<Line> {
        texts
            .iter()
            .enumerate()
            .map(|(i, t)| line(x, y - 14.0 * i as f32, 10.0, t))
            .collect()
    }

    fn load_generated(pages: &[Vec<Line>]) -> Vec<String> {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("gen.pdf");
        std::fs::write(&p, make_pdf(pages)).unwrap();
        texts(&PdfLoader.load(&p).unwrap())
    }

    const LEFT: [&str; 4] = [
        "The left column opens the story and",
        "keeps going down the page line by line",
        "until it reaches the very end of its",
        "text at the bottom of the column.",
    ];
    const RIGHT: [&str; 4] = [
        "The right column comes second and",
        "must not be read before the left one",
        "even though its lines sit at the very",
        "same height as those on the left.",
    ];
    const LEFT_PARA: &str = "The left column opens the story and keeps going down the page \
                             line by line until it reaches the very end of its text at the \
                             bottom of the column.";
    const RIGHT_PARA: &str = "The right column comes second and must not be read before the \
                              left one even though its lines sit at the very same height as \
                              those on the left.";

    /// A title at `title_y` over two columns of four lines, the right one at `right_x`.
    fn two_column_page(title_y: f32, right_x: f32) -> Vec<Line> {
        let mut page = vec![line(
            72.0,
            title_y,
            18.0,
            "Columns Under One Wide Title Here",
        )];
        page.extend(rows(72.0, 700.0, &LEFT));
        page.extend(rows(right_x, 700.0, &RIGHT));
        page
    }

    const TITLE: &str = "Columns Under One Wide Title Here";

    #[test]
    fn two_column_page_reads_left_column_first() {
        assert_eq!(
            load_generated(&[two_column_page(740.0, 320.0)]),
            vec![TITLE, LEFT_PARA, RIGHT_PARA]
        );
    }

    #[test]
    fn title_spanning_both_columns_comes_first() {
        // the title sits right above the columns (no blank band), and the
        // gutter is narrow enough for the extractor to run the rows together
        assert_eq!(
            load_generated(&[two_column_page(716.0, 268.0)]),
            vec![TITLE, LEFT_PARA, RIGHT_PARA]
        );
    }

    #[test]
    fn same_height_lines_in_columns_are_not_merged() {
        let mut page = rows(72.0, 700.0, &LEFT);
        page.extend(rows(320.0, 700.0, &RIGHT));
        assert_eq!(load_generated(&[page]), vec![LEFT_PARA, RIGHT_PARA]);
    }

    #[test]
    fn full_width_line_between_column_stretches_stays_in_place() {
        let mut page = rows(72.0, 700.0, &LEFT[..3]);
        page.extend(rows(320.0, 700.0, &RIGHT[..3]));
        page.push(line(
            72.0,
            656.0,
            10.0,
            "A caption runs across the whole width of the page, over both columns.",
        ));
        page.extend(rows(
            72.0,
            638.0,
            &["Lower left one", "lower left two", "lower left end."],
        ));
        page.extend(rows(
            320.0,
            638.0,
            &["Lower right one", "lower right two", "lower right end."],
        ));
        assert_eq!(
            load_generated(&[page]),
            vec![
                "The left column opens the story and keeps going down the page line by line \
                 until it reaches the very end of its",
                "The right column comes second and must not be read before the left one even \
                 though its lines sit at the very",
                "A caption runs across the whole width of the page, over both columns.",
                "Lower left one lower left two lower left end.",
                "Lower right one lower right two lower right end.",
            ]
        );
    }

    #[test]
    fn narrow_side_column_is_not_a_column() {
        // a contents page: titles left, page numbers far right, read row by row
        let mut page = vec![];
        let toc = [
            ("Chapter One", "5"),
            ("Chapter Two", "9"),
            ("Chapter Three", "14"),
            ("Chapter Four", "20"),
        ];
        for (i, (t, n)) in toc.iter().enumerate() {
            let y = 700.0 - 14.0 * i as f32;
            page.push(line(72.0, y, 10.0, t));
            page.push(line(520.0, y, 10.0, n));
        }
        assert_eq!(
            load_generated(&[page]),
            vec!["Chapter One 5 Chapter Two 9 Chapter Three 14 Chapter Four 20"]
        );
    }

    #[test]
    fn two_column_pages_lose_header_and_footer() {
        let pages: Vec<Vec<Line>> = (1..=3)
            .map(|n| {
                let mut page = two_column_page(740.0, 320.0);
                page.push(line(72.0, 770.0, 9.0, "Running Head Of The Book"));
                page.push(line(300.0, 40.0, 9.0, &format!("{n}")));
                page
            })
            .collect();
        let t = load_generated(&pages);
        assert_eq!(t.len(), 9, "{t:?}");
        assert_eq!(t[..3], [TITLE, LEFT_PARA, RIGHT_PARA]);
    }

    #[test]
    fn indented_paragraphs_without_spacing_split() {
        let mut page = rows(92.0, 700.0, &["First paragraph starts with an indent and"]);
        page.extend(rows(
            72.0,
            686.0,
            &[
                "runs on over a second line without any",
                "extra space below it",
            ],
        ));
        page.extend(rows(
            92.0,
            644.0,
            &["Second paragraph is indented the same way"],
        ));
        page.extend(rows(72.0, 630.0, &["and also ends without a gap"]));
        page.extend(rows(92.0, 616.0, &["Third one closes the page."]));
        assert_eq!(
            load_generated(&[page]),
            vec![
                "First paragraph starts with an indent and runs on over a second line without \
                 any extra space below it",
                "Second paragraph is indented the same way and also ends without a gap",
                "Third one closes the page.",
            ]
        );
    }

    #[test]
    fn short_line_on_sentence_end_closes_paragraph() {
        let page = rows(
            72.0,
            700.0,
            &[
                "A block paragraph with no indent at all",
                "ends on a short line.",
                "The next one starts flush left and runs",
                "across the whole width of the column",
                "and its lines are evenly spaced too",
                "until it stops.",
            ],
        );
        assert_eq!(
            load_generated(&[page]),
            vec![
                "A block paragraph with no indent at all ends on a short line.",
                "The next one starts flush left and runs across the whole width of the column \
                 and its lines are evenly spaced too until it stops.",
            ]
        );
    }

    #[test]
    fn hyphen_joining_survives_paragraph_splits() {
        let mut page = rows(92.0, 700.0, &["Indented paragraphs keep their hyph-"]);
        page.extend(rows(72.0, 686.0, &["enated words whole across line ends."]));
        page.extend(rows(
            92.0,
            672.0,
            &["So does a soft hyphen at the end of a com\u{ad}"],
        ));
        page.extend(rows(
            72.0,
            658.0,
            &["plete line, even when the next line is in\u{ad}"],
        ));
        page.extend(rows(92.0, 644.0, &["dented by mistake."]));
        assert_eq!(
            load_generated(&[page]),
            vec![
                "Indented paragraphs keep their hyphenated words whole across line ends.",
                "So does a soft hyphen at the end of a complete line, even when the next line \
                 is indented by mistake.",
            ]
        );
    }

    #[test]
    fn hyphenated_word_across_column_break_is_joined() {
        // the left column's last word goes on at the top of the right column
        let mut page = rows(
            72.0,
            700.0,
            &[
                "The left column opens the story and",
                "goes on until its very last word is",
                "split by a hyph-",
            ],
        );
        page.extend(rows(
            320.0,
            700.0,
            &["enated break that the", "right column finishes", "for it."],
        ));
        assert_eq!(
            load_generated(&[page]),
            vec![
                "The left column opens the story and goes on until its very last word is split \
                 by a hyphenated break that the right column finishes for it."
            ]
        );
    }

    #[test]
    fn sentence_continues_across_columns_without_hyphen() {
        let mut page = rows(
            72.0,
            700.0,
            &[
                "The left column opens the story and",
                "goes on until its very last line, which",
                "stops in the middle of a",
            ],
        );
        page.extend(rows(
            320.0,
            700.0,
            &[
                "sentence that the right column",
                "finishes here.",
                "Then a new one.",
            ],
        ));
        let t = load_generated(&[page]);
        assert_eq!(t.len(), 2, "{t:?}");
        assert!(
            t[0].contains("middle of a sentence that the right"),
            "{t:?}"
        );
    }

    #[test]
    fn column_starting_uppercase_after_open_line_is_not_joined() {
        let mut page = rows(
            72.0,
            700.0,
            &[
                "The left column opens the story and",
                "goes on until it stops",
                "without punctuation",
            ],
        );
        page.extend(rows(
            320.0,
            700.0,
            &[
                "Another story begins in the right",
                "column and runs on here",
                "until its end.",
            ],
        ));
        assert_eq!(load_generated(&[page]).len(), 2);
    }
}
