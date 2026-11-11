from fcc.book import Book, Chapter, Paragraph, book_id


def test_navigation_crosses_chapters():
    b = Book("x", (Chapter("1", (Paragraph("a"), Paragraph("b"))), Chapter(None, (Paragraph("c"),))))
    assert b.next_pos((0, 1)) == (1, 0)
    assert b.prev_pos((1, 0)) == (0, 1)
    assert b.next_pos((1, 0)) is None and b.prev_pos((0, 0)) is None
    assert b.paragraph_count() == 3 and b.index_of((1, 0)) == 2
    assert b.paragraph((1, 0)).text == "c"


def test_book_id_is_16_hex_of_sha256():
    assert book_id(b"abc") == "ba7816bf8f01cfea"
