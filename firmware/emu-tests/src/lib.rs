//! Host harness for the `rp2350-emu` firmware mock tests.
//!
//! This is the first tier of the mock-test chain required by docs/plan/13 §1.6:
//! the firmware runs as a real Cortex-M33 XIP flash image inside `rp2350-emu`,
//! with a scripted phone transport and mock OLED/encoder.
//!
//! Flow:
//!
//! 1. `firmware/rp2354-emu-fw` is built for `thumbv8m.main-none-eabihf` as an
//!    RP2350 flash image (vector table + picobin IMAGE_DEF are emitted for
//!    future real-bootrom / TUI use; see its `link.x` and `src/main.rs`);
//! 2. this harness loads the flash image at `0x1000_0000`, points the core at
//!    the firmware entry with a **synthetic bootrom** (ROM word 0 = SP,
//!    word 1 = PC) and runs it;
//! 3. the harness pokes a scenario id into the SRAM mailbox and polls it until
//!    the firmware reports `magic == "AURB"`.
//!
//! Why a synthetic bootrom instead of the real RP2350 bootrom? `rp2350-emu`
//! 0.2.6's own flash-boot test is `#[ignore]`d upstream: the emulated bootrom's
//! RCP seed/canary state does not yet match silicon (`___step4_init_rcp_seeds`
//! mismatch). The real bootrom therefore NMI-loops before reaching firmware.
//! The vendored bootrom under `firmware/rp2354-emu-fw/rom/` is kept for
//! `rp2350-emu-tui` once that upstream issue is fixed; until then the synthetic
//! ROM boot exercises the same firmware image.
//!
//! On a failed check id, inspect the same bootrom/flash pair with
//! `rp2350-emu-tui` (see docs/plan/13 §1.6.2).

use std::fs;
use std::path::Path;

use object::{Object, ObjectSection, SectionFlags};
use rp2350_emu::{Config, Emulator};

/// SRAM mailbox base, shared with `firmware/rp2354-emu-fw/src/main.rs`.
pub const MAILBOX_BASE: u32 = 0x2007_0000;

/// RP2350 XIP flash window.
const FLASH_BASE: u32 = 0x1000_0000;
const FLASH_LIMIT: u32 = 0x1020_0000;

/// RP2350 SRAM window.
const SRAM_BASE: u32 = 0x2000_0000;
const SRAM_LIMIT: u32 = 0x2008_2000;

/// Initial stack pointer used by the synthetic bootrom (also vector[0]).
const STACK_TOP: u32 = 0x2008_0000;

/// Mailbox field offsets (words).
const WORD_SCENARIO: u32 = 0;
const WORD_STATUS: u32 = 4;
const WORD_CHECK: u32 = 8;
const WORD_DETAIL: u32 = 12;
const WORD_HASH: u32 = 16;
const WORD_MAGIC: u32 = 20;
const WORD_DEBUG0: u32 = 24;
const WORD_DEBUG1: u32 = 28;
const WORD_DEBUG2: u32 = 32;
const WORD_DEBUG3: u32 = 36;

/// Magic written last by the firmware ("AURB").
const MAILBOX_MAGIC: u32 = 0x4155_5242;

/// Firmware's expected status values.
pub const STATUS_PASSED: u32 = 1;

/// Scenario ids, shared with `firmware/rp2354-emu-fw`.
pub const SCENARIO_ALLOC: u32 = 0;
pub const SCENARIO_READ_UIMLK: u32 = 1;
pub const SCENARIO_CONFIRM_SCREEN: u32 = 2;
pub const SCENARIO_NO_DEVICE: u32 = 3;

/// Max master cycles to wait for one scenario (bootrom + firmware).
const CYCLE_LIMIT: u64 = 200_000_000;
const CYCLE_CHUNK: u64 = 500_000;

/// Mailbox contents after a scenario run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmuOutcome {
    pub status: u32,
    pub check: u32,
    pub detail: u32,
    pub hash: u32,
    pub debug0: u32,
    pub debug1: u32,
    pub debug2: u32,
    pub debug3: u32,
    pub cycles: u64,
}

impl EmuOutcome {
    pub fn passed(&self) -> bool {
        self.status == STATUS_PASSED
    }
}

/// Default vendored bootrom path (repo-relative, resolves at compile time so the
/// harness works from any current directory).
pub fn default_bootrom_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../rp2354-emu-fw/rom/bootrom-combined.bin")
}

/// Load the firmware flash image into a fresh emulator and point the core at the
/// firmware entry with a synthetic bootrom.
///
/// Returns the emulator (reset) and the ELF entry point.
pub fn boot_firmware(elf_path: &Path) -> (Emulator, u32) {
    let elf = fs::read(elf_path).unwrap_or_else(|err| {
        panic!(
            "cannot read firmware ELF {}: {err} (run scripts/emu-test.ps1 or set \
             AUROKU_EMU_FW_ELF)",
            elf_path.display()
        )
    });
    let object = object::File::parse(&*elf)
        .unwrap_or_else(|err| panic!("cannot parse firmware ELF {}: {err}", elf_path.display()));

    let entry = u32::try_from(object.entry())
        .unwrap_or_else(|_| panic!("firmware entry point does not fit in u32"));
    if entry == 0 {
        panic!("firmware ELF has a zero entry point (check link.x / ENTRY(_start))");
    }

    let mut emulator = Emulator::new(Config::default());

    // Synthetic bootrom: word 0 = SP (top of SRAM), word 1 = PC (firmware
    // entry). The core still executes the real XIP flash image below, so the
    // firmware build/layout is the production one; only the Raspberry Pi
    // bootrom stage is replaced (see module docs).
    let mut bootrom = vec![0u8; 32 * 1024];
    bootrom[0..4].copy_from_slice(&STACK_TOP.to_le_bytes());
    bootrom[4..8].copy_from_slice(&(entry | 1).to_le_bytes());
    emulator.load_bootrom(&bootrom);

    let (flash, ram_sections) = build_memory_images(&object, elf_path);
    emulator.load_flash(&flash);
    for (address, data) in ram_sections {
        emulator.load_image(address, &data);
    }

    emulator.reset();
    (emulator, entry)
}

