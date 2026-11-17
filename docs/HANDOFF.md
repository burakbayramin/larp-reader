# larp-reader: devir notu

Son güncelleme: 2026-11-17. Bu dosya, işe başka bir bilgisayardan (ya da yeni bir
Claude oturumundan) devam etmek için gereken her şeyi toplar: proje ne, nerede
duruyor, nasıl çalışılıyor, sırada ne var.

## Bir bakışta

- **Ne:** terminalde e-kitap okuyucu; metni geliştirici aracı çıktısı kılığında
  gösterir (code, log, diff, test, build, git, docker, kubectl, terraform, sql, jupyter, agent görünümleri). Panik
  tuşu (`` ` ``) kitabı anında gizler.
- **Ad:** paket `larp-reader`, komut `larpr`. Eski adı `fcc` idi.
- **Dil:** Rust 2024, en az 1.88. Python sürümü kaldırıldı.
- **Depo:** https://github.com/burakbayramin/larp-reader (herkese açık, MIT).
- **Son sürüm:** v0.7.0, dört platform (Windows zip, macOS Apple Silicon / Intel,
  Linux tar.gz) + `SHA256SUMS`.
- **Homebrew:** https://github.com/burakbayramin/homebrew-tap,
  `brew install burakbayramin/tap/larpr`.
- **Durum:** 467 test geçiyor, fmt ve clippy temiz. v0.7.0 sonrası işler
  (aşağıda "Bu dönemde yapılanlar") `main`'de ama push edilmemiş olabilir:
  `git status -sb` ile bakın.

## Yeni bilgisayarda kurulum

```sh
git clone git@github.com:burakbayramin/larp-reader.git
cd larp-reader
# Rust: https://rustup.rs  (rustup default stable)
cargo test                                   # 467 test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo build --release                        # target/release/larpr
gh auth login                                # sürüm, iş akışı ve tap işleri için
```

Denemek için herhangi bir metin dosyası yeter:

```sh
./target/release/larpr build kitap.txt --mask=agent
```

Program ayarları ve ilerlemeyi `~/.config/larpr` (Windows'ta `%APPDATA%\larpr`)
klasöründe tutar; eski `~/.config/fcc` klasörü ilk açılışta oraya taşınır.

## Kod haritası

| Yer | Ne yapar |
| --- | --- |
| `src/main.rs` | terminal döngüsü (ratatui), sessiz panic kancası |
| `src/cli.rs` | `larpr build [DOSYA] --mask … --theme …`; seçenekler ayara kaydedilir |
| `src/book.rs`, `src/text.rs` | kitap modeli (kimlik = dosyanın SHA-256'sının ilk 16 hanesi), satır kırma (`ragged`, `ragged_count`) |
| `src/loaders/` | EPUB, PDF (iki sütunlu sayfalar dahil), MOBI/AZW3, FB2, DOCX, ODT, RTF, TXT/MD, HTML; Calibre yedek yolu |
| `src/masks/` | on iki görünüm; `Mask` trait'i `mod.rs`'te. `agent/` (alt modüller: `figure`, `welcome`, `reply`, `subagents`, `spinner`, `tools`, `draw`) karşılama kutusu, larper figürü, alt ajanlar, giriş kutusunun yanıtı. `time.rs` oturum başlangıcından türeyen zaman damgaları (log, git). `filler.rs` / `codegen.rs` tekrar etmeyen dolgu havuzları. `look.rs` tipli stiller (`Look`, `Role`) |
| `src/session.rs` | satır numaralama, sayfalama, bölüm geçişleri; derin atlama için sayım önbelleği |
| `src/state.rs`, `src/stats.rs` | ayar, ilerleme, istatistik JSON'ları (atomik yazma) |
| `src/keymap.rs` | `config.json` içindeki `keys` nesnesi: eylem adı → tuş(lar), varsayılanlar, çakışma/bilinmeyen eylem doğrulaması; panik tuşu taşınabilir ama kaldırılamaz |
| `src/pace.rs`, `src/noise.rs` | okuma hızı öğrenme, dolgu miktarı |
| `src/ui/app.rs` + `src/ui/app/` | `App` durum makinesi: durum ve açılış `app.rs`'te; `draw`, `keys`, `commands`, `timing`, `save` alt modülleri |
| `src/tick.rs`, `src/sync.rs`, `src/export.rs` | `larpr tick` (tmux tek satır), `sync_dir` ile bilgisayarlar arası ilerleme eşitleme, `larpr export` (yer imleri Markdown) |
| `src/ui/` diğerleri | `search`, `timer` (akış ve hız), `panel`, `menu`, `shell` (sahte kabuk), `reply` (agent kutusunun yanıtı), `report` (`S` istatistik ekranı), `theme` (7 tema, renk derinliği), `feed` |
| `src/ui/golden.rs` + `golden.txt` | her görünüm × tema × renk derinliği için ekran özeti testi |
| `src/ui/app/tests/` | arayüz testleri (`Harness`: tuşa bas, zamanı ilerlet, ekranı oku) |
| `examples/deep_jump.rs` | derin atlama ölçümü: `cargo run --release --example deep_jump` |

Değişmemesi gereken kurallar (testler denetler):

- Ekranda "book", "reader", "okuyucu" kelimeleri ve dosya adı görünmez.
- Panik açıkken tek kitap kelimesi kalmaz.
- `line_count == render().len()`; satırlar genişliğe sığar.
- Aynı paragraf her çizimde aynı çıkar (deterministik tohumlar).
- Sayfalama her satırı bir kez gösterir.

## Çalışma düzeni

- **Commit'ler:** her özellik ayrı commit, her commit kendi başına testlerden geçer.
  Mesajlar İngilizce, conventional (`feat:`, `fix:`, `test:`, `docs:`, `ci:`,
  `chore:`); Claude ile yapılanlar
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>` satırıyla biter.
- **Push:** yalnızca istendiğinde.
- **Her commit'ten önce:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test`. Testlerin sonucuna çıktıdaki `FAILED` / `panicked` ile bakın; bir
  boru hattının çıkış kodu test hatasını gizleyebiliyor (bir kez kırık bir sürüm
  commit'i böyle push edildi).
- **Golden test:** ekran çıktısını bilerek değiştiren bir iş yaptıysanız
  `FCC_BLESS=1 cargo test golden`, sonra `git diff src/ui/golden.txt` ile yalnızca
  beklenen satırların değiştiğine bakın.
- **Windows farkı:** agent görünümünün madde işareti Windows'ta `●`, diğerlerinde `⏺`.
  Testlerde işareti elle yazmayın, `crate::masks::agent::BULLET` kullanın.
- **Paralel ajanlarla çalışırken:** her ajan kendi git worktree'sinde çalıştı,
  işleri `git cherry-pick` ile `main`'e alındı. `git cherry-pick --abort` dizinin
  başına kadar geri sarar; dikkat.

## Sürüm çıkarma

1. `Cargo.toml`'da sürümü artır, `cargo build` (Cargo.lock güncellenir), testler.
2. `chore: release X.Y.Z` commit'i, push, CI'ın yeşil olmasını bekle.
3. `git tag vX.Y.Z && git push origin vX.Y.Z` → `release.yml` dört platformu derler
   ve sürümü oluşturur.
4. **Homebrew tarifini güncelle:** `homebrew-tap` deposunda `Formula/larpr.rb`
   içindeki üç adresteki sürümü ve üç `sha256`'yı yeni sürümün `SHA256SUMS`
   dosyasından al; `brew audit --tap burakbayramin/tap` temiz çıkmalı.
   (`version` satırı yazmayın; denetim gereksiz der.)

Var olan bir etiketi yeniden derleyip dosyalarını değiştirmek için:
`gh workflow run release.yml -f tag=vX.Y.Z` (testleri atlar, `gh release upload --clobber`
ile yükler).

Notlar: v0.6.0'da yalnızca macOS dosyaları var (o sırada Actions limiti doluydu,
yerelde derlendi). Depo herkese açık olduğundan Actions artık ücretsiz.

## Bu dönemde yapılanlar (özet)

- Python sürümü Rust'a taşındı (aşama 0-7), v0.5.0 / v0.5.1 yayımlandı; Python kaldırıldı.
- Renkler: Apple Terminal için 256/16 renk eşleme, tema arka planları, hücre
  genişliği ölçümü; yedi tema, varsayılan `monokai` (tam siyah zemin).
- agent görünümü: spinner ve fiiller, daktilo efekti, giriş kutusu, bölüm geçişleri,
  karşılama kutusu ve köpük kılıçlı larper figürü (kaldığın yerden açınca da görünür),
  ara ara alt ajan blokları, `i` ile yazılabilen kutu ve sahte yanıt (yanıt
  sırasında ekranda kitap metni kalmaz).
- Mimari: `App` modüllere bölündü; altı görünümde tekrar etmeyen dolgu; PDF'te sütun
  sütun okuma ve paragraf bölme; derin atlama hızı (20 bin paragrafta 340 ms → ~6-110 ms);
  tipli stiller ve App'e ait tema.
- Renkli durum satırı, her görünümün sağ köşesinde kendi bilgileri.
- Okuma istatistikleri (`S`), `stats.json`, gün sınırı `chrono` ile yerel saate göre.
- Proje adı `larp-reader` / `larpr`; depo yeniden adlandırıldı ve herkese açıldı;
  README İngilizce ve kullanıcıya yönelik; MIT lisansı; `cargo binstall` desteği;
  Homebrew tap'i; CI elle de başlatılabiliyor; sürüm iş akışı var olan etiketi
  yeniden derleyebiliyor.

## Bu dönemde yapılanlar (v0.7.0 sonrası)

- `agent.rs` ve `app.rs` alt modüllere bölündü (davranış değişmedi, golden aynı).
- Yeni görünümler: `kubectl`, `terraform`, `sql`, `jupyter`.
- Log ve git görünümlerinde gerçek saate yakın zaman damgaları; açık terminalde
  açık tema otomatik.
- `larpr tick`, `larpr export`, `sync_dir` eşitlemesi, `keys` ile tuş ayarı.
- Düzeltmeler: panik sonrası dolgu tekrarı, yanıt sırasında tuşun yutulması,
  `test` görünümü yüzdesi, kısa ekranların istatistiği, PDF (sütun arası cümle,
  sayfa numarası, kısa hücreli tablo).

## Sırada ne var

### 1. npm / npx ile dağıtım (konuşuldu, başlanmadı)

JavaScript ile yeniden yazmak gerekmiyor; derlenmiş `larpr` npm'den dağıtılır
(esbuild / Biome yöntemi). `larpr` ve `larp-reader` adları npm'de boş (kontrol
edildi). Plan:

