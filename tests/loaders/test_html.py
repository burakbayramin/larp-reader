from __future__ import annotations

from fcc.loaders.html import HtmlLoader, html_to_paragraphs


def test_html_paragraphs():
    html = (
        "<html><head><title>T</title><style>p{}</style></head><body>"
        "<nav><p>menü</p></nav><script>var x = 1;</script>"
        "<h1>Başlık &amp; Eş</h1><p>Birinci   paragraf &lt;ok&gt;.</p>"
        "<ul><li>madde</li></ul><blockquote>alıntı</blockquote>"
        "<p><img src='a/b/res.png'></p></body></html>"
    )
    title, paras = html_to_paragraphs(html)
    assert title == "Başlık & Eş"
    assert paras == ["Birinci paragraf <ok>.", "madde", "alıntı", "[asset: res.png]"]


def test_html_nested_blocks_not_duplicated():
    _, paras = html_to_paragraphs("<div><p>x</p></div><div><div>y</div></div>")
    assert paras == ["x", "y"]


def test_html_h2_title_and_heading_not_paragraph():
    title, paras = html_to_paragraphs("<h2>Bölüm</h2><p>metin</p>")
    assert title == "Bölüm"
    assert paras == ["metin"]


def test_html_loose_text_in_div_kept():
    _, paras = html_to_paragraphs("<div>serbest <b>kalın</b> metin</div>")
    assert paras == ["serbest kalın metin"]


def test_html_loader_file(tmp_path):
    p = tmp_path / "a.html"
    p.write_text("<h1>Ba</h1><p>İçerik şöyle.</p>", encoding="utf-8")
    loader = HtmlLoader()
    assert loader.can_load(p, b"")
    book = loader.load(p)
    assert book.chapters[0].title == "Ba"
    assert book.chapters[0].paragraphs[0].text == "İçerik şöyle."


def test_html_loader_cp1254_fallback(tmp_path):
    p = tmp_path / "a.htm"
    p.write_bytes("<p>ığşçöü</p>".encode("cp1254"))
    assert HtmlLoader().load(p).chapters[0].paragraphs[0].text == "ığşçöü"
