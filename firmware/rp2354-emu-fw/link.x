/*
 * Linker script for the RP2350 emulator test firmware.
 *
 * This is the *supported* emulator boot layout, matching the RP2350 bootrom's
 * flash-image requirements (see picoem `roms/rp2350/gen_blinky.py` and RP2350
 * datasheet §5.9.5):
 *
 *   0x1000_0000  vector table (initial SP, reset, fault handlers)
 *   0x1000_0040  picobin IMAGE_DEF block (20 bytes, must be in the first 4 KB)
 *   0x1000_0060  .text + .rodata (reset handler and firmware code)
 *   0x2000_0000  .bss / heap (SRAM)
 *
 * The emulator boots the real Raspberry Pi RP2350 bootrom (vendored under
 * `rom/`), which validates the IMAGE_DEF block, loads the vector table and
 * jumps to the reset handler. Entry:  _start  (defined in src/main.rs).
 */
ENTRY(_start)

MEMORY
{
  FLASH (rx)  : ORIGIN = 0x10000000, LENGTH = 2M
  RAM   (rwx) : ORIGIN = 0x20000000, LENGTH = 0x80000
}

SECTIONS
{
  .vectors ORIGIN(FLASH) : {
    KEEP(*(.vectors))
  } > FLASH

  .picobin 0x10000040 : {
    KEEP(*(.picobin))
  } > FLASH

  .text 0x10000060 : {
    KEEP(*(.text*))
    *(.rodata*)
  } > FLASH

  /* Initialised data (normally empty): loaded into SRAM by the host harness. */
  .data : {
    *(.data*)
  } > RAM

  .bss (NOLOAD) : {
    *(.bss*)
    *(COMMON)
  } > RAM

  /DISCARD/ : {
    *(.ARM.exidx*)
    *(.ARM.extab*)
    *(.comment)
  }
}
