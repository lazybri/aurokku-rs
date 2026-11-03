# Building the static libwdi pipeline

This document describes the pipeline in this orphan branch. It is the
"reproduce / upgrade / debug" manual for **maintainers**; product developers
never run it (they download the CI artifact, see `../README.md`).

## What the pipeline does

`tools/build-libwdi.ps1` performs the same steps as the old inline PowerShell
step in the product repository (`.github/workflows/build.yml` lines 111-157 on
main @ b219bd5), with the known gaps fixed:

1. **WDK redistributable.** Download
   `https://go.microsoft.com/fwlink/p/?LinkID=253170` (the WinUSB WDK 8.0
   redistributable, ~31 MiB) into `.cache/wdk-redist.msi` (the CI caches that
   file), verify its SHA256, then extract it with
   `msiexec /a ... /qn TARGETDIR=<work>\wdk`. Administrative install only: no
   admin rights and no system changes. Expected layout:
   `<work>\wdk\Windows Kits\8.0`.
2. **Pinned upstream.** Fetch the full 40-hex commit SHA from
   `upstream/libwdi.ref` (never a tag), check it out, and assert
   `git rev-parse HEAD`. The reusable workflow passes the same SHA as
   `libwdi-sha`; a mismatch aborts the build.
3. **Patches.** Normalise `msvc/config.h` and
   `libwdi/.msvc/libwdi_static.vcxproj` to LF (the vcxproj is checked out with
   CRLF because of upstream `.gitattributes`), then apply every
   `patches/*.patch` with `git apply --check` first. Afterwards the script
   asserts the patched content (OPT_M32/OPT_ARM/LIBUSB0_DIR/LIBUSBK_DIR off,
   zero `ProjectReference` entries, vcxproj still valid XML).
4. **msbuild**, same arguments as the old step: `/m`,
   `/p:Configuration=Release`, `/p:SolutionDir=<clone>\` and
   `/p:BuildMacros=WDK_DIR="<work>/wdk/Windows Kits/8.0"` (forward slashes:
   MSVC would treat `\a`, `\w`, `\W` in the macro as escape sequences).
   Targets, in order:
   - `libwdi\.msvc\detect_64build.vcxproj /p:Platform=x64` (generates
     `libwdi\build64.h` for the embedder),
   - `libwdi\.msvc\installer_x64.vcxproj /p:Platform=x64`,
   - `libwdi\.msvc\embedder.vcxproj /p:Platform=Win32` (only Win32 exists),
   - `libwdi\.msvc\libwdi_static.vcxproj /p:Platform=x64`.
5. **Stage the artifact.** Copy `libwdi\x64\Release\lib\libwdi.lib` and
   `COPYING-LGPL` into `<OutDir>\lib\`, assert the license matches
   `LICENSES/LGPL-3.0.txt`, then write `BUILD-INFO.json` and `SHA256SUMS`.
6. **Export.** Set `LIBWDI_LIB_DIR=<OutDir>\lib` for the workflow
   (`GITHUB_ENV`) and emit `lib_sha256`/`lib_dir` outputs.

## Prerequisites

- Windows (the upstream vcxproj files and msiexec are Windows-only).
- Visual Studio Build Tools with MSVC **v143** and `msbuild.exe` on `PATH`.
  In CI this comes from `microsoft/setup-msbuild@v2` on `windows-2022`.
- `git`, `curl.exe`, PowerShell 7 (`pwsh`), network access to GitHub and
  Microsoft.
- `dumpbin.exe` only for `tools/verify-libwdi.ps1` (found through `vswhere` or
  `PATH`; pass `-Dumpbin` explicitly if needed).

No WDK install is required (the redistributable is extracted, not installed).

## Running locally

```powershell
# Full build: WDK -> libwdi -> patches -> msbuild -> libwdi-out/lib/*
./tools/build-libwdi.ps1

# Verify the artifact contract (SHA256, symbols, /MT heuristic, LGPL, metadata)
./tools/verify-libwdi.ps1 -LibDir ./libwdi-out/lib

