# fcc

Terminal e-kitap okuyucusu; metni bir geliştirici aracının çıktısı gibi gösterir
(`code`, `log`, `diff`, `test`, `build`, `git`, `docker`, `agent` görünümleri). Yoldan geçen biri bir derleme
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
fcc build <dosya> --mask=code --lang=python --noise=2
```

- `--mask`: `code`, `log`, `diff`, `test`, `build`, `git` (`git log --stat`), `docker` (`docker compose up` logları), `agent` (kod yazan bir ajan oturumu: araç çağrıları, yeşil/kırmızı diff'ler, komut çıktıları)
- `--lang`: `python`, `typescript`, `go`, `rust`, `java`, `csharp` (code görünümü için)
- `--noise`: araya karışan sahte çıktı miktarı (0-5)
- `--auto`: otomatik akış
- `--wpm`: okuma hızı, dakikada kelime (60-1500, varsayılan 220); otomatik akış buna göre ilerler
- `--hide`: panik tuşu ne göstersin: `stream` (akan sahte çıktı) veya `shell` (komut yazılabilen sahte shell)
- Dosya verilmezse en son açılan kitaptan devam edilir.

Windows Terminal'de ekranı bölmek için `Alt+Shift+D`: bir tarafta kitap, diğer tarafta
gerçek bir geliştirme oturumu.

## Tuşlar

| Tuş | İşlev |
| --- | --- |
| `Space` / `PgDn` | bir sayfa ileri |
| `u` / `PgUp` | bir sayfa geri |
| `Down` / `j` | bir satır ileri |
| `Up` / `k` | bir satır geri |
| `]` / `[` | sonraki bölüm / bölüm başı, oradaysa önceki bölüm |
| `/` | metinde ara (büyük-küçük harf ve İ/ı fark etmez); eşleşmeler vurgulanır |
| `f` / `F` | sonraki / önceki eşleşme (sona gelince başa sarar) |
| `a` | otomatik akışı aç/kapat |
| `+` / `-` | okuma hızını ~%10 artır / azalt (`throttle 240/min` mesajı) |
| `m` / `M` | görünümü değiştir: sonraki / önceki (code, log, diff, test, build, git, docker, agent) |
| `l` / `L` | kod dilini değiştir: sonraki / önceki (code görünümüne geçer) |
| `n` / `N` | gürültüyü artır / azalt |
| `b` | yer imi ekle (`cache saved` mesajı) |
| `'` / `"` | sonraki / önceki yer imine git |
| `:` | ayar menüsü (dosya aç, görünüm, dil, okuma hızı, gürültü, panik biçimi, yer imleri, bölümler) |
| `o` / `O` | klasördeki sonraki / önceki kitaba geç (her kitabın yeri ayrı saklanır) |
| `Esc` | yan panel: kısayollar ve klasördeki kitaplar; panel açıkken `1`-`9` ile listeden kitap aç |
| `` ` `` | panik: anında sahte çıktı akışı ya da sahte shell (panel, menü veya arama açıksa onu da kapatır) |
| `q` | ilerlemeyi kaydet ve çık |

Menüde `Tab`/`Up`/`Down` ile alan değiştirilir, `Left`/`Right` ile değer seçilir,
`Enter` uygular.

Otomatik akış okuma hızına göre ilerler: her satır, içindeki kelime sayısına göre
ekranda kalır (220 WPM'de 11 kelimelik bir satır 3 saniye). Hız elle sayfa
çevirdikçe öğrenilir: `Space` ile her ekran geçişinde o ekrandaki kelimeleri ne kadar
sürede okuduğun ölçülür ve kayıtlı hız yavaşça buna yaklaşır. Çok hızlı geçilen
(2 sn altı) ya da uzun süre açık kalan (5 dk üstü) ekranlar ve panik modunda
geçen süre hesaba katılmaz.

`agent` görünümü bir kod ajanı oturumu gibi akar: `⏺ Update(...)` diff'leri, `⏺ Bash(...)`
komut çıktıları, `Read`/`Search` çağrıları ve yapılacaklar listesi. Paragrafların çoğu
diff'te eklenen (yeşil) satırlar, kalanı ajanın açıklama metni olarak görünür. Dolgu kodu
binlerce farklı satırlık havuzlardan sırayla okunur, bu yüzden uzun süre aynı satır
tekrar etmez. Sürekli akış için: `fcc build kitap.epub --mask=agent --auto`.

Sahte shell (`--hide=shell` ya da menüde `on hide: shell`): panik tuşuna basınca
gerçek bir prompt çıkar (Windows'ta `PS C:\...>`, macOS/Linux'ta kullanıcı@makine).
`ls`, `pwd`, `git status`, `clear` beklenen çıktıyı verir; `npm`, `pytest`, `cargo`,
`docker` gibi araçlar o anki görünümün çıktısını basar; bilinmeyen komutlar
`command not found` verir. Kitaba dönmek için yine `` ` ``.

## Desteklenen biçimler

EPUB, PDF (XPS, CBZ), MOBI/AZW/AZW3, FB2, DOCX, ODT, RTF, TXT/Markdown, HTML.
Desteklenmeyen veya DRM'li bir biçim için Calibre kuruluysa `ebook-convert` ile
EPUB'a çevirip onu açabilirsiniz (Calibre yedek yol olarak kullanılır).

## Kapsam dışı

- Taranmış belgeler için OCR
- DRM çözme
