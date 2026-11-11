"""Chapter jumps and text search on the book model (UI independent)."""
from fcc.book import Book, Chapter, Paragraph


def book(*chapters: tuple[str | None, list[str]]) -> Book:
    return Book("id", tuple(Chapter(t, tuple(Paragraph(p) for p in ps)) for t, ps in chapters))


B = book(
    ("Bir", ["alfa beta", "gama"]),
    ("Boş", []),
    ("İki", ["delta İstanbul", "epsilon"]),
    (None, ["zeta alfa"]),
)


def test_chapter_list_skips_empty_chapters():
    assert B.chapter_list() == [(0, "Bir"), (2, "İki"), (3, None)]


def test_next_chapter_goes_to_the_first_paragraph_of_the_next_non_empty_one():
    assert B.next_chapter((0, 1)) == (2, 0)
    assert B.next_chapter((2, 0)) == (3, 0)
    assert B.next_chapter((3, 0)) is None


def test_prev_chapter_goes_to_this_chapters_start_first_then_the_previous_one():
    assert B.prev_chapter((2, 1)) == (2, 0)
    assert B.prev_chapter((2, 0)) == (0, 0)
    assert B.prev_chapter((0, 0)) is None

