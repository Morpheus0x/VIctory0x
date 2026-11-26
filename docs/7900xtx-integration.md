# Integrating avatarmanz/VIctory-7900XTX

Source: <https://github.com/avatarmanz/VIctory-7900XTX>, `main` at `0ce46d5`
(2026-11-26). Branch: `dev-7900xtx`, based on our `dev` at `f425067`.

## It is a different lineage, not a fork of ours

Its README names **Marvisak/VIctory** as its base and its history is a fresh
`Initial commit` (758 commits, 236 merges, 258 files). Our tree descends from
**Belobug/VIctory-Belobug-AMD** (186 files). No commit is shared, so git treats
every change as foreign; there is no merge base to merge along.

The code bases are closely related in places and far apart in others:

| File | Theirs | Ours |
|---|---:|---:|
| `crates/victory-gpu/src/host.rs` | 11,069 | 4,494 |
| `crates/victory-gpu/src/draw.rs` | 4,629 | 2,187 |
| `crates/victory-gpu/src/watch.rs` | 4,670 | 1,197 |
| `crates/victory-gpu/src/descriptor/host.rs` | 5,970 | 3,979 |
| `crates/victory-gpu/src/shadow.rs` | 5,887 | 2,786 |
| `crates/victory-shader/src/emit/lower.rs` | 14,041 | 15,049 |

They also carry whole modules we do not have (`port.rs`, `port_descriptors.rs`,
`inplace.rs`, `land.rs`, `list/thin.rs`, the `host/` split), and they have no
Steam Controller module, no Win10 Agility work and none of our HLE fixes.

## What was tested

Every one of their **522 non-merge commits** was applied alone onto our
`dev-7900xtx` with `git cherry-pick --no-commit`, then rolled back:

- **133 applied without conflict** (25%).
- **389 conflicted** (75%).

Conflict hotspots (number of commits touching each file):

```
100 crates/victory-gpu/src/host.rs
 61 crates/victory-shader/src/emit/lower.rs
 60 crates/victory-gpu/src/draw.rs
 57 crates/victory-gpu/src/descriptor/host.rs
 56 crates/victory-gpu/src/shadow.rs
 38 crates/victory-gpu/src/watch.rs
 31 crates/victory-shader/src/emit/mod.rs
```

## Applied on `dev-7900xtx` (18 commits)

Curated from the conflict-free set: shader correctness/perf, HLE/loader
robustness, GPU API edges. Authors preserved by `cherry-pick -x`.

```
51d9354 Morpheus0x      Port fixup: the ported v_perm test fills every Parts field
de415b6 Fuzeofficial    Shaders: test s_bfe_u32 and s_bfe_i32 literal, width, SCC
446b6cc BastienRdz      GPU: fences answer for ID3D12Fence1 as documented
b526090 Tex            Shaders: precompute MD5 round constants, no per-block Vec
ade4881 Tex            Shaders: fold empty loop latches containing only dead phis
0903ae0 EatCodeSleep2   Shaders: fold conditional branches to their fallthrough
61d83a5 给伊莉雅补魔     GPU: validate null args in create_deserializer
750a39e David Palacios  CLI: AVX2 pre-flight CPU check before executing titles
f6d6fcc Nixitov         HLE: implement the RPC runtime's GUID calls
3824452 KoLolX3         GPU: VICTORY_* switches set to 0 are off
3601550 Devin           Shaders: reuse lane guards for unchanged EXEC in a block
bbade48 marquezini      Kernel: GetModuleFileName into a null buffer
1dd2bda Duff            HLE: load a host DLL outside the module cache's lock
1a720d3 Manuel Pérez    Loader: suspend a thread before reading its TEB
f9aa7e0 warmooff-ops    hle/gameinput: reading history 32 -> 256 for low FPS
aa99032 Pedro Fortunato fix(shader): lower dynamic v_perm_b32 without INT64
dbf8611 julio           GPU: give null texture UAVs the title's view dimension
0032c23 Duff            Files: send rewritten narrow paths to the host
```

Status: `cargo test -p victory-shader` **203 passed**, `cargo check
--target x86_64-pc-windows-msvc -p victory` clean.

## What could not be taken directly

Their performance work is mostly **architecture**, not patches:

- **Renderer port 1** (`VICTORY_PORT_CONSTANTS`): the title's 32-bit graphics
  heap used in place, no uploads/fills. Needs their `inplace.rs` and host
  copy model; ours has no in-place heap.
- **Renderer port 2** (`VICTORY_PORT_DESCRIPTORS`): patches the title's
  descriptor record stores (byte-checked) so slots are reported instead of
  scanning ~600k a frame. Needs their descriptor/watch layout and loader
  write-site hooks.
- **Thin lists / preparing thread** (`VICTORY_THIN`, submission split): list
  calls recorded and translated on several threads, submissions prepared on a
  thread of its own.

The AMD-specific shader work is high value for your card but every one of those
commits conflicts today: tiled cascade shadows on wave32 (`4fd34fdd`), wave64
builder thread counts (`06fe22a7`), LDS flag regions (`ad0f9dda`), BVH fences
(`30f46c92`), row permutations on wave32 (`4d3aaab7`). They need manual conflict
resolution against our `lower.rs`/`emit` (we already carry Belobug #75/#88/#186
and the clean-up passes).

Their own notes put a 55 ms water frame at: preparing thread 48 ms, card 41 ms,
36k list calls (~230 ms of work over 8 threads), 400k descriptor slots scanned.
That is the same wall our `frame.log` shows (preparing 40-385 ms, 20-95
submissions, 100-190 MB of copies).

## Suggested plan

1. **Test `dev-7900xtx` now** as a low-risk compatibility bump (bug fixes,
   shader folding, input at low fps). It does not contain the big renderer
   patches.
2. **Port the wave32 shader fixes one at a time** onto our `dev`: resolve
   conflicts in `emit/lower.rs` per commit, gate each with `cargo test -p
   victory-shader` and the AMD hardware that failed before (cascade shadows,
   LDS flag region, BVH). This is the most tractable big win for an RDNA card.
3. **Renderer port 2 first** (descriptor stores) if we reimplement one of their
   architectural changes: our `watch.rs` already knows write sites, and the
   win is the largest single item in their table (280 -> 124 ms).
4. **Renderer port 1** (constants heap in place) second; it needs an in-place
   heap concept in our host layer, so it is a larger design change.
5. Do **not** merge their tree wholesale: we would lose Steam input, Win10
   Agility, the boat/read-back default, the crash symbolization and our HLE
   fixes, and their target is 24 GB VRAM / 64 GB RAM where we run 16 GB.

Every step is A/B-able with the existing `frame.log` (usual frame, preparing,
copies MB, uploads, card lists).
