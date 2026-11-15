# larp-reader

**Terminalde kitap oku, çalışıyormuş gibi görün.**

larp-reader e-kitabınızı terminalde açar ve her satırı bir geliştirici aracının
çıktısı gibi gösterir: kod, derleme logu, diff, test raporu ya da bir kod ajanının
oturumu. Yanınızdan geçen biri ekrana bakınca bir derlemeyi izlediğinizi sanır.
Biri iyice yaklaşırsa tek tuşla kitap tamamen kaybolur.

İsim argodaki *larp*'tan geliyor: rol yapmak, öyleymiş gibi davranmak.
Komutun adı kısaca `larpr`.

```
╭──────────────────────────────────────────────────────╮
│  ▄█████▄ │  larpr v0.7.0                              │
│  █ ▀ ▀ █ │  role: senior engineer (self-appointed)    │
│  ▐█████▌▄┼  cwd: ~/work/api                           │
│   ▟▛ ▜▙  █  armor: cardboard, sword: foam             │
╰──────────────────────────────────────────────────────╯

⏺ Update(src/cache/store.py)
  ⎿  Updated src/cache/store.py with 4 additions and 1 removal
    212       def evict(self, key: str) -> None:
    213 -         self._hits.pop(key, None)
    213 +         O sabah erkenden uyandı, pencereyi açtı
    214 +         ve uzun uzun denizi seyretti.
```

Yeşil satırlar kitabınız; geri kalan her şey sahte.

## Hızlı başlangıç

1. [Releases](../../releases) sayfasından işletim sisteminize uygun arşivi indirin
   (Windows, macOS Apple Silicon / Intel, Linux).
