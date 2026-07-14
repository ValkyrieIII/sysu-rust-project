//! Real x86_64 4-level page table management.
//! Uses the physical memory offset from boot_info to access page tables.

use x86_64::structures::paging::{
    OffsetPageTable, PageTable, PageTableFlags as Flags,
    Page, PhysFrame, Size4KiB, Mapper, FrameAllocator as X64FrameAlloc,
};
use x86_64::{PhysAddr, VirtAddr};
use crate::frame_allocator;

/// Virtual address range for "user" pages managed by S3-FIFO.
const USER_AREA_START: u64 = 0x4000_0000_0000;
const USER_AREA_PAGES: usize = 256;

/// Global page table mapper.
static mut MAPPER: Option<OffsetPageTable<'static>> = None;

/// Track which virtual page each physical frame maps to.
static mut FRAME_TO_VPAGE: [Option<u64>; frame_allocator::TOTAL_FRAMES] =
    [None; frame_allocator::TOTAL_FRAMES];

/// Page fault event counter.
static mut PF_COUNT: u64 = 0;

/// Wrapper to make our frame allocator work with x86_64's FrameAllocator trait.
struct KernelFrameAllocator;

unsafe impl X64FrameAlloc<Size4KiB> for KernelFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        frame_allocator::alloc_frame().map(|f| {
            PhysFrame::from_start_address(
                PhysAddr::new(frame_allocator::frame_to_phys(f))
            ).unwrap()
        })
    }
}

/// Initialize paging: get the active PML4 and create an OffsetPageTable.
pub fn init(physical_memory_offset: u64) {
    let (pml4_frame, _) = x86_64::registers::control::Cr3::read();
    let pml4_phys = pml4_frame.start_address().as_u64();
    let pml4_virt = physical_memory_offset + pml4_phys;
    let pml4: &'static mut PageTable = unsafe { &mut *(pml4_virt as *mut PageTable) };

    let mapper = unsafe {
        OffsetPageTable::new(pml4, VirtAddr::new(physical_memory_offset))
    };

    unsafe { MAPPER = Some(mapper); }

    serial_println!(
        "[paging] PML4 at phys={:#x}, offset={:#x}, mapper ready",
        pml4_phys, physical_memory_offset,
    );
    serial_println!(
        "[paging] User area: {:#x} - {:#x} ({} pages)",
        USER_AREA_START,
        USER_AREA_START + (USER_AREA_PAGES * 4096) as u64,
        USER_AREA_PAGES,
    );
}

/// Map a virtual page in the user area to a physical frame number.
/// Returns true on success.
pub fn map_user_page(vpage: u64, frame_num: usize) -> bool {
    let mapper = unsafe { MAPPER.as_mut().unwrap() };
    let va = VirtAddr::new(USER_AREA_START + vpage * 4096);
    let page = Page::<Size4KiB>::from_start_address(va).unwrap();
    let phys = PhysAddr::new(frame_allocator::frame_to_phys(frame_num));
    let phys_frame = PhysFrame::from_start_address(phys).unwrap();

    let mut fa = KernelFrameAllocator;
    let flags = Flags::PRESENT | Flags::WRITABLE;

    match unsafe { mapper.map_to(page, phys_frame, flags, &mut fa) } {
        Ok(mapper_flush) => {
            mapper_flush.flush();
            unsafe {
                FRAME_TO_VPAGE[frame_num] = Some(vpage);
            }
            true
        }
        Err(e) => {
            serial_println!("[paging] map_to failed: {:?}", e);
            false
        }
    }
}

/// Unmap a virtual page in the user area.
pub fn unmap_user_page(vpage: u64) {
    let mapper = unsafe { MAPPER.as_mut().unwrap() };
    let va = VirtAddr::new(USER_AREA_START + vpage * 4096);
    let page = Page::<Size4KiB>::from_start_address(va).unwrap();

    let (_frame, flush) = mapper.unmap(page).unwrap();
    flush.flush();
}

/// Check if addr is in the user-managed area.
pub fn is_user_addr(addr: VirtAddr) -> bool {
    let a = addr.as_u64();
    a >= USER_AREA_START && a < USER_AREA_START + (USER_AREA_PAGES * 4096) as u64
}

/// Convert a user virtual address to page number.
pub fn addr_to_vpage(addr: VirtAddr) -> u64 {
    (addr.as_u64() - USER_AREA_START) / 4096
}

/// Convert a virtual page number to address.
pub fn vpage_to_addr(vpage: u64) -> VirtAddr {
    VirtAddr::new(USER_AREA_START + vpage * 4096)
}

/// Get the page fault count.
pub fn pf_count() -> u64 {
    unsafe { PF_COUNT }
}

/// Increment the page fault counter.
pub fn inc_pf_count() {
    unsafe { PF_COUNT += 1; }
}

/// Translate a virtual page to its physical frame number (if mapped).
pub fn vpage_to_frame(vpage: u64) -> Option<usize> {
    let mapper = unsafe { MAPPER.as_ref().unwrap() };
    let va = VirtAddr::new(USER_AREA_START + vpage * 4096);
    let page = Page::<Size4KiB>::from_start_address(va).unwrap();

    match mapper.translate_page(page) {
        Ok(phys_frame) => {
            let phys = phys_frame.start_address().as_u64();
            frame_allocator::phys_to_frame(phys)
        }
        Err(_) => None,
    }
}

/// Get the virtual page that a frame is mapped to (if any).
pub fn frame_to_vpage(frame: usize) -> Option<u64> {
    unsafe { FRAME_TO_VPAGE[frame] }
}

/// Try to read a value from a user virtual address (for testing page faults).
/// Returns Some(value) if mapped, None if page fault.
pub unsafe fn read_user_u64(vpage: u64) -> u64 {
    let ptr = (USER_AREA_START + vpage * 4096) as *const u64;
    core::ptr::read_volatile(ptr)
}

/// Try to write a value to a user virtual address (for testing page faults).
pub unsafe fn write_user_u64(vpage: u64, value: u64) {
    let ptr = (USER_AREA_START + vpage * 4096) as *mut u64;
    core::ptr::write_volatile(ptr, value);
}

/// Run a page table stress test.
pub fn run_test() {
    serial_println!();
    serial_println!("=== Page Table Test ===");

    // Map page 0 and write to it
    let frame0 = frame_allocator::alloc_frame().unwrap();
    serial_println!("[ptest] Mapping vpage 0 → frame {}", frame0);
    map_user_page(0, frame0);

    serial_println!("[ptest] Writing to vpage 0...");
    unsafe { write_user_u64(0, 0xDEAD_BEEF_CAFE_BABE); }

    let val = unsafe { read_user_u64(0) };
    serial_println!("[ptest] Read back: {:#x}", val);
    assert!(val == 0xDEAD_BEEF_CAFE_BABE);

    // Unmap it
    serial_println!("[ptest] Unmapping vpage 0");
    unmap_user_page(0);
    frame_allocator::free_frame(frame0);

    serial_println!("[ptest] Page table test passed.");
}
