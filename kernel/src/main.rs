#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]

#[macro_use]
mod serial;
mod gdt;
mod interrupts;
mod memory;
mod allocator;
mod frame_allocator;
mod paging;
mod s3fifo;
mod ring_buffer;
mod events;
mod task;

use bootloader_api::{entry_point, BootInfo};
use bootloader_api::config::{BootloaderConfig, Mapping};

/// Bootloader configuration: request physical memory mapping
pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config.kernel_stack_size = 128 * 1024; // 128 KiB stack
    config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    serial::init();
    serial_println!("╔══════════════════════════════════════════╗");
    serial_println!("║     Rust OS v0.1 — Lightweight Kernel   ║");
    serial_println!("║     x86_64 | QEMU | BIOS Boot           ║");
    serial_println!("╚══════════════════════════════════════════╝");
    serial_println!();

    // Init GDT (with TSS for double fault stack)
    serial_println!("[init] Setting up GDT...");
    gdt::init();
    serial_println!("[init] GDT loaded.");

    // Init IDT + PIC
    serial_println!("[init] Setting up IDT and PIC...");
    interrupts::init();
    serial_println!("[init] IDT loaded. Interrupts disabled.");

    // Init memory info
    serial_println!("[init] Probing memory...");
    memory::init(&boot_info.memory_regions);

    // Init heap allocator
    serial_println!("[init] Initializing kernel heap...");
    allocator::init(&boot_info.memory_regions);

    // Init frame allocator
    serial_println!("[init] Initializing frame allocator...");
    frame_allocator::init();

    // Init paging (real page tables)
    let phys_offset = boot_info.physical_memory_offset
        .into_option()
        .expect("physical_memory_offset not set by bootloader");
    serial_println!("[init] Initializing page tables...");
    paging::init(phys_offset);

    // Log boot event
    events::log_event(events::EventType::SystemBoot as u8, 0, 0, 0);

    serial_println!();
    s3fifo::run_tests();

    serial_println!();
    paging::run_test();

    serial_println!();
    serial_println!("=== Task Demo ===");
    task::demo();

    serial_println!();
    serial_println!("=== System halted ===");
    loop {
        x86_64::instructions::hlt();
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    serial_println!();
    serial_println!("╔═══════════════════════════════════╗");
    serial_println!("║        KERNEL PANIC               ║");
    serial_println!("╚═══════════════════════════════════╝");
    let msg = info.message();
    serial_println!("Message: {}", msg);
    if let Some(loc) = info.location() {
        serial_println!("Location: {}:{}:{}", loc.file(), loc.line(), loc.column());
    }
    loop {
        x86_64::instructions::hlt();
    }
}