# Optional: cross-check a pinned hash that a product repository checks in
./tools/verify-libwdi.ps1 -LibDir ./libwdi-out/lib -ExpectedSha256 <sha256>
```

Scratch state lives in `.work/libwdi` (upstream clone + extracted WDK) and
`.cache/wdk-redist.msi`; both are git-ignored. Re-running is safe: the script
resets the clone to the pinned commit and re-applies the patches.

### Parameters

| Parameter | Default | Meaning |
| --- | --- | --- |
| `-LibwdiSha` | `upstream/libwdi.ref` | Upstream commit SHA; when given it must equal the pin |
| `-OutDir` | `<repo>/libwdi-out` | Artifact root; the contract files land in `lib/` |
| `-WorkDir` | `<repo>/.work/libwdi` | Upstream clone + WDK extraction |
| `-WdkMsiUrl` | Microsoft LinkID 253170 | WDK redistributable URL |
| `-WdkMsiSha256` | `29314207...a583d6` | Expected MSI SHA256 (see "Rotating the WDK MSI") |

## Artifact contract

```text
libwdi-out/lib/libwdi.lib        static library (x64, WinUSB payload, /MT)
libwdi-out/lib/COPYING-LGPL      LGPLv3 text, identical to LICENSES/LGPL-3.0.txt
libwdi-out/lib/BUILD-INFO.json   provenance record
libwdi-out/lib/SHA256SUMS        "<sha256>  <name>" for the other three files
```

`BUILD-INFO.json` fields: `schema` (1), `artifact` (`libwdi-static`),
`version` (`libwdi-1.5.1-x64-r<N>`, taken from the pushed tag when the workflow
runs on a tag), `upstream`, `upstream_tag`, `upstream_ref` (40-hex commit),
`platform` (`x64`), `payload`, `runtime`, `patches` (name + SHA256 per patch),
`wdk_link_id`, `wdk_msi_sha256`, `toolset_version`, `runner_image`,
`runner_image_version`, `built_at_utc`, `lib_sha256`, `lib_size`, `license`.

The GitHub Actions artifact is named `libwdi-static` and contains the `lib/`
tree; `actions/upload-artifact@v4` strips the `libwdi-out` prefix.

## How the patches were produced

The two patches are a mechanical port of the old inline `-replace` calls, so
they are generated the same way (never hand-edited):

```powershell
git clone https://github.com/pbatard/libwdi.git scratch-libwdi
git -C scratch-libwdi checkout 9b23b82a2dd1cbffc16d46c212f92c6bf8c0c602
# normalise msvc/config.h and libwdi/.msvc/libwdi_static.vcxproj to LF first
# (keeping the vcxproj UTF-8 BOM), then apply exactly the old replacements:
#   "#define OPT_M32"        -> "/* #define OPT_M32 */"
#   "#define OPT_ARM"        -> "/* #define OPT_ARM */"
#   "(?m)^#define LIBUSB0_DIR.*$" -> "/* #define LIBUSB0_DIR (not embedded) */"
#   "(?m)^#define LIBUSBK_DIR.*$" -> "/* #define LIBUSBK_DIR (not embedded) */"
#   "(?s)\s*<ProjectReference Include=\"[^\"]+\">.*?</ProjectReference>" -> ""
git -C scratch-libwdi diff -- msvc/config.h > patches/0001-config-x64-winusb-only.patch
git -C scratch-libwdi diff -- libwdi/.msvc/libwdi_static.vcxproj > patches/0002-drop-project-references.patch
```

Why the project references are dropped: on `Release|x64` MSBuild would try to
resolve the referenced projects (`embedder`/`installer_x86` are Win32-only) and
fail with **MSB8013**. The pipeline builds them explicitly before
`libwdi_static`, so the references are unnecessary.

Patches are stored with `-text` in `.gitattributes` so git never rewrites their
line endings; `tools/build-libwdi.ps1` also normalises the target files to LF
before `git apply`.

## Upgrading libwdi (publishing r<N+1>)

Treat every step as a reviewed change; the old tag must stay untouched.

1. Resolve the new upstream commit: `git ls-remote --tags https://github.com/pbatard/libwdi.git`
   (for an annotated tag also pass `refs/tags/<tag>^{}`), then update
   `upstream/libwdi.ref`.