/// Split the ELF into a flat XIP flash image and initialised SRAM sections.
///
/// The firmware is linked so that everything the bootrom needs lives in flash
/// (vectors, picobin block, code+rodata), while `.bss` is NOBITS and SRAM starts
/// zeroed in the emulator. Any initialised `.data` sections are loaded directly
/// into SRAM (the firmware has no startup copy loop; the test firmware keeps
/// mutable initialised data out of the image).
fn build_memory_images(
    object: &object::File<'_>,
    elf_path: &Path,
) -> (Vec<u8>, Vec<(u32, Vec<u8>)>) {
    let mut flash_sections: Vec<(u32, Vec<u8>)> = Vec::new();
    let mut ram_sections: Vec<(u32, Vec<u8>)> = Vec::new();

    for section in object.sections() {
        let data = match section.data() {
            Ok(data) => data,
            Err(_) => continue,
        };
        if data.is_empty() {
            continue;
        }
        let is_alloc = match section.flags() {
            SectionFlags::Elf { sh_flags } => sh_flags & 0x2 != 0,
            _ => false,
        };
        if !is_alloc {
            continue;
        }
        let address = section.address() as u32;
        if (FLASH_BASE..FLASH_LIMIT).contains(&address) {
            flash_sections.push((address, data.to_vec()));
        } else if (SRAM_BASE..SRAM_LIMIT).contains(&address) {
            ram_sections.push((address, data.to_vec()));
        } else {
            panic!(
                "section {} of {} has address 0x{address:08x} outside the RP2350 \
                 flash/SRAM windows",
                section.name().unwrap_or("<unnamed>"),
                elf_path.display()
            );
        }
    }

    let flash_len = flash_sections
        .iter()
        .map(|(address, data)| (address - FLASH_BASE) as usize + data.len())
        .max()
        .unwrap_or(0);
    if flash_len == 0 {
        panic!(
            "no flash sections found in {}; the firmware must be linked at 0x1000_0000",
            elf_path.display()
        );
    }

    let mut flash = vec![0u8; flash_len];
    for (address, data) in &flash_sections {
        let offset = (address - FLASH_BASE) as usize;
        flash[offset..offset + data.len()].copy_from_slice(data);
    }

    (flash, ram_sections)
}

pub fn run_scenario(elf_path: &Path, scenario: u32) -> EmuOutcome {
    let (mut emulator, _entry) = boot_firmware(elf_path);
    emulator.poke(MAILBOX_BASE + WORD_SCENARIO, scenario);

    let mut cycles = 0u64;
    while cycles < CYCLE_LIMIT {
        emulator
            .run(CYCLE_CHUNK)
            .expect("serial emulator run must not fail");
        cycles += CYCLE_CHUNK;

        let status = emulator.peek(MAILBOX_BASE + WORD_STATUS);
        let magic = emulator.peek(MAILBOX_BASE + WORD_MAGIC);
        if status != 0 && magic == MAILBOX_MAGIC {
            return EmuOutcome {
                status,
                check: emulator.peek(MAILBOX_BASE + WORD_CHECK),
                detail: emulator.peek(MAILBOX_BASE + WORD_DETAIL),
                hash: emulator.peek(MAILBOX_BASE + WORD_HASH),
                debug0: emulator.peek(MAILBOX_BASE + WORD_DEBUG0),
                debug1: emulator.peek(MAILBOX_BASE + WORD_DEBUG1),
                debug2: emulator.peek(MAILBOX_BASE + WORD_DEBUG2),
                debug3: emulator.peek(MAILBOX_BASE + WORD_DEBUG3),
                cycles,
            };
        }
    }

    panic!(
        "firmware did not finish scenario {scenario} within {CYCLE_LIMIT} cycles \
         (status=0x{:x} magic=0x{:x}); inspect with rp2350-emu-tui using bootrom {} \
         and flash image {}",
        emulator.peek(MAILBOX_BASE + WORD_STATUS),
        emulator.peek(MAILBOX_BASE + WORD_MAGIC),
        default_bootrom_path().display(),
        elf_path.display()
    );
}

/// Resolve the firmware ELF from `AUROKU_EMU_FW_ELF`, or `None` for a skipped
/// local run. CI sets `AUROKU_REQUIRE_EMU=1` so a missing path is fatal instead
/// of silently skipping the emulator mock tests.
pub fn firmware_elf_from_env() -> Option<std::path::PathBuf> {
    match std::env::var_os("AUROKU_EMU_FW_ELF") {
        Some(path) => Some(std::path::PathBuf::from(path)),
        None => {
            if std::env::var_os("AUROKU_REQUIRE_EMU").is_some() {
                panic!(
                    "AUROKU_REQUIRE_EMU is set but AUROKU_EMU_FW_ELF is missing; \
                     build firmware/rp2354-emu-fw first (scripts/emu-test.ps1)"
                );
            }
            eprintln!(
                "SKIP: AUROKU_EMU_FW_ELF is not set; build firmware/rp2354-emu-fw and \
                 run scripts/emu-test.ps1 to execute the rp2350-emu mock tests"
            );
            None
        }
    }
}
