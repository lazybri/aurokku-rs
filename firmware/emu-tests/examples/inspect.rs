//! Debug helper: boot the emulator firmware and dump core/mailbox state.
//!
//! Usage:
//!   cargo run -p auroku-emu-tests --example inspect -- <firmware-elf> [scenario]
//!
//! This complements `rp2350-emu-tui` (docs/plan/13 §1.6.2): it is a quick way to
//! see whether the firmware is executing, faulted or stuck in a loop.

use std::path::Path;

use auroku_emu_tests::{boot_firmware, MAILBOX_BASE};
use object::{Object, ObjectSection, ObjectSymbol};

fn main() {
    let mut args = std::env::args().skip(1);
    let elf_path = args
        .next()
        .expect("usage: inspect <firmware-elf> [scenario]");
    let scenario: u32 = args
        .next()
        .map(|value| value.parse().expect("scenario must be an integer"))
        .unwrap_or(0);

    dump_layout(Path::new(&elf_path));

    let (mut emulator, entry) = boot_firmware(Path::new(&elf_path));
    println!("entry = 0x{entry:08x}");

    dump_entry_bytes(Path::new(&elf_path), entry);

    emulator.poke(MAILBOX_BASE, scenario);

    if std::env::var_os("AUROKU_TRACE").is_some() {
        let mut history: Vec<(u64, u32, u32, u32, [u32; 4])> = Vec::new();
        for step in 0..200_000u32 {
            let cycles = emulator.step().expect("step");
            let core = emulator.core(0);
            let regs = [
                core.regs.r[0],
                core.regs.r[1],
                core.regs.r[2],
                core.regs.r[3],
            ];
            history.push((cycles, core.regs.pc(), core.regs.lr(), core.regs.r[13], regs));
            if history.len() > 60_000 {
                history.remove(0);
            }
            if core.is_halted() {
                println!("halted after {step} steps; firmware PCs around the fault:");
                if let Some(index) = history
                    .iter()
                    .rposition(|(_, pc, _, _, _)| *pc >= 0x1000_0000)
                {
                    let start = index.saturating_sub(24);
                    for (cycles, pc, lr, sp, [r0, r1, r2, r3]) in &history[start..=index] {
                        println!(
                            "  cycles={cycles:>8} pc=0x{pc:08x} lr=0x{lr:08x} sp=0x{sp:08x} \
                             r0=0x{r0:08x} r1=0x{r1:08x} r2=0x{r2:08x} r3=0x{r3:08x}"
                        );
                    }
                } else {
                    println!("  (no SRAM PC in the retained history)");
                }
                return;
            }
            if step < 30 {
                println!(
                    "step {step:>3}: pc=0x{:08x} lr=0x{:08x} sp=0x{:08x}",
                    core.regs.pc(),
                    core.regs.lr(),
                    core.regs.r[13]
                );
            }
        }
        println!("no halt within 200k steps");
        return;
    }

    for chunk in 0..40u32 {
        let run = emulator.run(50_000);
        let core = emulator.core(0);
        println!(
            "chunk {chunk:02}: run={run:?} pc=0x{:08x} lr=0x{:08x} sp=0x{:08x} \
             xpsr=0x{:08x} halted={}",
            core.regs.pc(),
            core.regs.lr(),
            core.regs.r[13],
            core.regs.xpsr,
            core.is_halted(),
        );
        println!(
            "  mailbox: scenario={} status=0x{:x} check=0x{:x} detail=0x{:x} hash=0x{:x} magic=0x{:x} debug0=0x{:x} debug1=0x{:x} debug2=0x{:x} debug3=0x{:x}",
            emulator.peek(MAILBOX_BASE),
            emulator.peek(MAILBOX_BASE + 4),
            emulator.peek(MAILBOX_BASE + 8),
            emulator.peek(MAILBOX_BASE + 12),
            emulator.peek(MAILBOX_BASE + 16),
            emulator.peek(MAILBOX_BASE + 20),
            emulator.peek(MAILBOX_BASE + 24),
            emulator.peek(MAILBOX_BASE + 28),
            emulator.peek(MAILBOX_BASE + 32),
            emulator.peek(MAILBOX_BASE + 36),
        );

        let status = emulator.peek(MAILBOX_BASE + 4);
        let magic = emulator.peek(MAILBOX_BASE + 20);
        if status != 0 && magic == 0x4155_5242 {
            println!("firmware finished");
            break;
        }

        if core.is_halted() {
            let sp = core.regs.r[13];
            println!("HALTED (IPSR={}): stacked frame at 0x{sp:08x}", core.regs.xpsr & 0x1ff);
            for (index, label) in [
                "r0", "r1", "r2", "r3", "r12", "lr", "pc", "xpsr",
            ]
            .iter()
            .enumerate()
            {
                println!("  {label:>4} = 0x{:08x}", emulator.peek(sp + (index as u32) * 4));
            }
            for (label, addr) in [
                ("CFSR", 0xE000_ED28u32),
                ("HFSR", 0xE000_ED2C),
                ("MMFAR", 0xE000_ED34),
                ("BFAR", 0xE000_ED38),
            ] {
                println!("  {label:>5} = 0x{:08x}", emulator.mmio_read32(addr));
            }
            break;
        }
    }
}

fn dump_entry_bytes(elf_path: &Path, entry: u32) {
    let elf = std::fs::read(elf_path).expect("read ELF");
    let object = object::File::parse(&*elf).expect("parse ELF");
    let entry_even = entry & !1;
    for section in object.sections() {
        let address = section.address() as u32;
        let size = section.size() as u32;
        if entry_even >= address && entry_even < address + size {
            let data = section.data().expect("section data");
            let start = (entry_even - address) as usize;
            let end = (start + 64).min(data.len());
            println!(
                "bytes at entry (section {} + 0x{start:x}): {:02x?}",
                section.name().unwrap_or("<unnamed>"),
                &data[start..end]
            );
            return;
        }
    }
    println!("entry not inside any section");
}

fn dump_layout(elf_path: &Path) {
    let elf = std::fs::read(elf_path).expect("read ELF");
    let object = object::File::parse(&*elf).expect("parse ELF");

    println!("--- sections ---");
    for section in object.sections() {
        let name = section.name().unwrap_or("<unnamed>");
        if !(name.starts_with(".text")
            || name.starts_with(".rodata")
            || name.starts_with(".data")
            || name.starts_with(".bss")
            || name.starts_with(".vectors")
            || name.starts_with(".picobin"))
        {
            continue;
        }
        println!(
            "{name:<28} addr=0x{:08x} size=0x{:x}",
            section.address(),
            section.size()
        );
    }

    println!("--- symbols of interest ---");
    for symbol in object.symbols() {
        let name = symbol.name().unwrap_or("<unnamed>");
        let included = name.contains("MAILBOX")
            || name.contains("_start")
            || name.contains("run_scenario")
            || (symbol.address() >= 0x2000_0000 && symbol.address() < 0x2000_0400);
        if included {
            println!(
                "{name:<40} addr=0x{:08x} size=0x{:x}",
                symbol.address(),
                symbol.size()
            );
        }
    }
}
