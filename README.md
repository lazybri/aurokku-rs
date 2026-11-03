# libwdi-static

Standalone, orphan branch that owns everything needed to build a **static
libwdi** (x64 only, WinUSB payload only, static CRT /MT) and to link it from
Rust: the build pipeline, the verified artifact, the upstream pin, the patches
and the `libwdi-sys` bindings crate.

It shares **no history** with the product branches (`main`, `qt6`). Never merge
it into them and never merge them into it: product repositories only consume
the artifact and the crate by immutable tag (see "Stable contracts" below).

Why a separate branch: the old pipeline lived inline in the product repository's
`.github/workflows/build.yml`, downloaded ~31 MiB of WDK, cloned a mutable
upstream tag and patched sources with `-replace`. It could not be reviewed,
shared with a second product branch, or audited. Moving it here is
docs/plan/03-libwdi-decoupling.md (P1a/P1b).

## Layout

```text
Cargo.toml                     pure virtual workspace (no [package]) -- see remark
crates/libwdi-sys/             bindings + build script (links = "libwdi")
tools/build-libwdi.ps1         WDK -> pinned libwdi -> patches -> msbuild -> lib/
tools/verify-libwdi.ps1        artifact contract verification (CI gate)
patches/0001-config-*.patch    config.h: x64 only, WinUSB payload only
patches/0002-drop-*.patch      libwdi_static.vcxproj: drop MSB8013 project refs
upstream/libwdi.ref            full 40-hex upstream commit SHA (v1.5.1)
LICENSES/LGPL-3.0.txt          pinned copy of upstream COPYING-LGPL
.github/workflows/libwdi-static.yml
docs/BUILDING.md               how the pipeline works, how to upgrade/retry
```

`Cargo.toml` is deliberately a *pure* virtual workspace (only `[workspace]`,
`members = ["crates/libwdi-sys"]`, no `[package]`): a git dependency is resolved
by reading the repository root manifest and then locating the requested
package. With a root `[package]` of a different name, Cargo stops at the root
and fails with `no matching package named 'libwdi-sys' found`.

## Stable contracts

These are the only things products may rely on. Anything else is an internal
detail of this branch.

### Contract 1 - environment: `LIBWDI_LIB_DIR`

`LIBWDI_LIB_DIR` must point at the directory that contains `libwdi.lib`
(the artifact's `lib/` directory). `crates/libwdi-sys/build.rs` turns it into
`-L native=<dir>` + `-l static=libwdi` + the 19 system import libraries
(setupapi, ole32, ntdll, advapi32, user32, gdi32, comctl32, crypt32, wintrust,
version, shlwapi, cfgmgr32, msi, shell32, oleaut32, uuid, comdlg32, winspool,
userenv).

Semantics: unset -> warn only (optional dependency, `cargo check`/metadata must
keep working); set but without `libwdi.lib` -> **panic** (an explicitly
requested static libwdi must never degrade silently into the external
`wdi-simple.exe` fallback).

### Contract 2 - artifact: `libwdi-static`

`.github/workflows/libwdi-static.yml` uploads an artifact named
`libwdi-static`. Extracted, it is exactly:

```text
lib/libwdi.lib
lib/COPYING-LGPL
lib/BUILD-INFO.json
lib/SHA256SUMS
```

`SHA256SUMS` lists every other file in `lib/` as `<sha256>  <name>`.
`BUILD-INFO.json` records the upstream commit, the WDK MSI hash, the patch
hashes, the MSVC toolset version, the build time, and `lib_sha256`/`lib_size`.
`tools/verify-libwdi.ps1` is the gate: SHA256 consistency, `dumpbin /symbols`
for the six `wdi_*` entry points, the `.drectve` `LIBCMT` (static CRT)
heuristic, LGPLv3 text, and BUILD-INFO self-consistency.

### Contract 3 - Cargo: `libwdi-sys` at an immutable tag

```toml
[target.'cfg(windows)'.dependencies]
libwdi-sys = { git = "https://github.com/lazybri/aurokku-rs",
               tag = "libwdi-1.5.1-x64-r1", optional = true }

[features]
static-libwdi = ["dep:libwdi-sys"]
```

Package name `libwdi-sys`, library name `libwdi_sys`, `links = "libwdi"`,
`publish = false`. The product feature name `static-libwdi` stays stable across
releases; the tag suffix `r<N>` is the pipeline revision, and tags are
**immutable** (supply-chain anchor). Publish a new tag instead of moving an old
one. `Cargo.lock` in the product repository records the resolved
`git+https://github.com/lazybri/aurokku-rs?tag=...#<commit>`.

## Consuming the artifact from a product workflow

```yaml
jobs:
  libwdi:
    uses: lazybri/aurokku-rs/.github/workflows/libwdi-static.yml@libwdi-1.5.1-x64-r1
    with:
      libwdi-sha: 9b23b82a2dd1cbffc16d46c212f92c6bf8c0c602
      libwdi-ref: libwdi-1.5.1-x64-r1   # optional, this is the default

  build:
    needs: [test, libwdi]
    steps:
      # github.sha inside the called workflow is the CALLER commit, which is
      # why the called workflow checks out libwdi-ref instead.
      - uses: actions/download-artifact@v4
        with:
          name: libwdi-static
          path: libwdi-out
      - shell: pwsh
        run: |
          $env:LIBWDI_LIB_DIR = "$PWD/libwdi-out/lib"
          ./build-windows.ps1 -Target x86_64-pc-windows-msvc
```

The artifact is only valid for the run that produced it
(`actions/download-artifact@v4` does not download across runs), so the product
job must `needs:` the libwdi job.

## Release / tag policy

1. Land changes on the `libwdi-static` branch; the branch push self-tests the
   pipeline (build + verify + crate link smoke) but publishes nothing.
2. When the branch is green, create an **annotated** tag pointing at the final
   commit that contains the pipeline *and* the crate:
   `git tag -a libwdi-1.5.1-x64-r1 -m "libwdi 1.5.1 x64 static pipeline r1"`.
3. Push the tag. The tag run repeats build + verify + crate smoke and publishes
   a GitHub Release with `BUILD-INFO.json` and `SHA256SUMS` as assets.
4. Need changes after a tag? Do **not** move it: publish
   `libwdi-1.5.1-x64-r2` and update the products' `uses:` ref and Cargo tag.

## Building locally

Only Windows with Visual Studio Build Tools (MSBuild + MSVC v143) and network
access to Microsoft/GitHub can run the pipeline; see
[docs/BUILDING.md](docs/BUILDING.md). Product developers do not need it: the
product repositories download the artifact from CI and never build libwdi.

```powershell
./tools/build-libwdi.ps1        # -> libwdi-out/lib/{libwdi.lib,COPYING-LGPL,BUILD-INFO.json,SHA256SUMS}
./tools/verify-libwdi.ps1 -LibDir ./libwdi-out/lib
```

## Provenance and LGPL

- Upstream: https://github.com/pbatard/libwdi, pinned to
  `9b23b82a2dd1cbffc16d46c212f92c6bf8c0c602` (v1.5.1), full commit SHA, never a
  tag. `tools/build-libwdi.ps1` asserts the checked-out commit.
- Build inputs that change upstream files are real patches in `patches/` and
  are listed with hashes in `BUILD-INFO.json`.
- libwdi is LGPLv3. The artifact always ships `COPYING-LGPL` (the build fails
  instead of silently omitting it) and this branch is public, so recipients can
  rebuild and relink libwdi from the pinned source + patches.
