# fcc — Fiction Compiler Collection: Tasarım

- **Tarih:** 2026-11-10
- **Durum:** İncelemede

## 1. Amaç

Terminalde Claude CLI (veya başka bir geliştirme işi) çalışırken, yandaki bölmede kitap okumayı
sağlayan bir terminal uygulaması. Ekrana birkaç saniye bakan bir geliştirici bile çıktıyı sıradan
bir derleme / log / test çıktısı sanmalı; kullanıcı ise metni rahatça okuyabilmeli.

### Başarı ölçütleri

1. Ekranda kitap adı, "reader/okuyucu" kelimesi veya kitap arayüzünü andıran hiçbir öğe yok.
2. Kitap metni seçili kamuflaj içinde eksiksiz ve sıralı okunabiliyor.
3. Panik tuşu kitap metnini ekrandan ve kaydırma geçmişinden anında siliyor.
4. Desteklenen tüm formatlar aynı deneyimle açılıyor.

### Kapsam dışı (v1)

OCR (taranmış PDF), DRM kaldırma, Claude Code eklentisi, resim gösterimi.

## 2. Ortam ve teknoloji

- Windows 11 + Windows Terminal (bölmeli kullanım: `Alt+Shift+D`), Linux/macOS'ta da çalışmalı.
- Python ≥ 3.11 (geliştirme ortamında 3.14), arayüz: **Textual**.
- Dağıtım: `pipx install .` → `fcc` komutu.
- Bağımlılıklar: `textual`, `pymupdf`, `ebooklib`, `beautifulsoup4`, `python-docx`, `striprtf`,
  `odfpy`, `mobi`. Calibre (`ebook-convert`) isteğe bağlı yedek.
- Not: PyMuPDF AGPL lisanslıdır; kişisel kullanım için uygundur.

## 3. Mimari

Tek yönlü akış: **dosya → loader → `Book` → mask (+ noise) → ekran**.

```
fcc/
├── cli.py            # komut satırı: "fcc build <dosya> --mask=… --lang=… --noise=…"
├── app.py            # Textual uygulaması, ekranlar, tuş bağlamaları
├── book.py           # Book → Chapter → Paragraph veri modeli
├── loaders/
│   ├── base.py       # Loader arayüzü + kayıt/seçim mantığı
│   ├── mupdf.py      # pdf, epub, mobi, fb2, xps, cbz
│   ├── epub.py       # ebooklib ile daha temiz bölüm ayrımı (epub için öncelikli)
│   ├── azw3.py       # mobi kütüphanesiyle aç → html'e indir
│   ├── docx.py, odt.py, rtf.py, html.py
│   ├── text.py       # txt, md (kodlama tespiti: utf-8 → cp1254 → latin-1)
│   └── calibre.py    # ebook-convert kuruluysa epub'a dönüştür, epub loader'a ver
├── masks/
│   ├── base.py       # Mask arayüzü
│   ├── code.py       # kod yorumu; dil: python, typescript, go, rust, java, csharp
│   ├── log.py        # uygulama logları (zaman damgası + seviye + modül)
│   ├── diff.py       # git diff (hunk başlıkları, +/− satırlar)
│   ├── test.py       # pytest/jest çıktısı
│   └── build.py      # derleme çıktısı (cargo/tsc/gradle tarzı)
├── noise.py          # her mask için uygun sahte gürültü satırları
├── panic.py          # kitap izi olmayan sonsuz sahte akış
└── state.py          # ayarlar, kaldığın yer, yer imleri
```

### 3.1 Veri modeli (`book.py`)

```python
@dataclass(frozen=True)
class Paragraph:
    text: str                 # düz metin, tek paragraf

@dataclass(frozen=True)
class Chapter:
    title: str | None
    paragraphs: list[Paragraph]

@dataclass(frozen=True)
class Book:
    id: str                   # dosya içeriğinin SHA-256'sının ilk 16 hanesi
    chapters: list[Chapter]
```

Okuma konumu `(chapter_index, paragraph_index)` ikilisidir.
Resimler `[asset: <dosya-adı>]` biçiminde tek satırlık paragraf olarak temsil edilir.

### 3.2 Loader'lar

```python
class Loader(Protocol):
    extensions: tuple[str, ...]
    def can_load(self, path: Path, head: bytes) -> bool: ...
    def load(self, path: Path) -> Book: ...
```

- Seçim: uzantı + ilk 4 KB sihirli baytlara göre öncelik sıralı aday listesi.
- Bir loader hata verirse sıradaki denenir; hepsi başarısızsa Calibre yedeği; o da yoksa
  `UnsupportedFormat` hatası.
