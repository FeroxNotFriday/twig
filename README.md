<div align="center">

# 🌿 twig

**A directory-tree query language.**
Filter with math, not flags.

[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-windows%20%7C%20linux%20%7C%20macos-lightgrey?style=flat-square)]()
[![Speed](https://img.shields.io/badge/speed-2.4%C3%97_faster_than_tree.com-success?style=flat-square)]()

</div>

## Why twig?

`tree` shows everything or nothing. `find` is a script. `Get-ChildItem` is a pipeline — you filter after listing, paying I/O twice.

`twig` makes the tree itself a query:

```sh
twig                              # full tree
twig src                          # only src/
twig rs                           # *.rs files anywhere
twig +10mb                        # files > 10 MB
twig -1d                          # modified in the last 24h
twig %target                      # everything except target/
twig rs $ toml                    # *.rs OR *.toml
twig ... rs $ toml ... +1kb       # (*.rs OR *.toml) AND > 1 KB
```

## Install

Binary lands at `target/release/twig` (`twig.exe` on Windows) — **2.0 MB**, statically linked, no runtime dependencies.

**From source (any OS):**
```sh
git clone https://github.com/FeroxNotFriday/twig.git
cd twig
cargo build --release
```
Requires Rust 1.85+. Or `cargo install --path . --force`.

**Windows:**
```powershell
[Environment]::SetEnvironmentVariable("Path", $env:Path + ";C:\path\to\twig\target\release", "User")
```

**Linux / macOS:**
```sh
sudo install -m755 target/release/twig /usr/local/bin/twig
```

## Shell compatibility

The syntax works **without quotes** in every shell. 

`$` is the OR operator. Twig's grammar always puts whitespace around it (`rs $ toml`), so your shell sees a bare `$` and treats it as a literal character. You only need to worry about quotes for globs in POSIX shells (`*.rs`), which bash expands before twig sees it:

```sh
twig '*.rs'                    # bash / zsh / fish — quote the glob
twig *.rs                      # PowerShell / cmd — no quotes needed
twig rs                        # universal — extension form never needs quotes
```

If your terminal shows mojibake for `├──` instead of the correct box drawing, run `chcp 65001` (Windows legacy) or switch to Windows Terminal.

## Syntax

A twig command is **a path** followed by **a query expression**. The first argument that exists on disk becomes the root; everything else is the expression.

```
twig  [path]  [expression]
      ↓       ↓
      src     ... rs $ toml ... +1kb %target
```

**Operators**

| | Meaning | Example | Reads as |
| :---: | :--- | :--- | :--- |
| `+` | greater / older | `+10mb` `+2h` | > 10 MB, older than 2 h |
| `-` | less / newer | `-100b` `-1d` | < 100 B, newer than 1 day |
| `=` | exactly | `=0` `=100l` | == 0 B, exactly 100 lines |
| *(none)* | at least (≥) | `100b` `2h` | ≥ 100 B, at least 2 h old |
| `$` | OR | `rs $ toml` | Rust OR TOML |
| `%` | NOT | `%target` | not target |
| `...` | group | `... a $ b ... +1kb` | (a OR b) AND > 1 KB |

Adjacency means AND. `rs +1kb -1d` reads as `.rs AND > 1 KB AND newer than a day`.

**Units**

| Suffix | Means | Example |
| :---: | :--- | :--- |
| `b` `kb` `mb` `gb` `tb` `pb` | size | `+100mb` `-1gb` |
| `ns` `us` `ms` `s` `m` `h` `d` `w` `mo` `y` | time | `-30m` `+2h` `-1w` |
| `l` | lines | `+100l` |
| `c` | UTF-8 chars | `-500c` |

Note: `m` = minutes, `mb` = megabytes, `mo` = months. Longest suffix wins.

**What you can write**

| You type | Parsed as | Matched against |
| :--- | :--- | :--- |
| `rs` `rs,toml` `.rs` | extension list | file extension |
| `*.rs` `foo?` | glob | file name or rel path |
| `/^src\//` *(with slashes)* | regex | file name or rel path |
| `main.rs` `Cargo` | substring | file name or rel path |
| `+10mb` `-1d` `=100l` | quantity | size / mtime / LOC / chars |

Regex **only** with `/.../ ` — `^main` without slashes is a literal substring.

**Directories and filters**

Directories are transparent to size/time/LOC filters. `twig +10mb` doesn't exclude `src/` just because the folder itself isn't a 10 MB file — it keeps the folder and checks the files inside. 

For name-based filters (`Ext`, `Glob`, `Regex`, `Substring`), a match keeps the folder; no match is treated as "not applicable", so the folder is still traversed. This is why `twig %target` prunes the whole `target/` subtree, but `twig rs` keeps `src/`.

**Sorting and Pagination**

Directories are always listed before files. Within each group, names are sorted alphabetically (case-insensitive). 
If you use `-n N`, twig will show the first `N` entries per directory and append `... (and K more items)` to indicate the hidden remainder.

## Recipes

**Project analysis**
```sh
twig -szltc                          # full metadata: sizes, disk, LOC, dates, counters
twig rs -s                           # every .rs file with its size
twig ... rs $ toml ... -1d           # recent Rust + TOML changes
twig +100l -1w rs                    # big files changed this week
```

**Disk cleanup**
```sh
twig -z -d 2                         # what's eating my disk (top 2 levels)
twig +100mb                          # individual files > 100 MB
twig +1y +50mb                       # old and big — archive candidates
twig %target %'*.lock' -z            # ignore build artifacts, show disk usage
```

**Mixing shell variables with OR**
```bash
# bash / zsh
PROJECT=src
twig "$PROJECT" $ toml               # src/*.rs OR src/*.toml
```
```powershell
# PowerShell
$dir = "src"
twig $dir $ toml                     # src/*.rs OR src/*.toml
```

**Scripting**
```sh
twig ... rs $ toml ... -r -s > files.txt
twig rs +1kb -j | jq -r '.children[].path'
```

## Flags

Flags exist for what is not a filter: display, traversal, output.

| Flag | Does |
| :--- | :--- |
| `-s` | show per-file size |
| `-z` | show cumulative folder size (disk usage) |
| `-l` | show line count |
| `-t` | show relative mtime (`2h ago`) |
| `-c` | show `[Nf, Nd]` counters per folder |
| `-a` | include hidden files (`.git`, `.env`) |
| `-d N` | max depth |
| `-n N` | max entries per directory |
| `-D` | directories only |
| `-x` | stay on one filesystem |
| `-r` | raw output (no ANSI colors) |
| `-j` | JSON export (nested) |
| `--gitignore` | respect `.gitignore` |

## Speed

`twig` walks the same filesystem as `tree` but does more per entry — 3-valued logic, path checks, extension dispatch, sorting. It wins anyway.

Benchmarked on Windows 11, 12-core x86_64, NVMe SSD. 3 warm-up runs, 10 timed iterations, median reported.

**Project directory (600 files, 313 MB `target/`)**

```text
twig complex    ████                                               7.6 ms    (4.7× faster than tree.com)
twig rs         ███████                                           13.1 ms
twig            ████████                                          15.3 ms
tree.com        ███████████████████                               36.0 ms
cmd dir /s      ██████████████████████████████████                61.4 ms
Get-ChildItem   █████████████████████████████████████████████     84.6 ms
```

**System32 (~4800 files, hundreds of subdirs)**

```text
twig rs         ████████                                         136 ms    (1.6× faster than tree.com)
twig complex    ████████                                         139 ms
tree.com        ████████████                                     214 ms
twig            █████████████                                    230 ms
cmd dir /s      █████████████████                                305 ms
Get-ChildItem   ████████████████████████████████████████████     746 ms
```

Filtering makes `twig` faster, not slower. A query that prunes a subtree at the walker level costs nothing and saves everything — `%target` on the project never enters the 313 MB directory. On System32 the bare tree is no faster than `tree.com` (twig pays for the sort + path stripping that `tree` skips) — but `rs` and `complex` still beat it by 1.6×.

**Metadata flags are priced honestly:**

| Flag | Project | System32 | What it does |
| :---: | ---: | ---: | :--- |
| `-s` | +3% | -7% | per-file size |
| `-z` | +4% | -9% | cumulative disk usage |
| `-t` | +5% | -9% | relative mtime |
| `-c` | +16% | -9% | `[Nf, Nd]` counters |
| `-l` | +29% | +262% | reads every text file |
| `-j` | +238% | +366% | JSON serialisation |
| `-szltc` | +29% | +278% | all of the above |

`-l` reads every text file fully up to 4 MB — no truncation, no silent wrong numbers. Files larger than 4 MB, and known binaries (`.exe`, `.pdb`, `.rlib`, `.dll`, `.png`, etc.), get `line_count = None`. This is why `-l` is expensive on System32 (hundreds of `.xml` / `.config` / `.log` files) and cheap on a project.

## How it works

- **Compiled matchers.** Every `Regex` and `GlobMatcher` is built once, not per file.
- **Walker-level pruning.** `%target` returns `false` from `filter_entry`; the `target/` directory is never entered.
- **Three-valued logic.** A directory evaluates filters as `None` (not applicable), not `false`. A directory is excluded only when the expression resolves to `Some(false)`.
- **SIMD line counting.** `memchr::memchr_iter` counts `\n` in chunks of 16 KB.
- **Binary skip for LOC.** `.exe` / `.pdb` / `.rlib` / images / archives are never opened for line counting.
- **Short-circuits.** `And` stops on first `false`, `Or` stops on first `true`.
- **LTO + panic=abort + strip.** Single static binary, no unwind tables.

## Behaviour notes

- Path first, expression second. `/target/`, `./target/`, `target/` all resolve the same on Windows. On Unix, `/target/` is an absolute path.
- `./` and `.\` are stripped — `%./target` = `%target`.
- `...` is never a path — 3+ dots skipped by path resolution.
- Hidden excluded by default. `-a` reveals. `.gitignore` opt-in via `--gitignore`.
- Empty dirs are pruned **only if a filter is active** and neither `-D` nor `-d` is set.
- `-r` strips ANSI only, not UTF-8.
- `-j` output is nested — each node has `children: []`, mirrors `TreeNode`.
- Duplicate flags idempotent — `-a -a`, `-szltc -szltc`, `-d 2 -d 3` (last wins).
- `$` needs whitespace — `rs $ toml` (correct), `rs$toml` (shell eats it).

---

<div align="center">
<p>MIT License · see <a href="LICENSE">LICENSE</a></p>
<p>Made with 🦀 in Rust</p>
</div>