# larp-reader

larp-reader opens your e-book in the terminal and dresses every line up as
developer-tool output: source code, a build log, a diff, a test run or an AI coding
agent's session. Anyone walking past your screen sees someone watching a build.
If they come closer, one key makes the book disappear completely.

The name comes from the slang *larp*: to pretend, to act the part. The command is
`larpr` for short.

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
    213 +         Call me Ishmael. Some years ago, never mind how
    214 +         long precisely, having little or no money in my purse
```

The green lines are your book; everything else is fake.

## Quick start

On macOS or Linux with [Homebrew](https://brew.sh):

```
brew install burakbayramin/tap/larpr
```

Otherwise:

1. Download the archive for your system from the [Releases](../../releases) page
   (Windows, macOS Apple Silicon / Intel, Linux).
2. Unpack it and put `larpr` (`larpr.exe` on Windows) in a folder on your PATH.
   Nothing else to install.

Then open your book:

```
larpr build book.epub
```

Next time just type `larpr build`: the last book you read opens where you left off.

`Space` to read, `q` to quit. Someone coming? `` ` `` (backtick).

## The panic key

The moment you press `` ` ``, not a single word of the book stays on screen; fake
output that matches the current view fills it instead. Press it again to get back
to where you were.

You can make the panic key open a working fake shell instead (`--hide=shell`):
`ls`, `pwd` and `git status` give the output you'd expect, tools like `npm`,
`pytest` or `cargo` print output, and unknown commands say `command not found`.

The file name never appears on screen; even `--help` and `--version` look like a
build tool's.

## Views

Switch views with `m` (`M` goes back):

| View | Looks like |
| --- | --- |
| `code` | Well-documented source code in an editor; your book shows up as the functions' doc comments (`l` changes the language: Python, TypeScript, Go, Rust, Java, C#) |
| `log` | Timestamped application logs |
| `diff` | `git diff` output |
| `test` | A running pytest session |
| `build` | `cargo build`-style compiler output |
| `git` | `git log --stat` |
| `docker` | `docker compose up` logs |
| `agent` | An AI coding agent at work: tool calls, coloured diffs, sub-agents |

`n` / `N` set how much fake output is mixed in. The fake lines are drawn from
thousands of different ones, so they don't repeat any time soon.

### The agent view

The most convincing view. It opens with a welcome box and a small figure in a
cardboard helmet with a foam sword: now and then it swings the sword, sometimes the
sword bends, and it marches in place while text streams.

If someone walks up, press `i`, type something into the box at the bottom and hit
`Enter`, say "add retries to the upload client". The agent pretends to work on it
for a few seconds: it thinks, reads files, runs commands, shows a diff and wraps up
with a short summary. Nothing from the book is on screen meanwhile. When it's done,
or when you press any key, the book continues where it was.

## Auto scroll

With `a` the text scrolls by itself, like the output of a running command. The
speed follows your reading speed; adjust it with `+` / `-`. As you turn pages by
hand, larpr learns how fast you actually read and adapts.

Splitting the terminal works well too: larpr on one side, your real work on the
other (`Alt+Shift+D` in Windows Terminal).

## Reading stats

`S` shows how much you've read: pages and time today and this week, how many days in
a row you've been reading, your average speed, the time left to finish the book, and
a small chart of the last 14 days. The report is disguised as well: in the `git`
view it looks like `git log --stat`, in the `test` view like a pytest summary.

Only pages you actually read count; pages you flip through quickly and time spent
on the panic screen don't.

## Keys

| Key | What it does |
| --- | --- |
| `Space` / `PgDn` | next page |
| `u` / `PgUp` | previous page |
| `j` / `k` (or arrows) | one line down / up |
| `]` / `[` | next chapter / start of chapter |
| `/` | search the book; `f` / `F` next / previous match |
| `b` | set a bookmark; `'` / `"` next / previous bookmark |
| `a` | auto scroll on / off; `+` / `-` speed |
| `m` / `M` | change view |
| `l` / `L` | change code language (`code` view) |
| `n` / `N` | more / less fake output |
| `t` | change colour theme |
| `i` | type a request into the box (`agent` view) |
| `S` | reading stats |
| `o` / `O` | next / previous book in the same folder |
| `Esc` | side panel: all shortcuts and the books in the folder (open with `1`-`9`) |
| `:` | settings menu |
| `` ` `` | panic: hide everything at once / bring it back |
| `q` | save and quit |

Your place in every book, your bookmarks and your settings are saved automatically.

## Colour themes

The default theme is `monokai`: code colours on a pure black background, looking
the same in every terminal, Apple Terminal included. Press `t` for the others:

- `dark`, `light`: dark and light editor colours
- `dark-daltonized`, `light-daltonized`: colour-blind friendly, blue/orange instead of green/red
- `dark-ansi`, `light-ansi`: use your terminal's own colour scheme

If your terminal doesn't support 24-bit colour, colours are mapped to the nearest
256 or 16; there's nothing to set up.

## Supported formats

EPUB, PDF, MOBI / AZW / AZW3, FB2, DOCX, ODT, RTF, TXT / Markdown and HTML.

With [Calibre](https://calibre-ebook.com) installed, formats not on this list (XPS,
CBZ, DJVU …) open too. DRM-protected files don't open, and scanned PDFs (pages that
are only images) can't be read.

## Start-up options

You can change most of these with keys inside the program as well; whatever you
pass here is saved.

```
larpr build book.epub --mask=agent --auto --wpm=300
```

| Option | What it does |
| --- | --- |
| `--mask` | view to start in: `code`, `log`, `diff`, `test`, `build`, `git`, `docker`, `agent` |
| `--lang` | language of the `code` view: `python`, `typescript`, `go`, `rust`, `java`, `csharp` |
| `--noise` | amount of fake output, 0-5 |
| `--auto` | start with auto scroll |
| `--wpm` | reading speed in words per minute (60-1500, default 220) |
| `--hide` | what the panic key shows: `stream` (scrolling output) or `shell` (fake shell) |
| `--theme` | colour theme (see above) |

## Sync between computers

To keep your place on several machines, point `sync_dir` in the settings file
(`config.json` in the data folder, see below) at a folder that something else
syncs for you (Dropbox, Syncthing, a network drive):

```json
{ "sync_dir": "C:/Users/you/Dropbox/larpr" }
```

Your place and bookmarks for each text are written there under its ID number, so
the same text opened from a different path on another computer is matched by its
contents. The synced files hold no file name, path or title. If two computers
disagree, the newest one wins. A missing or unwritable folder is ignored silently.
Leave `sync_dir` empty (the default) to turn this off.

## Status bar mode

`larpr tick` prints one line that looks like CI or git status and carries the
next few words of the text you last had open, then moves your place forward by
what it showed. Put it in a tmux status bar and the text trickles by:

```
set -g status-interval 5
set -g status-right '#(larpr tick --width 70)'
```

`--width` is the widest the line may be (default 80). With nothing open, or if
the file is gone, it prints a neutral "all checks passed" line and exits 0.

## Exporting bookmarks

`larpr export` prints the bookmarks of a book as Markdown: grouped by chapter, in
reading order, each with its paragraph quoted. Without a file it uses the last book
you opened; `--out` writes to a file instead of the terminal.

```
larpr export book.epub > bookmarks.md
larpr export --out bookmarks.md
```

The export names the book and quotes it, so it is the one thing larpr writes that
isn't disguised; it is only ever written where you ask.

## Other ways to install

With [cargo-binstall](https://github.com/cargo-bins/cargo-binstall), one command
fetches the ready-made program:

```
cargo binstall --git https://github.com/burakbayramin/larp-reader larp-reader
```

With Rust installed (1.88 or newer) you can build it from source:

```
cargo install --git https://github.com/burakbayramin/larp-reader
```

To check it's installed: `larpr --version`.

## Your data

Everything stays on your computer; the program never goes online. Settings, your
place in each book and your stats live in `~/.config/larpr` (`%APPDATA%\larpr` on
Windows). Book titles and contents are never written to any file; books are known
only by an ID number.

If you used the program under its old name (`fcc`), your settings move to the new
folder the first time you start it.

## License

MIT, see [LICENSE](LICENSE).