- DRM tespit edilirse (`DrmProtected`) yedeklere geçilmeden doğrudan bildirilir.
- PDF'de hiç metin yoksa `NoTextLayer` hatası.
- Metin normalizasyonu ortak: tire ile bölünmüş satır sonlarını birleştir, sayfa başlık/altlıklarını
  (PDF'de tekrar eden satırlar) at, boş paragrafları sil.

### 3.3 Mask'ler

```python
class Mask(Protocol):
    name: str                                  # "code", "log", "diff", "test", "build"
    def header(self, rng: Random) -> list[Line]: ...
    def render(self, para: Paragraph, width: int, rng: Random) -> list[Line]: ...

@dataclass(frozen=True)
class Line:
    segments: list[tuple[str, str]]           # (metin, stil)
    is_book: bool                              # kitap satırı mı, gürültü mü
```

- Her mask kitap paragrafını kendi biçimine sarar (yorum bloğu, log mesajı, diff'teki `+` satırı,
  test assertion mesajı, derleyici uyarı notu) ve verilen genişliğe göre kelime sınırından böler.
- `code` mask'i `--lang` ile dil seçer; yorum sözdizimi ve sahte kod satırları dile göre değişir.

### 3.4 Gürültü (`noise.py`)

- Seviye 0–5. Her kitap paragrafından sonra eklenecek gürültü satırı sayısı
  `rng` ile `[0, 2·seviye]` aralığından seçilir; seviye 0 ⇒ hiç gürültü yok.
- Gürültü, aktif mask'in biçimine uygun üretilir (log mask'inde sahte log satırları,
  build mask'inde sahte "Compiling …" satırları vb.).
- Gürültü satırları `is_book=False` işaretlidir ve okuma ilerlemesine/hızına sayılmaz.

### 3.5 Panik (`panic.py`)

- Aktif mask'in yalnızca gürültü üreticisini kullanan sonsuz akış.
- Panik açılınca ekran tamponu ve kaydırma geçmişi temizlenir; geri dönüldüğünde
  kaldığın paragraftan yeniden çizilir.

### 3.6 Durum (`state.py`)

- Konum: `%APPDATA%\fcc\` (Windows), `~/.config/fcc/` (diğer).
- `config.json`: varsayılan mask, dil, gürültü, hız, mod.
- `progress.json`: `{book_id: {path, chapter, paragraph, bookmarks: [...], updated_at}}`.
- 10 saniyede bir ve çıkışta yazılır; yazım geçici dosya + yeniden adlandırma ile atomiktir.
- Bozuk dosya: `.bak` olarak yedeklenir, varsayılanlarla devam edilir.

## 4. Kullanım

### 4.1 Komut satırı

```
fcc build <dosya> [--mask=code|log|diff|test|build] [--lang=…] [--noise=0..5]
                  [--auto] [--speed=<satır/sn>]
fcc               # son kitaba son ayarlarla devam
fcc --version     # "fcc 0.4.2 (x86_64-pc-windows-msvc)" gibi masum bir çıktı
```

### 4.2 Ekran

- Tam ekran Textual uygulaması; içerik alanı + en altta tek satırlık ilerleme çubuğu
  (`Building [=====>      ] 142/380` — okuma yüzdesi). Başlık çubuğu yok.
- Açılışta mask'in `header()` satırları basılır (ör. `Compiling fcc-core v0.4.2`).
- **Otomatik mod:** satırlar alttan eklenir, yukarı kayar (gerçek komut çıktısı gibi);
  hız kitap satırı/saniye cinsindendir.
- **Elle mod:** sayfa sayfa / satır satır ilerleme.

### 4.3 Tuşlar

| Tuş | İşlev |
|---|---|
| `Space` / `↓` / `j` | ilerle |
| `↑` / `k` | geri |
| `a` | otomatik ↔ elle |
| `+` / `-` | otomatik hız |
| `m` | sonraki mask |
| `n` / `N` | gürültü artır / azalt |
| `b` | yer imi |
| `Esc` / `` ` `` | panik aç/kapa |
| `:` | ayar menüsü (kitap, mask, dil, hız, gürültü, yer imleri) |
| `q` | kaydet ve çık |

## 5. Hata davranışı

Tüm hatalar aktif mask'in üslubunda gösterilir (ör. build mask'inde
`error[E0432]: unresolved import` başlığı altında), ama gerçek nedeni açıkça yazar:

| Durum | Davranış |
|---|---|
| Desteklenmeyen/bozuk dosya | sıradaki loader → Calibre → anlaşılır hata |
| DRM | "DRM korumalı, açılamaz"; kırma denemesi yok |
| Metinsiz PDF | "Metin katmanı yok, OCR desteklenmiyor" |
| Bozuk durum dosyası | `.bak` yedeği, varsayılanlarla devam |
| Terminal < 40 sütun | yeniden sarma + kısa uyarı satırı |

## 6. Test

- `pytest` + Textual `Pilot`; TDD ile geliştirilir.
- **Loader'lar:** `tests/fixtures/` altında her format için küçük örnek; bölüm/paragraf sayısı ve
  Türkçe karakterli metnin bozulmadan çıktığı doğrulanır. EPUB ve DOCX örnekleri testte üretilir;
  diğerleri küçük, açık lisanslı dosyalardır.
- **Mask'ler:** kitap metninin tüm kelimeleri çıktıda sırayla bulunur; gürültü 0 iken
  `is_book=False` satır yalnızca `header()` kaynaklıdır.
- **Panik:** panikte ekranda kitap metninden tek kelime bulunmaz (`Pilot` ile ekran içeriği taranır).
- **Durum:** kaydet → kapat → aç ⇒ aynı paragraf; bozuk JSON ⇒ çökme yok.

## 7. Geliştirme sırası (öneri)

1. `book.py` + `text.py` loader + `code` mask + temel Textual ekranı (elle mod) — uçtan uca ilk sürüm.
2. Otomatik mod, panik, durum kaydı.
3. Kalan mask'ler + gürültü.
4. Kalan loader'lar + Calibre yedeği.
5. Ayar menüsü, yer imleri, `pipx` paketleme.