2. Arşivi açın, içindeki `larpr` (Windows'ta `larpr.exe`) dosyasını PATH'teki bir
   klasöre koyun. Başka bir şey kurmanız gerekmez.
3. Kitabınızı açın:

```
larpr build kitap.epub
```

Bir dahaki sefere `larpr build` yazmanız yeterli: en son okuduğunuz kitap kaldığınız
yerden açılır.

Okumak için `Space`, çıkmak için `q`. Biri gelirse: `` ` `` (backtick).

## Panik tuşu

`` ` `` tuşuna bastığınız an kitaptan tek kelime kalmaz; ekranı o anki görünüme uygun
sahte çıktı doldurur. Aynı tuşla kaldığınız yere dönersiniz.

İsterseniz panik tuşu bunun yerine çalışan bir sahte terminal açar
(`--hide=shell`): `ls`, `pwd`, `git status` beklenen çıktıyı verir, `npm`, `pytest`,
`cargo` gibi komutlar çıktı basar, bilinmeyen komutlar `command not found` der.

Dosya adı ekranda hiçbir zaman görünmez; `--help` ve `--version` bile bir derleme
aracınınki gibi durur.

## Görünümler

`m` ile görünümler arasında geçersiniz (`M` geri):

| Görünüm | Neye benzer |
| --- | --- |
| `code` | Bir editörde açık, bol belgelenmiş kaynak kod; kitabınız fonksiyonların açıklama yorumları olarak görünür (`l` ile dil: Python, TypeScript, Go, Rust, Java, C#) |
| `log` | Zaman damgalı uygulama logları |
| `diff` | Bir `git diff` çıktısı |
| `test` | Akan bir pytest oturumu |
| `build` | `cargo build` benzeri derleme çıktısı |
| `git` | `git log --stat` |
| `docker` | `docker compose up` logları |
| `agent` | Kod yazan bir yapay zekâ ajanının oturumu: araç çağrıları, renkli diff'ler, alt ajanlar |

Araya karışan sahte satırların miktarını `n` / `N` ile ayarlarsınız. Sahte satırlar
binlerce farklı satırdan seçilir, kısa sürede kendini tekrar etmez.

### agent görünümü

En inandırıcı görünüm bu. Açılışta bir karşılama kutusu ve karton miğferli, köpük
kılıçlı küçük bir figür çıkar: ara sıra kılıcını savurur, kılıç bazen bükülür, metin
akarken yerinde yürür.

Biri yanınıza gelirse `i` ile alttaki kutuya bir şey yazıp `Enter`'a basın; örneğin
"add retries to the upload client". Ajan birkaç saniye isteğiniz üzerinde çalışıyormuş
gibi yapar: düşünür, dosya okur, komut çalıştırır, diff gösterir ve kısa bir özetle
bitirir. Bu sırada ekranda kitaptan hiçbir şey kalmaz. Bitince ya da herhangi bir tuşa
basınca kitap kaldığı yerden devam eder.

## Kendiliğinden akış

`a` ile metin kendi kendine akar, tıpkı çalışan bir komutun çıktısı gibi. Hız okuma
hızınıza göredir; `+` / `-` ile ayarlanır. Program siz elle sayfa çevirdikçe
gerçek hızınızı öğrenir ve kendini ona göre ayarlar.

Ekranı bölmek de işe yarar: bir tarafta larpr, diğer tarafta gerçek işiniz
(Windows Terminal'de `Alt+Shift+D`).

## Okuma istatistikleri

`S` ne kadar okuduğunuzu gösterir: bugün ve bu hafta okunan sayfa ve süre, kaç gündür
aralıksız okuduğunuz, ortalama hızınız, kitabın bitmesine kalan süre ve son 14 günün
küçük bir grafiği. Rapor da o anki görünümün kılığına girer: `git` görünümünde
`git log --stat`, `test` görünümünde pytest özeti gibi durur.

Yalnızca gerçekten okuduğunuz sayfalar sayılır; hızla geçtiğiniz sayfalar ve panik
ekranında geçen süre sayılmaz.

## Tuşlar

| Tuş | Ne yapar |
| --- | --- |
| `Space` / `PgDn` | sonraki sayfa |
| `u` / `PgUp` | önceki sayfa |
| `j` / `k` (ya da oklar) | bir satır ileri / geri |
| `]` / `[` | sonraki bölüm / bölüm başı |
| `/` | kitapta ara; `f` / `F` sonraki / önceki sonuç |
| `b` | yer imi koy; `'` / `"` sonraki / önceki yer imi |
| `a` | kendiliğinden akışı aç / kapat; `+` / `-` hız |
| `m` / `M` | görünüm değiştir |
| `l` / `L` | kod dilini değiştir (`code` görünümü) |
| `n` / `N` | sahte satırları artır / azalt |
| `t` | renk temasını değiştir |
| `i` | `agent` görünümünde kutuya istek yaz |
| `S` | okuma istatistikleri |
| `o` / `O` | aynı klasördeki sonraki / önceki kitap |
| `Esc` | yan panel: tüm kısayollar ve klasördeki kitaplar (`1`-`9` ile aç) |
| `:` | ayar menüsü |
| `` ` `` | panik: her şeyi anında gizle / geri getir |
| `q` | kaydet ve çık |

Her kitabın yeri, yer imleri ve ayarlarınız kendiliğinden kaydedilir.

## Renk temaları

Varsayılan tema `monokai`: tam siyah zemin üzerinde kod renkleri; Apple Terminal dahil
her terminalde aynı görünür. `t` ile diğer temalara geçebilirsiniz:

- `dark`, `light`: koyu ve açık editör renkleri
- `dark-daltonized`, `light-daltonized`: renk körlüğüne uygun; yeşil/kırmızı yerine mavi/turuncu
- `dark-ansi`, `light-ansi`: terminalinizin kendi renk şemasını kullanır

Terminaliniz 24 bit rengi desteklemiyorsa renkler en yakın 256 ya da 16 renge
çevrilir; bir şey yapmanız gerekmez.

## Desteklenen biçimler

EPUB, PDF, MOBI / AZW / AZW3, FB2, DOCX, ODT, RTF, TXT / Markdown ve HTML.

Bilgisayarınızda [Calibre](https://calibre-ebook.com) kuruluysa listede olmayan
biçimler (XPS, CBZ, DJVU …) de açılır. DRM korumalı dosyalar açılmaz; taranmış
(yalnızca resimden oluşan) PDF'ler de okunamaz.

## Başlatırken seçenekler

Çoğunu program içinden tuşlarla da değiştirebilirsiniz; burada verdikleriniz
kaydedilir.

```
larpr build kitap.epub --mask=agent --auto --wpm=300
```

| Seçenek | Ne işe yarar |
| --- | --- |
| `--mask` | açılış görünümü: `code`, `log`, `diff`, `test`, `build`, `git`, `docker`, `agent` |
| `--lang` | `code` görünümünün dili: `python`, `typescript`, `go`, `rust`, `java`, `csharp` |
| `--noise` | sahte satır miktarı, 0-5 |
| `--auto` | kendiliğinden akışla başla |
| `--wpm` | okuma hızı, dakikada kelime (60-1500, varsayılan 220) |
| `--hide` | panik tuşu ne göstersin: `stream` (akan çıktı) ya da `shell` (sahte terminal) |
| `--theme` | renk teması (yukarıya bakın) |

## Diğer kurulum yolları

[cargo-binstall](https://github.com/cargo-bins/cargo-binstall) kullanıyorsanız hazır
programı tek komutla indirir:

```
cargo binstall --git https://github.com/burakbayramin/larp-reader larp-reader
```

Rust kuruluysa (1.88 ya da yenisi) kaynaktan derleyebilirsiniz:

```
cargo install --git https://github.com/burakbayramin/larp-reader
```

Kurulduğunu görmek için: `larpr --version`.

## Verileriniz

Her şey bilgisayarınızda kalır; program internete bağlanmaz. Ayarlar, kaldığınız
yerler ve istatistikler `~/.config/larpr` klasöründedir (Windows'ta `%APPDATA%\larpr`).
Kitapların adı ya da içeriği hiçbir dosyaya yazılmaz; kitaplar yalnızca bir kimlik
numarasıyla tanınır.

Daha önce programı eski adıyla (`fcc`) kullandıysanız ayarlarınız ilk açılışta yeni
klasöre taşınır.