- Dört platform paketi, her biri yalnızca kendi `larpr` dosyasını içerir ve
  `package.json`'da `"os"` / `"cpu"` ile hangi makine için olduğunu söyler:
  `@larp-reader/darwin-arm64`, `@larp-reader/darwin-x64`,
  `@larp-reader/linux-x64`, `@larp-reader/win32-x64`.
- Ana paket `larpr`: bu dördünü `optionalDependencies` olarak listeler ve
  10-15 satırlık bir başlatıcı içerir (doğru dosyayı bulup `stdio: "inherit"`
  ile çalıştırır, çıkış koduyla çıkar). Kurulumda dosya indiren `postinstall`
  yöntemi önerilmiyor (`--ignore-scripts` ve proxy'ler yüzünden kırılgan).
- Repoya `npm/` klasörü; `release.yml`'a etiket push'unda beş paketi aynı sürüm
  numarasıyla yayımlayan bir adım.
- **Kullanıcının yapacağı:** npm hesabı, `larp-reader` npm organizasyonu, bir
  yayın token'ı (automation) ve onu GitHub deposuna `NPM_TOKEN` gizli değişkeni
  olarak eklemek.
- Sonuç: `npx larpr build kitap.epub` ya da `npm install -g larpr`.

### 2. Küçük işler

- **Panik tuşu taşınınca:** `ui/input.rs` ve menü hâlâ backtick'i özel sayıyor;
  panik tuşu başka bir tuşa taşınırsa giriş kutusuna backtick yazılamıyor.
- **Otomatik açık tema:** yalnızca `COLORFGBG` okunuyor (OSC 11 sorgusu yok).
  Otomatik seçilen tema ayara yazılmıyor, ama uygulama ayarı başka bir sebeple
  kaydederse (örneğin `t` ile tema değişince) kalıcı olur.

### 3. Teknik borç ve bilinen kusurlar

- **Bayat dokümanlar:** `docs/superpowers/` altındaki plan ve spec dosyaları
  Python dönemine ait ve `fcc` adını kullanıyor; tarihî kayıt olarak duruyorlar
  (dosyaların başında not var).
- **PDF:** hücreleri satır başına ortalama 3 kelimeden uzun olan tablolar hâlâ
  sütunlara bölünebiliyor; tablo satırları tek paragrafta birleşiyor.
- **Derin atlama:** 20 bin paragraflık (~22 MB) yapay kitapta ilk soğuk atlama
  ~110 ms; gerçek bir romanda ~10 ms beklenir ama ölçülmedi.

## Claude için notlar

Claude'un bu bilgisayardaki hafızası diğer bilgisayara taşınmaz; önemli olanlar burada:

- Kullanıcı Türkçe yazıyor; cevaplar Türkçe.
- Her özellik ayrı commit, her commit testlerden geçer.
- Push yalnızca istendiğinde ("pushla" gibi).
- İşler çoğu zaman ajanlara bölündü: ağır ve dikkat isteyen iş Opus'a, desen
  belli olan iş Sonnet'e. Ajanlar ayrı worktree'lerde çalışır, işleri
  cherry-pick ile alınır, sonra worktree ve dalları silinir.
