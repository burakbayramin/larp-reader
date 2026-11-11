# fcc

Terminal e-kitap okuyucusu; metni bir geliştirici aracının çıktısı gibi gösterir
(`build`, `log`, `diff`, `test`, `code` görünümleri). Yoldan geçen biri bir derleme
aracına baktığınızı sanır.

## Kurulum

```
pipx install .
# veya
pip install .
```

Python 3.11+ gerekir. Kurulumu doğrulamak için: `fcc --version`.

## Kullanım

```
fcc build <dosya> --mask=build --lang=rust --noise=2
```

- `--mask`: `code`, `log`, `diff`, `test`, `build`
- `--lang`: `python`, `typescript`, `go`, `rust`, `java`, `csharp` (code görünümü için)
- `--noise`: araya karışan sahte çıktı miktarı (0-5)
- `--auto`, `--speed`: otomatik akış ve hızı
- Dosya verilmezse en son açılan kitaptan devam edilir.

Windows Terminal'de ekranı bölmek için `Alt+Shift+D`: bir tarafta kitap, diğer tarafta
gerçek bir geliştirme oturumu.

## Tuşlar

| Tuş | İşlev |
| --- | --- |
| `Space` | bir sayfa ileri |
| `Down` / `j` | bir satır ileri |
| `Up` / `k` | bir satır geri |
| `a` | otomatik akışı aç/kapat |
| `+` / `-` | otomatik akış hızı |
| `m` | görünümü değiştir (mask) |
| `n` / `N` | gürültüyü artır / azalt |
| `b` | yer imi ekle (`cache saved` mesajı) |
| `:` | ayar menüsü (dosya aç, görünüm, dil, hız, gürültü, yer imleri) |
| `Esc` / `` ` `` | panik: anında sahte çıktı akışı (menü açıksa menüyü de kapatır) |
| `q` | ilerlemeyi kaydet ve çık |

Menüde `Tab`/`Up`/`Down` ile alan değiştirilir, `Left`/`Right` ile değer seçilir,
`Enter` uygular.

## Desteklenen biçimler

EPUB, PDF (XPS, CBZ), MOBI/AZW/AZW3, FB2, DOCX, ODT, RTF, TXT/Markdown, HTML.
Desteklenmeyen veya DRM'li bir biçim için Calibre kuruluysa `ebook-convert` ile
EPUB'a çevirip onu açabilirsiniz (Calibre yedek yol olarak kullanılır).

## Kapsam dışı

- Taranmış belgeler için OCR
- DRM çözme
