//! Interrupt handling: IDT and CPU exception handlers.
//! Hardware interrupts (PIC) are not used in V1/V2.

use x86_64::structures::idt::{
    InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode,
};

// ---- IDT ----

lazy_static::lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();

        // CPU exception handlers
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);
        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(crate::gdt::DOUBLE_FAULT_IST_INDEX);
        }

        idt
    };
}

pub fn init() {
    serial_println!("[init] Loading IDT...");
    IDT.load();
    serial_println!("[init] IDT loaded.");
    // Interrupts remain disabled — PIC is not initialized.
}

// ---- Exception Handlers ----

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    serial_println!(
        "[EXCEPTION] BREAKPOINT at {:#x}",
        stack_frame.instruction_pointer,
    );
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!(
        "DOUBLE FAULT at {:#x}\n{:#?}",
        stack_frame.instruction_pointer, stack_frame
    );
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;
    let fault_addr = Cr2::read();

    // Only handle faults in the user-managed area
    if !crate::paging::is_user_addr(fault_addr) {
        panic!(
            "PAGE FAULT in kernel space at {:#x}, ip={:#x}, error={:?}",
            fault_addr,
            stack_frame.instruction_pointer,
            error_code,
        );
    }

    let vpage = crate::paging::addr_to_vpage(fault_addr);
    crate::paging::inc_pf_count();

    // Try to allocate a free frame
    let frame = match crate::frame_allocator::alloc_frame() {
        Some(f) => f,
        None => {
            // No free frames — use S3-FIFO to evict one user page
            let evicted_vpage = crate::s3fifo::evict_one_global();
            if let Some(old_frame) = crate::paging::vpage_to_frame(evicted_vpage) {
                crate::paging::unmap_user_page(evicted_vpage);
                crate::frame_allocator::free_frame(old_frame);
            }
            crate::frame_allocator::alloc_frame()
                .expect("frame_allocator: no free frame after eviction")
        }
    };

    // Map vpage → frame
    crate::paging::map_user_page(vpage, frame);
    // Track in S3-FIFO
    crate::s3fifo::access_global(vpage);
}
