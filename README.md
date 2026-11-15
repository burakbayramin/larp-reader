# larp-reader

Terminal e-kitap okuyucusu; metni bir geliştirici aracının çıktısı gibi gösterir
(`code`, `log`, `diff`, `test`, `build`, `git`, `docker`, `agent` görünümleri). Yoldan geçen biri bir derleme
aracına baktığınızı sanır.

## Kurulum

Windows, macOS (Apple Silicon ve Intel) ve Linux için hazır programlar
[Releases](../../releases) sayfasında. Arşivi açıp `larpr` (Windows'ta `larpr.exe`)
dosyasını PATH'teki bir klasöre koymanız yeterli; başka bir şey kurmak gerekmez.

Kaynaktan derlemek için (Rust 1.88+):

```
cargo install --git https://github.com/burakbayramin/larp-reader
# ya da repo klasöründe
cargo install --path .
```

Kurulumu doğrulamak için: `larpr --version`.

Komutun adı `larpr`. Ayarlar ve ilerleme `~/.config/larpr` (Windows'ta
`%APPDATA%\larpr`) klasöründe tutulur. Daha önce `fcc` adıyla kullandıysanız eski
klasör ilk açılışta buraya taşınır; her kitap kaldığı yerden açılır.

## Kullanım

```
larpr build <dosya> --mask=code --lang=python --noise=2
```

- `--mask`: `code`, `log`, `diff`, `test`, `build`, `git` (`git log --stat`), `docker` (`docker compose up` logları), `agent` (kod yazan bir ajan oturumu: araç çağrıları, yeşil/kırmızı diff'ler, komut çıktıları)
- `--lang`: `python`, `typescript`, `go`, `rust`, `java`, `csharp` (code görünümü için)
- `--noise`: araya karışan sahte çıktı miktarı (0-5)
- `--auto`: otomatik akış
- `--wpm`: okuma hızı, dakikada kelime (60-1500, varsayılan 220); otomatik akış buna göre ilerler
- `--hide`: panik tuşu ne göstersin: `stream` (akan sahte çıktı) veya `shell` (komut yazılabilen sahte shell)
- `--theme`: renk teması: `dark` (varsayılan), `light`, `dark-daltonized`, `light-daltonized` (renk körlüğüne uygun: yeşil/kırmızı yerine mavi/turuncu), `dark-ansi`, `light-ansi` (terminalin kendi 16 rengi)
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
| `t` | renk temasını değiştir (dark, light, daltonized, ansi) |
| `:` | ayar menüsü (dosya aç, görünüm, dil, okuma hızı, gürültü, panik biçimi, tema, yer imleri, bölümler) |
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
tekrar etmez. Sürekli akış için: `larpr build kitap.epub --mask=agent --auto`.

Sahte shell (`--hide=shell` ya da menüde `on hide: shell`): panik tuşuna basınca
gerçek bir prompt çıkar (Windows'ta `PS C:\...>`, macOS/Linux'ta kullanıcı@makine).
`ls`, `pwd`, `git status`, `clear` beklenen çıktıyı verir; `npm`, `pytest`, `cargo`,
`docker` gibi araçlar o anki görünümün çıktısını basar; bilinmeyen komutlar
`command not found` verir. Kitaba dönmek için yine `` ` ``.

## Renkler

Terminal 24 bit renk destekliyorsa (`COLORTERM=truecolor`; iTerm2, Windows Terminal,
VS Code, WezTerm, Ghostty …) renkler olduğu gibi gösterilir. macOS 26 öncesindeki
Terminal uygulaması gibi desteklemeyenlerde en yakın 256 renge, gerekirse 16 renge
çevrilir. Terminalinizin kendi renk şemasıyla uyumlu olsun isterseniz `dark-ansi` /
`light-ansi` temalarını deneyin.

## Desteklenen biçimler

EPUB, PDF, MOBI/AZW/AZW3, FB2, DOCX, ODT, RTF, TXT/Markdown, HTML.
Bunların dışındaki biçimler (XPS, CBZ, DJVU …) Calibre kuruluysa `ebook-convert` ile
EPUB'a çevrilip açılır (Calibre yedek yol olarak kullanılır). DRM'li dosyalar açılmaz.

## Kapsam dışı

- Taranmış belgeler için OCR
- DRM çözme
