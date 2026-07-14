use core::alloc::{GlobalAlloc, Layout};
use spin::Mutex;
use bootloader_api::info::{MemoryRegions, MemoryRegionKind};

pub const HEAP_SIZE: usize = 2 * 1024 * 1024; // 2 MiB

/// Bump allocator: simple, no `free()` reclaim (dealloc is no-op)
struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: usize,
    allocations: u64,
}

impl BumpAllocator {
    const fn new() -> Self {
        BumpAllocator {
            heap_start: 0,
            heap_end: 0,
            next: 0,
            allocations: 0,
        }
    }

    unsafe fn init(&mut self, start: usize, size: usize) {
        self.heap_start = start;
        self.heap_end = start + size;
        self.next = start;
    }
}

/// Wrapper to implement GlobalAlloc on Mutex<BumpAllocator>
/// (needed because of Rust's orphan rule)
pub struct LockedBumpAllocator(Mutex<BumpAllocator>);

impl LockedBumpAllocator {
    pub const fn new() -> Self {
        LockedBumpAllocator(Mutex::new(BumpAllocator::new()))
    }
}

unsafe impl GlobalAlloc for LockedBumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut alloc = self.0.lock();

        let align = layout.align().max(16);
        let size = layout.size();
        let next_aligned = (alloc.next + align - 1) & !(align - 1);

        if next_aligned + size > alloc.heap_end {
            return core::ptr::null_mut();
        }

        let ptr = next_aligned as *mut u8;
        alloc.next = next_aligned + size;
        alloc.allocations += 1;

        // Log allocation event
        crate::events::log_event(
            crate::events::EventType::MemAlloc as u8,
            size as u64,
            ptr as u64,
            0,
        );

        ptr
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, layout: Layout) {
        // Bump allocator: dealloc is a no-op
        // But we still log the event
        crate::events::log_event(
            crate::events::EventType::MemFree as u8,
            layout.size() as u64,
            _ptr as u64,
            0,
        );
    }
}

#[global_allocator]
static ALLOCATOR: LockedBumpAllocator = LockedBumpAllocator::new();

/// Initialize the kernel heap from available memory regions
pub fn init(memory_regions: &MemoryRegions) {
    let mut best_start: u64 = 0;
    let mut best_size: u64 = 0;

    for region in memory_regions.iter() {
        if region.kind == MemoryRegionKind::Usable {
            let size = region.end - region.start;
            if size > best_size {
                best_size = size;
                best_start = region.start;
            }
        }
    }

    let heap_size = (best_size as usize).min(HEAP_SIZE);
    // Skip the first 1 MiB (reserved for kernel .text/.data/.bss)
    let heap_start = ((best_start as usize).max(0x100000) + 0x1000) & !0xFFF;

    unsafe {
        ALLOCATOR.0.lock().init(heap_start, heap_size);
    }

    serial_println!(
        "[allocator] Heap: {:#x} - {:#x} ({} KiB), {} allocs",
        heap_start,
        heap_start + heap_size,
        heap_size / 1024,
        allocations(),
    );
}

/// Return number of allocations so far
pub fn allocations() -> u64 {
    ALLOCATOR.0.lock().allocations
}

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("allocation error: {:?}", layout);
}
