# auRokku Qt front-end

C++20 / Qt 6 Widgets front-end for the `auroku` Rust backend.

- Native Qt widgets, light and dark palettes.
- Simplified Chinese / English UI with runtime switching.
- Every USB call runs on a dedicated `QThread` (`RustBridge`); results arrive as
  queued signals, so the window never blocks on libusb.

## Layout

```
gui/
  CMakeLists.txt
  src/
    main.cpp                 entry point, CLI flags
    core/
      FfiApi.h               C ABI declarations (auroku-ffi)
      AppState.{h,cpp}       UI state, device form validation, request builder
      PageHost.h             interface between pages and the main window
      RustBridge.{h,cpp}     QObject facade over the worker thread
      RustWorker.{h,cpp}     blocking FFI calls, log trampoline
      JsonHelpers.{h,cpp}    Python-like JSON value rendering
    platform/UsbAccess.{h,cpp} temporary per-device Linux USB ACL management
    ui/
      Theme.{h,cpp}          light/dark Qt palettes + minimal QSS
      I18n.{h,cpp}           Chinese source strings → English dictionary
      Fonts.{h,cpp}          CJK-capable UI and monospace font discovery
      Icons.{h,cpp}          hand-painted 24×24 icon set
      Widgets.{h,cpp}        cards, banners, chips, metrics, switch, segments
      Page.{h,cpp}           page base class + shared result summary
      MainWindow.{h,cpp}     navigation rail, stacked pages, footer, toast
      ConfirmDialog.{h,cpp}
      pages/                 Overview, Uim, Pin, Probe, Batch, Session, Console
```

## Build

Requirements: Qt 6.5+, CMake 3.21+, Ninja, Rust toolchain (GNU or MSVC matching
the Qt build).

```bash
cmake -S gui -B build/gui -G Ninja -DCMAKE_BUILD_TYPE=Release \
      -DCMAKE_PREFIX_PATH=/path/to/Qt/6.11.2/gcc_64
cmake --build build/gui
```

CMake builds `crates/auroku-ffi` with Cargo and links the resulting static
library (`libauroku_ffi.a` / `auroku_ffi.lib`) directly into the GUI executable.
No separate FFI shared library is needed at build time or at runtime. Native
Wayland sessions also need the distribution package `qt6-wayland` (otherwise Qt
falls back to XWayland). Linux runtime ACL handling requires the `acl` package
(`getfacl` and `setfacl`).

### FFI contract

`crates/auroku-ffi` exports:

```c
char *auroku_ffi_version(void);
char *auroku_scan_devices(int vid, int pid);   /* < 0 = no filter */
char *auroku_run_task(const char *request_json, AurokuLogCallback cb, void *user);
void  auroku_string_free(char *text);
```

Every `char *` is a JSON envelope:

```json
{ "ok": true,  "data": ... }
{ "ok": false, "kind": "ValueError", "message": "..." }
```

`auroku_run_task` accepts:

```json
{
  "vid": 1154, "pid": 2652, "sn": "SKYKA077394", "interface": null,
  "ep_out": 4, "ep_in": 132, "verbose": false, "type2": false,
  "task": { "kind": "status" }
}
```

Task kinds: `status`, `readuimlk`, `clearlk`, `writelk` (`value`, `control`),
`readiccid`, `readpin`, `resetpin`, `maintenance_bit` (`control`),
`batch` (`profile`), `probe` (`probe`, `timeout_secs`), `exituimlk`, `exit`.

## Developer flags

```
--lang=en|zh           initial interface language
--dark                 start in the dark theme (default)
--light                start in the light theme
--smoke-test           start, then quit (CI smoke test)
--page=N               open page N (0 overview … 6 console)
--screenshot=FILE      save a window screenshot and quit
```

With `QT_QPA_PLATFORM=offscreen` the last three allow headless UI checks.

On Linux, install `qt6-wayland` for the native Wayland platform plugin. Without
it, Qt can fall back to XWayland; set `QT_QPA_PLATFORM=xcb` to select XWayland
explicitly and avoid a failed Wayland-plugin lookup. The GUI process runs
unprivileged. Scan for USB devices and explicitly select one to request polkit
authorization for a narrowly scoped `setfacl` command, granting the current
user temporary read/write ACL access to that device node. Selecting another
device or rescanning restores the previous ACL; closing the app also removes the
temporary rule. `acl` and a polkit agent are required. USB operations are pinned
to the selected bus/address so devices with identical VID/PID values are not
confused. A forced process termination or system crash can prevent cleanup and
leave a temporary ACL for an administrator to remove manually. The GUI waits
up to two minutes for the desktop polkit authorization prompt.
