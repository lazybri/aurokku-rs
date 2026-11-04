//! Bump allocator for the emulator test firmware.
//!
//! The firmware needs `alloc` only because the shared protocol core uses
//! `Vec`/`String`. The test firmware runs one scenario and then spins, so a
//! never-freeing bump allocator over a static 64 KB arena is sufficient and
//! keeps the harness simple. This crate is test-only: `unsafe` is confined to
//! the allocator plumbing (the production firmware/HAL crates stay
//! `forbid(unsafe_code)`).
//!
//! Deliberately **non-atomic**: the firmware runs on one core with interrupts
//! disabled, and the RP2350 emulator used for the mock tests does not model the
//! exclusive monitor (`LDREX`/`STREX`) reliably, which made an atomic cursor
//! spin forever. A plain memory cursor is correct for this single-threaded
//! scenario runner and keeps the emulator tests meaningful.

use core::alloc::{GlobalAlloc, Layout};
use core::ptr;
use core::sync::atomic::{compiler_fence, Ordering};

/// Heap arena size. Peak protocol usage is a few KB (docs/plan/08 §4.3).
const HEAP_SIZE: usize = 64 * 1024;

#[repr(align(16))]
#[allow(dead_code)] // only its address is used; the bytes are the heap arena
struct HeapArena([u8; HEAP_SIZE]);

/// Zero-initialised SRAM arena (`.bss`, region defined by `link.x`).
static mut HEAP: HeapArena = HeapArena([0; HEAP_SIZE]);

/// Next free offset into [`HEAP`]; only touched from the single firmware core.
static mut HEAP_NEXT: usize = 0;

struct BumpAllocator;

// SAFETY: the arena is a dedicated static buffer; the cursor is only read and
// written from the single firmware core with interrupts disabled, so the
// non-atomic access cannot race. `dealloc` intentionally leaks (the firmware
// never frees before the scenario ends).
unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() == 0 {
            return ptr::null_mut();
        }
        let base = ptr::addr_of_mut!(HEAP) as usize;
        let cursor = ptr::read(ptr::addr_of!(HEAP_NEXT));
        let aligned = (cursor + layout.align() - 1) & !(layout.align() - 1);
        let Some(next) = aligned.checked_add(layout.size()) else {
            return ptr::null_mut();
        };
        if next > HEAP_SIZE {
            return ptr::null_mut();
        }
        ptr::write(ptr::addr_of_mut!(HEAP_NEXT), next);
        compiler_fence(Ordering::SeqCst);
        (base + aligned) as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator: individual frees are ignored; the arena is discarded
        // when the emulator instance is dropped.
    }
}

#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator;