2. Re-generate `patches/*.patch` against the new commit following the recipe
   above; fix them if upstream changed the surrounding lines.
3. Update `LICENSES/LGPL-3.0.txt` if (and only if) upstream changed
   `COPYING-LGPL`; the build fails when the texts differ.
4. Update `upstream_tag`/`version`-related text (`README.md`, this document,
   workflow comments) and the tag suffix, e.g. `libwdi-1.6.0-x64-r2`.
5. Push the branch; the self-test must be green.
6. Tag the final commit and push the tag; then update the consumers (`uses:`
   ref + Cargo `tag` + the `libwdi-sha` input) in the product repositories.

## Rotating the WDK MSI

Microsoft's LinkID download can be re-pointed to a newer MSI at any time. When
the build fails with:

```text
WDK redist MSI sha256 mismatch: expected ..., got ...
```

verify that the new file is really the WDK 8.0 redistributable, then update
`-WdkMsiSha256` in `tools/build-libwdi.ps1` **and** the
`actions/cache` key in `.github/workflows/libwdi-static.yml` in the same
commit. Never delete the check to "make CI green".

## Troubleshooting

| Symptom | Cause / fix |
| --- | --- |
| `libwdi.lib was not produced` | An msbuild target failed; scroll up for MSB errors. The script now fails on non-zero exit codes instead of continuing. |
| **MSB8013** after patching | `0002` was not applied (patched file still contains `ProjectReference`). The script asserts this; regenerate the patch if upstream moved it. |
| **MSB8020** (toolset not found) | The runner image lacks MSVC v143; the workflow pins `windows-2022`. If a future upstream bump needs another toolset, add a dedicated patch instead of editing sources ad hoc. |
| `patch does not apply` | Upstream moved the file. Regenerate the patch against the new pinned SHA (do not hand-edit); the `--check` step is intentional so drift fails loudly. |
| `WDK extraction did not produce "Windows Kits/8.0"` | msiexec failed or TARGETDIR was not honoured; check the MSI hash and the log line above. |
| `dumpbin.exe not found` | Run on a machine with VS Build Tools, or pass `-Dumpbin`; in CI `microsoft/setup-msbuild@v2` provides it through `vswhere`. |
| `does not default-link LIBCMT` | `libwdi_static` was built with a dynamic CRT. The upstream Release configuration uses `MultiThreaded` (/MT); check that no patch changed it and that the Release|x64 target was used. |
| `libwdi.lib is suspiciously small` | The WinUSB payload was not embedded (installer/embedder steps skipped or config.h patch not applied). |
| `the upstream COPYING-LGPL differs from LICENSES/LGPL-3.0.txt` | Upstream changed the license. Review the diff, update the pinned copy and the product documentation in a dedicated commit. |
| `libwdi-sys ... does not contain libwdi.lib` (cargo panic) | `LIBWDI_LIB_DIR` points at the wrong directory; it must be the artifact's `lib` directory. |

## CI behaviour

- `push` to `libwdi-static`: build + verify + crate link smoke on
  `windows-2022`; nothing is published.
- `push` to a `libwdi-*` tag: the same checks, then a Release job publishes
  `BUILD-INFO.json` and `SHA256SUMS` as assets (requires `contents: write` for
  that job only).
- `workflow_call` (product CI): checkout uses
  `inputs.libwdi-ref || github.sha` — under `workflow_call` `github.sha` is the
  **caller's** commit, so the default `libwdi-ref` (the release tag) is what
  provides `tools/` and `patches/`.
- The WDK MSI is cached with a key containing its SHA256, so a rotated download
  cannot be served from a stale cache.
