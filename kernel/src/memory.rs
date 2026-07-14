use bootloader_api::info::{MemoryRegions, MemoryRegionKind};

/// Physical memory summary parsed from bootloader info
pub struct MemoryInfo {
    pub total_usable: u64,
    pub largest_usable_region_start: u64,
    pub largest_usable_region_size: u64,
}

static mut MEMORY_INFO: MemoryInfo = MemoryInfo {
    total_usable: 0,
    largest_usable_region_start: 0,
    largest_usable_region_size: 0,
};

/// Parse memory regions from bootloader and store the summary
pub fn init(memory_regions: &MemoryRegions) {
    let mut total: u64 = 0;
    let mut largest_start: u64 = 0;
    let mut largest_size: u64 = 0;

    for region in memory_regions.iter() {
        serial_println!(
            "  Region: {:#018x} - {:#018x} ({:>6} KiB) {:?}",
            region.start,
            region.end,
            (region.end - region.start) / 1024,
            region.kind,
        );
        if region.kind == MemoryRegionKind::Usable {
            let size = region.end - region.start;
            total += size;
            if size > largest_size {
                largest_size = size;
                largest_start = region.start;
            }
        }
    }

    serial_println!(
        "[memory] Total usable: {} MiB, Largest region: {:#x} ({} MiB)",
        total / (1024 * 1024),
        largest_start,
        largest_size / (1024 * 1024),
    );

    unsafe {
        MEMORY_INFO = MemoryInfo {
            total_usable: total,
            largest_usable_region_start: largest_start,
            largest_usable_region_size: largest_size,
        };
    }
}

/// Get a reference to the stored memory info
pub fn info() -> &'static MemoryInfo {
    unsafe { &MEMORY_INFO }
}
