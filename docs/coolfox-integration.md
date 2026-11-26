# cool_fox baseline (`dev-coolfox`)

Source: <https://github.com/carlm125/cool_fox>, `main` at `f4b48ad` ("Publish the
pareto-residency Windows build and portable launcher", 2026-11-26).

It shares our history up to `8e79c80` (2026-11-24) and carries **426 commits**
since, including a merge of `avatarmanz/VIctory-7900XTX` (`0a43566`) plus NVIDIA
Wave32 lowering, movie-plane uploads, XMA fixes and 32 GB residency work. Where
it overlaps with our tree we keep **cool_fox's** implementation; our own fixes
are ported on top.

## What cool_fox brings (kept as-is)

From its README/findings, and visible in the tree:

- **Thin lists** (`VICTORY_THIN`): list calls are recorded, not executed, on the
  title's threads; eight worker threads translate them.
- **Asynchronous submission** (`VICTORY_ASYNC_SUBMIT`, `VICTORY_JOIN_PARTS`):
  a thread prepares submissions, parts are spread (refreshes, AVX2 compares,
  descriptor records, landings).
- **Relay/land** (`VICTORY_RELAY`, `VICTORY_LAND`): what shaders wrote comes
  back beside the write guards, by changed dwords.
- **Card steps / up queue**: buffer fills needing card bytes run on the card;
  textures upload on a copy queue and show in slots only after the upload ran.
- **Boats float**: texture read-back from fxceless/Geckomacabre is **on by
  default** (`VICTORY_TEXTURE_BACK` unset = the four water kinds).
- **Frame record and replay** with per-pass times (`docs/replay.md`).
- Its own descriptor allocator, watch layer and host copy model — newer and
  larger than ours, which is why our descriptor/watch patches are not applied.

## Ported from our fork

| Feature | Commits | Notes |
|---|---|---|
| Windows 10 Agility SDK (SM 6.6) | 5 commits from `win10-agility` | `cargo xtask agility`, `D3D12SDKVersion/Path`, `agility_runtime` cfg for tests. cool_fox documented Win10 but had no implementation. |
| Steam Controller over HID, no Steam | `Input: read a Steam Controller…` | `crates/victory-hle/src/modules/steam.rs`; gameinput slot-0 provider; `VICTORY_PAD_LOG`. |
| Steam interface paths keep their NUL | `HLE: Steam controller interface paths keep their NUL` | Fixes the `CreateFileW` crash we hit (RtlInitUnicodeStringEx read past the copy). |
| Crash reports name function + line | `Crash reports name the function and line from the PDB` | dbghelp symbolization beside the exe; needs `victory.pdb`. |

Our `descriptor-fix` (#89), `shader-lookup` (#88), `shader-ir-alloc` (#75) and
`shader-atc-probe` (#186) are **not** ported: cool_fox already carries its own,
newer code in those areas, and the patches conflict wholesale.

## Launcher, config, cache, saves

- `build.bat` (workspace root): `cargo run -p xtask -- agility`, then
  `cargo build --locked --profile diag -p victory --features diagnostics`.
- `run.bat` (workspace root): rewrites `D:\GTA6\victory0x-data\victory.toml`
  with cool_fox's schema
  (`[paths] game_dir/cache_dir/data_dir`), exports the same values as
  `VICTORY_*` variables, warms shaders with `victory shaders <shaders.rpf>` when
  the exe hash changed, then streams UTF-8 into `logs\run.log`.
- **Saves**: cool_fox stores persistent data at
  `<data_dir>\<TITLEID>\storage` and saves beside it — the same layout we used,
  so `D:\GTA6\victory0x-data\data\7952987A\storage` is picked up unchanged.
- **Shader cache**: cool_fox keys the store by the *translator fingerprint*
  (its `build.rs`), not the exe hash. The old cache entries are not reused; the
  first run after switching translates once (the launcher does it before
  starting). Later rebuilds of unrelated code reuse the store.
- The old `D:\GTA6\victory0x-data\config.toml` is left untouched but no longer
  read; the generated `victory.toml` carries the same three paths.

## Frame line

cool_fox prints its 5-second frame line through `victory_gpu::host` at info.
`run.bat` sets `VICTORY_LOG=warn,victory_gpu::host=info`, so the line goes to
the console and `logs\run.log` (there is no separate `frame.log` in this tree).
Its fields differ from our old frame log: preparing, copies, card steps, lands,
descriptor records, list calls.

## Verified in the sandbox

- `cargo check --locked --target x86_64-pc-windows-msvc -p victory`: clean.
- `cargo test -p victory-shader`: **203 passed**, 2 ignored.
- `victory.exe --help` / `config` under Wine (see below).

## Open questions

1. Keep this as a parallel branch, or switch `dev` to it after your test run?
2. Steam Controller mapping: the module assumes lizard-mode feature reports and
   maps to XInput; anything specific you want changed (deadzones, paddles)?
3. First switch re-translates the whole shader cache (new key). Acceptable as a
   one-time cost, or should the old store be converted?
4. Keep `VICTORY_SKIP_PIPELINES` and `VICTORY_AMBIENT=0.6` in the launcher, or
   start from cool_fox's defaults?
