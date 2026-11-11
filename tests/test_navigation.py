"""Chapter jumps and text search on the book model (UI independent)."""
from fcc.book import Book, Chapter, Paragraph, fold


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


def test_fold_keeps_length_and_ignores_case_and_turkish_dotted_i():
    for s in ("İstanbul", "ISTANBUL", "ıstanbul", "Straße"):
        assert len(fold(s)) == len(s)
    assert fold("İstanbul") == fold("istanbul") == fold("ISTANBUL") == fold("ıstanbul")


def test_find_forward_starts_at_the_given_paragraph_and_wraps():
    assert B.find("alfa", (0, 0)) == ((0, 0), False)
    assert B.find("alfa", (0, 1)) == ((3, 0), False)
    assert B.find("beta", (2, 0)) == ((0, 0), True)  # wrapped past the end


def test_find_backward():
    assert B.find("alfa", (2, 1), backward=True) == ((0, 0), False)
    assert B.find("zeta", (0, 0), backward=True) == ((3, 0), True)


def test_find_is_case_and_dotted_i_insensitive():
    assert B.find("istanbul", (0, 0)) == ((2, 0), False)


def test_find_missing_or_empty_returns_none():
    assert B.find("omega", (0, 0)) is None
    assert B.find("  ", (0, 0)) is None


def test_find_ranges_in_a_string():
    from fcc.book import match_spans

    assert match_spans("Alfa beta ALFA", "alfa") == [(0, 4), (10, 14)]
    assert match_spans("abc", "") == []
