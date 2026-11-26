# VIctory0x — fork progress

Our fork of **Belobug/VIctory-Belobug-AMD**, a recompilation of GTA VI (Xbox Series X,
title `7952987A`) to run as a native Windows x64 executable. This file records what has
been integrated, what is verified working, what is still broken, and where the
performance work stands.

- Upstream: <https://github.com/Belobug/VIctory-Belobug-AMD>
- Fork: <https://github.com/Morpheus0x/VIctory0x>
- `main` mirrors upstream (`4f62c2a`), `dev` is the integration branch (`c133826`).

## Target environment

- Windows 10 x64 19045 (Windows 11 is not an option; Win10 limits are fixed, not worked around).
- Ryzen 9 9900X3D, AMD Radeon (RDNA), no Steam installed; Steam Controller used directly over HID.
- Game: `D:\GTA6\GTA6_XBOX\game_scarlett_master.exe`, console series-x.
- Data outside the repo: `D:\GTA6\victory0x-data\` with `config.toml`, `data\`, `cache\`, `logs\`.
  Nothing is written into the drive root or into the repo for runtime data.
- Repo: `D:\GTA6\VIctory0x`; launcher `run.bat` lives in the workspace root.

## Repository model

- `main` = upstream mirror; only fast-forwarded.
- `dev` = integration branch: upstream + our patches, always rebased (no merge commits).
- One feature branch per change set; **branches are never deleted**. Imported commits keep
  their original author; our own commits use `Morpheus0x <git@morpheus0x.com>`.
- "Merge" always means rebase + fast-forward. The tree stays portable between branches.
- Upstream sync: `git fetch upstream`, ff `main`, `git rebase main` on `dev` (git's patch-id
  skips commits already applied, including upstream commits imported through PR branches),
  rebuild, push. Rebase done 2026-11-26: `main` 4f62c2a, `dev` c133826.

## Integrated work

### Windows 10 / build

| Area | What | Branch |
|---|---|---|
| Agility SDK | Load the D3D12 Agility SDK runtime for shader model 6.6 on Win10; `cargo xtask agility`; tests use the SDK runtime | `win10-agility` |

### Input

| Area | What | Branch |
|---|---|---|
| Steam Controller | Native HID reader: enumerates Valve HID interfaces, decodes state packets, emulates XInput; lizard-mode feature report; no Steam / Steam Input in the path | `steam-controller` |

### Shader pipeline & warm-up

| Area | What | Branch |
|---|---|---|
| Warm cache | `translate --warm [--jobs N]` translates every shader into the run's cache ahead of a run; `run.bat` warms again whenever the exe hash changes (cache key is BLAKE3 of victory.exe + wave size) | `shader-warm` |
| Descriptor lookup | Loads look only in the block that writes a value; loaded sources listed once (400 twin descriptor loads: 363 ms → 11 ms) | `shader-lookup` (upstream #88) |
| IR build | Successor walk without allocation, register sets by set bits, read dedupe bitset (~15% faster IR build) | `shader-ir-alloc` (upstream #75) |
| Instruction decode | `s_atc_probe` immediate is not a register (fake SGPR write removed) — removes one source of "address in an SGPR" refusals | `shader-atc-probe` (upstream #186) |

### GPU correctness & performance

| Area | What | Branch |
|---|---|---|
| Dynamic descriptors | Cursor wraparound scan, per-heap failed-walk cap, capped headroom (fixes "no free dynamic descriptor slots" cascades) | `descriptor-fix` (upstream #89) |
| Per-draw overhead | Worker pool instead of per-call threads, per-heap locking, sparse-shadow write-watch, copy-back pinning | `perf-cpu-sync` (upstream #127, rebased with conflicts resolved) |
| Frame log | `VICTORY_FRAME_LOG`: per-5 s frame breakdown, long-frame records, GPU list meter, driver watch | `perf-frame-log`, `frame-log-format` |
| Texture read-back | `VICTORY_TEXTURE_BACK` copies shader-written water/ocean textures into title memory after submission (boat buoyancy) | `texture-back` (upstream #205) |
| Device paths | DualSense and Steam controller interface paths keep their NUL — `CreateFileW` read past the copy and crashed the process | `dualsense-path-nul` (upstream #156), `steam-path-nul` |
| Crash reports | dbghelp symbolization: crashes print `function at file:line` when the PDB sits beside the exe, not just module+RVA | `crash-symbols` |

### Upstream commits now in `dev` (4f62c2a)

Ambient light follows the sky dome and holds it; screen-probe ray program left out without
rays; clean-up passes over translated shaders with a trace of what they drop; pipeline
compiles no longer held endless; placed-texture LOD clamp; texture census of pool/host;
refused-pipeline shaders saved.

## Launcher (`run.bat`)

Hardcoded absolute paths, one variable per path, no `cd`/`%~dp0`. Resolves the config in
order `D:\GTA6\victory0x-data\config.toml` → repo `config.toml` → `victory.toml`, aborts
with a clear message if none exists. Creates cache/data/logs, warms shaders when the exe
hash differs from `cache\warm.sha`, then streams stdout+stderr to console and appends
UTF-8 (no BOM) to `logs\run.log` (PowerShell `Tee-Object` wrote UTF-16 and produced the
spaced-out log).

Enabled by default:

- `VICTORY_LOG=warn`, `RUST_BACKTRACE=1`
- `VICTORY_FRAME_LOG=1`, `VICTORY_FRAME_LOG_FILE=...\frame.log`
- `VICTORY_TEXTURE_BACK=64x64f16,128x128f36` (the two kinds that floated the boat on NVIDIA;
  the full water set is `1` = `256x256f41,256x256f16,64x64f16,128x128f36`)
- `VICTORY_AMBIENT=0.6`, `VICTORY_HAIR=1`
- `VICTORY_SKIP_PIPELINES=4e35b816…,b3ba0ef9…,5d627391…,ec6e4c36…,c89489af…,45f08139…,63083c77…,0bd4d80a…`

Deliberately **not** set by default (each was a large slowdown):

- `VICTORY_GPU_TIMING` — times every list, serialises submissions.
- `VICTORY_PROFILE` — sampling profiler, suspends the submit thread every 1 ms.
- `victory_loader=trace,victory_gpu=debug` logs — synchronous console writes per line.

## Verified milestones

1. Menus and loading screen render (initial bring-up).
2. Prologue (Lucia) runs; the crash after the cafe/flashback sequence is fixed
   (descriptor slot exhaustion + `CreateFileW` path over-read).
3. Extended session (~75 min) without a crash after the path fixes.
4. Jason's boat mission: the boat floats, drives and steers with `VICTORY_TEXTURE_BACK`
   set — previously it sat on the seabed and the mission failed with "Cal fell off the boat".
5. ~20–25 minutes of continuous world gameplay in the diag build; frame log produced.

## Performance state (2026-11-26, diag build, `frame.log`)

30 fps = 33 ms/frame. Current readings:

- Usual frame: **84–180 ms** (median 170–225 ms); bursts of 234–814 ms long frames.
- `execute all` (GPU): 15–430 ms per frame.
- `preparing` (CPU submission): 40–385 ms; `copies` 50–190 MB/frame in heavy frames.
- `textures`: 1–75 ms refresh, **500–13,500 uploads** per frame in bursts.
- `cardmeter lists`: 30–285 ms; `submissions` 20–95 per frame.
- Disk streaming: 0–837 reads, 0–61 MB in long frames (DStorage worker time up to 9.5 s).
- Faults: 1,600–38,000 page faults and 30–115 guard faults per 5 s window.
- Memory free ~2.3–2.6 GB.

Cost drivers, in order: **texture upload volume**, **submission/preparing time**, **GPU
execute**, **disk streaming bursts**, **page faults/remaps from sparse shadows**.

### Improvement avenues

1. **Texture uploads** — thousands of uploads and 100–190 MB of copies in burst frames.
   Suballocation/staging ring, batch by resource, skip unchanged mips, keep small textures
   resident. Biggest single lever toward 30 fps.
2. **Descriptor slots** — one `no free dynamic descriptor slots n=259 spare=1048576` at
   00:09 in the last session: slots are still leaking or the cap still starves a table.
   Instrument taken/free counts over time.
3. **Command-list cost** — `cardmeter lists` 30–285 ms; review fence waits, allocator,
   root-signature/PSO churn, and the cost of closing thousands of lists.
4. **Mesh shaders over 32 KiB** — three mesh pipelines dropped (`72da0ba1`, `8a621a22`,
   `60f97463`); upstream #138 links over-budget mesh/pixel shaders.
5. **Refused compute shaders** — ~30 hashes still fail with "address in an SGPR that is no
   root parameter" (`3cea4534`, `f6e47efa`, `3b90165a`, …). #186 removed one cause; #172
   (dead-branch pruning, also helps pipeline OOM) and #90 (EXEC from scalar) are rebased
   branches but their tests fail against our structure pass — needs a fix, not a merge.
6. **Disk streaming** — long frames correlate with DStorage read bursts; consider larger
   cache, read-ahead, or fewer, larger requests.
7. **Present / pacing** — `present` up to 90 ms in spikes; check swap-chain and pacing.

## Known issues / upstream watchlist

- Boat/water simulation: #196 (issue), #205 (fix, merged here), Marvisak #2248/#2573.
- Untranslatable compute shaders: upstream #141 (diagnosis), #172, #186 (merged here).
- Mesh over 32 KiB: upstream #138.
- Descriptor slot starvation: #89 (merged here) fixed the cascade; a residual leak is visible.
- Crash symbolization needs `victory.pdb` beside the exe (diag profile builds it).
- Mixed-DPI/present spikes and DStorage stalls are logged in `frame.log` long-frame records.

## Rebuild / run

```powershell
cd D:\GTA6\VIctory0x
git switch dev
git pull --ff-only
cargo xtask agility
cargo build --profile diag -p victory --features diagnostics
run.bat
```

Tests used for gating (Linux sandbox + Wine):

```
cargo test -p victory-shader          # 199 passed
cargo test -p victory-gpu --lib descriptor::host   # 15 passed
cargo check --target x86_64-pc-windows-msvc -p victory
```

## Appendix — `dev` commits on top of upstream (`main..dev`)

```
c133826 CaravelaLabs   Shaders: s_atc_probe's immediate is not a register
e63d3db TobiasFornell  Textures that shaders write and the title reads back are copied into title memory
dff785e Morpheus0x     HLE: Steam controller interface paths keep their NUL
e40b691 pjanes22       HLE: DualSense device paths keep their NUL
ee4bbec Morpheus0x     Crash reports name the function and line from the PDB beside the exe
ac10b04 Matheus        perf(gpu): cut per-draw and title-memory sync CPU overhead
588a21e romainhedouin  Shader translation walks a block's successors without allocating
86cdd1f romainhedouin  Descriptor loads look only in the block that writes a value
15ab965 dutch2005      GPU: fix dynamic descriptor cursor wraparound, per-heap walk cap
e88215f LukaManstein   translate --warm: --jobs 0 is an error instead of a run that warms nothing
8baae99 LukaManstein   translate --warm: every shader into the run's cache ahead of a run
0b16c51 Morpheus0x     Input: read a Steam Controller directly over HID, without Steam
5b4363b Morpheus0x     GPU: keep the frame log small and separable
9abcf8d dutch2005      GPU: harden frame log, card meter, and driver watch unit/GPU tests
ae72a38 dutch2005      GPU: add per-frame breakdown log, GPU list timestamp meter, driver watch
855608e Arvi23         Tests: the Agility exports are a linker directive of the test-only module
ee513fc Arvi23         GPU tests: use the Agility SDK runtime when it is there
0c735fe Arvi23         README: the Windows 10 step in this fork's section
9cf6397 GamingRipperDoc Shader tests: use the Agility SDK runtime when it is there
a6f7049 kykifox1337-source Windows 10: load the D3D12 Agility SDK runtime for shader model 6.6
```
