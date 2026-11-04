**English** | [简体中文](README.zh-CN.md)

# auRokku · RP2354 firmware branch (`rp2354`)

This branch turns auRokku into a **handheld RP2354A firmware** project. It is a
research/firmware branch: the desktop application (CLI + egui/Qt GUI, Windows
driver stack, release pipeline) has been removed from this branch by user
direction and continues to live on `main` / `qt6`. The protocol code was ported
from auRokku v0.3.2 (`main` @ `ee37a79`).

The branch is **mock-first**: everything below the USB host stack and the OLED
panel is a normal Rust library that runs under `cargo test` on the host, using a
scripted phone instead of hardware. This lets the firmware logic be developed
and reviewed before the custom PCB (docs/plan/06 §7, G1 gate) exists.

## Layout

```text
crates/auroku-core/          # no_std + alloc protocol core (shared with desktop)
  src/codec.rs               #   nibble/lockcode/hex codecs
  src/type2auth.rs           #   MD5 Type2Auth 0x52/0x53
  src/frame.rs               #   frame buffering, deadlines, 0xAF filter
  src/session.rs             #   base/UIMLK session + authentication
  src/ops.rs                 #   all 12 tasks with typed reports
  src/mock.rs                #   scripted request/response UsbTransport
firmware/auroku-firmware/    # RP2354 firmware logic (host-testable)
  src/hal.rs                 #   Display / Input / Clock traits
  src/ui.rs                  #   OLED menu state machine (docs/plan/09)
  src/protocol.rs            #   auroku-core session wrapper
  src/app.rs                 #   input -> UI -> protocol -> display loop
  src/mock.rs                #   MockDisplay / MockInput / MockClock
spike/auroku-core-nostd/     # original no_std feasibility spike (docs/plan/10)
docs/plan/                   # RP2354 research + design documents (06-10, 13)
```

## Status

| Area | State |
|---|---|
| Protocol core (`crates/auroku-core`) | ported from desktop v0.3.2; all 12 tasks; mock E2E tests |
| Firmware UI (`ui.rs`) | working skeleton (flat menu, confirm, result); full pages in progress |
| Firmware app loop (`app.rs`) | working, fully mock-driven |
| RP2354 USB host stack (cotton/TinyUSB) | **not started** - hardware G1 gate first (docs/plan/06 §7.2) |
| OLED / EC11 drivers, PCB | not started |

## Build & test

```bash
# Host tests (no hardware needed: mocks simulate the display, encoder and phone)
cargo test --workspace

# Cortex-M33 firmware build
rustup target add thumbv8m.main-none-eabihf
cargo build -p auroku-core    --target thumbv8m.main-none-eabihf --release
cargo build -p auroku-firmware --target thumbv8m.main-none-eabihf --release
```

## Documents

- [06 feasibility](docs/plan/06-rp2354-port-feasibility.md) - go/no-go, costs, risks
- [07 platform/toolchain](docs/plan/07-rp2354-platform-and-toolchain.md) - RP2354, USB host, board
- [08 core portability](docs/plan/08-core-portability.md) - what was ported and how
- [09 OLED/buttons UI](docs/plan/09-oled-buttons-ui.md) - screen, menu, interaction
- [10 no_std spike](docs/plan/10-core-nostd-spike.md) - verification evidence
- [13 session rules](docs/plan/13-session-rules.md) - branch working rules

## License

MIT. Original Python implementation by Yuu; see `LICENSE`.
