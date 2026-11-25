# claude-GTA6 / VIctory fork

> **No game files are included or linked, and none ever will be.** You need your own copy of the game (your own dump of the
> Xbox Series version you own). This project is not affiliated with or endorsed by the game's developer or publisher.
> Issues or comments that ask for game files, or link to them, are deleted.

This build runs an Xbox Series title natively on Windows through Direct3D 12. It is built on
[fxceless/victory-fork](https://github.com/fxceless/victory-fork), with fixes from
[myokex/victory-amd](https://github.com/myokex/victory-amd), and all of it builds on
[Marvisak/VIctory](https://github.com/Marvisak/VIctory). Nearly all of the code is theirs. This repository adds keyboard
and mouse controls, tests each published commit on an RTX 5090, and sends general fixes upstream as pull requests. It is
worked on by a tester together with Claude (AI-assisted).

**It is not a playable game yet: it shows the opening room at about 2 frames per second.**

## Where it stands (2026-11-25, code commit eb5ce609)

Measured on one machine: Windows 11, NVIDIA RTX 5090, AMD Ryzen 9 9950X3D. Gate runs, no driver reset or device loss in
any of them.

| Build | Frame time in the world | Picture |
|---|---|---|
| fxceless/victory-fork a803c87c | 505-525 ms (about 2.0 fps), 15 min cold | the first room, without its walls |
| ceb7ef43 (the build before this one) | median 326 ms, 302 ms once settled (3.1-3.3 fps), 15 min cold | the opening room whole, but its interior dark |
| **this build, eb5ce609** | 445-466 ms (about 2.2 fps), 7 min warm | the interior lit for the first time: ceiling lights, walls, door, floor, bunks, hair and clothing in colour |

More is drawn now and it is slower than the build before it: about 6,900 draws per frame instead of 4,700.

Most of the earlier gains are not ours: the speed and picture of ceb7ef43 came with fxceless's commit c96b4f2b, and the
walls with myokex's fixes (among them c6dc4d8c). What lit the interior is ours (first item below).

## What this repository adds

- **Record counts of structured buffers read by index** (9ae17530): the title's shaders read a structured buffer's record
  count as data; it is now the buffer's raw size over its stride. This is what lit the room. The same finding went upstream
  as [#1762](https://github.com/Marvisak/VIctory/pull/1762) (open).
- **The loading crash fixed** (DirectStorage: a raw read the kernel cannot land in a guarded destination is read into our
  own buffer and copied in). It was seen in about 1 start in 3 on the build before.
- **Keyboard and mouse played like a PC game**: raw mouse input, letters by key position whatever the layout, every key
  rebindable in `[controls]` of `victory.toml` (see `victory.example.toml`). Also offered upstream as
  [#1529](https://github.com/Marvisak/VIctory/pull/1529) (open).
- **A compute fix**: the alpha-transmission prefilter now runs on 32-lane cards.
- **Pull requests to fxceless/victory-fork merged and tested together** (#32, #349, #229, #51, #280b, #25), by Noname0202,
  Kamiloj9, pgm1207, bawhen, AscilCH and armin; and myokex's fixes.
- **Tested settings**: `Play.cmd` uses only what passed our runs (see Settings).

## Build

Requirements: Windows 10 or 11 x64, a CPU with AVX2, a Direct3D 12 GPU with shader model 6.6 and mesh shaders, plenty of
free memory commit (the title uses around 20 GB in play), [Rust](https://rustup.rs) (the version is pinned in
`rust-toolchain.toml` and installed for you), and the Visual Studio Build Tools with the "Desktop development with C++"
workload (Rust's MSVC linker).

```powershell
git clone https://github.com/claude-GTA6/VIctory.git
cd VIctory
git checkout release
rustup toolchain install
cargo build --release -p victory
```

This produces `target\release\victory.exe` (about 15 minutes on our machine while it was busy with other builds).

Optional checks (the tests need no game files; some run on the GPU, and a few use the DXIL validator `dxil.dll` from the
Windows SDK):

```powershell
cargo xtask policy
cargo test --workspace
```

## Run

Double-click **`Play.cmd`** and drag the title's `.exe` (or the folder that holds it) onto it, or paste its path when asked.
It remembers the path and starts the title at high priority. By hand:

```powershell
$env:VICTORY_RELAY = "0"
target\release\victory.exe run "<your game folder>\<the title's exe>" --execute
```

The first run in the world compiles thousands of shaders and stutters for several minutes; they are cached for the next
run. Saves, settings and `crashes.log` are kept in `%LOCALAPPDATA%\victory\data`.

## Settings

| Setting | Value | Why |
|---|---|---|
| `VICTORY_RELAY` | `0` | the relay is on by default in this code (fxceless's setting); our numbers above were measured with it off and we have not finished testing it on our hardware. Remove the line from `Play.cmd` to use it |
| `[controls]` in `victory.toml` | see `victory.example.toml` | mouse sensitivity, inverted Y, key bindings |

Nothing else is needed. Leave `VICTORY_RELAY=2`, `VICTORY_LATE_WAIT=3`, `VICTORY_BVH_BUILDER` and `VICTORY_WAVE_WIDE` off:
in testing (ours or fxceless's) they caused crashes, device loss, a blue screen report or missing geometry.

## Known issues

- About 2 fps in the opening room; not playable.
- 106 structured buffer views are still left unconverted, so some lighting data is still wrong.
- Shadowed areas outside the opening room are untested.
- The loading crash is fixed in this build (seen in 1 start in 3 on the build before).
- A GPU hang or driver reset can still happen. Close the title and start it again.
- Tested on an RTX 5090 (ours) and the GPUs fxceless and myokex report. Other GPUs: please report.
- Windows error 1455 in `crashes.log` means not enough memory commit: close other programs or enlarge the paging file.

Bug reports are welcome as issues: GPU, driver version, what you saw, and `crashes.log`. Never attach game files or dumps.
Problems that also happen on fxceless/victory-fork belong there.

## Our pull requests to Marvisak/VIctory (all open)

- [#1951](https://github.com/Marvisak/VIctory/pull/1951): skip barriers whose before and after states are equal
- [#1762](https://github.com/Marvisak/VIctory/pull/1762): structured buffer views as raw views of the same bytes (NVIDIA
  read 0 through them, which left the world black; the diagnosis and fix came from fxceless, credited)
- [#1563](https://github.com/Marvisak/VIctory/pull/1563): link mesh and pixel programs whose mesh outputs pass 32 KiB
- [#1529](https://github.com/Marvisak/VIctory/pull/1529): keyboard and mouse played like a PC game, rebindable `[controls]`
- [#638](https://github.com/Marvisak/VIctory/pull/638): GPU diagnostics: what a hang waits for, a look under a black frame

## Credits

- **fxceless** and the contributors to [fxceless/victory-fork](https://github.com/fxceless/victory-fork): the work that got
  the world drawn and the frame rate up. This repository is their tree plus the changes listed above.
- **myokex** ([myokex/victory-amd](https://github.com/myokex/victory-amd)): the fixes that brought the room's walls back.
- **Noname0202, Kamiloj9, pgm1207, bawhen, AscilCH, armin**: the fxceless/victory-fork pull requests merged here.
- **Marvisak** and every [VIctory](https://github.com/Marvisak/VIctory) contributor: the loader, runtime, Direct3D 12 back
  end and shader recompiler under all of it.
- **Reviewers and testers** on our pull requests: adamdotpy, DenRakEiw, mercuryVM, MichaelSmithAI, Vaximous.
- Every author keeps their authorship in the history; only our own commits are ours.

## License

GPL-3.0-or-later, like fxceless/victory-fork and VIctory (see `LICENSE`). The base's own README, with its full change list,
commands and switches, is in [`docs/victory-README.md`](docs/victory-README.md).
