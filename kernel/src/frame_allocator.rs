//! Physical page frame allocator — bitmap-based.
//! Each 4 KiB frame = 1 bit. 0 = free, 1 = allocated.

use crate::memory;

pub const PAGE_SIZE: usize = 4096;

/// Number of frames to manage (512 KiB of memory for V2 demo).
pub const TOTAL_FRAMES: usize = 128;
const BITMAP_WORDS: usize = (TOTAL_FRAMES + 63) / 64; // ceil(TOTAL_FRAMES / 64)

/// Frame allocator state
pub struct FrameAllocator {
    /// Bitmap: each bit = one 4 KiB frame. 1 = allocated, 0 = free.
    bitmap: [u64; BITMAP_WORDS],
    /// Physical address of frame 0.
    base: u64,
    /// Number of free frames.
    free_count: usize,
}

impl FrameAllocator {
    pub const fn new() -> Self {
        FrameAllocator {
            bitmap: [0; BITMAP_WORDS],
            base: 0,
            free_count: 0,
        }
    }

    /// Initialize: mark the frame pool region as all free.
    /// The pool starts after the kernel heap.
    pub fn init(&mut self) {
        let mem = memory::info();
        // Frame pool: heap_end → end of largest usable region
        self.base = mem.largest_usable_region_start + mem.largest_usable_region_size
            - (TOTAL_FRAMES * PAGE_SIZE) as u64;
        // Page-align
        self.base = (self.base + PAGE_SIZE as u64 - 1) & !(PAGE_SIZE as u64 - 1);

        self.bitmap = [0; BITMAP_WORDS];
        self.free_count = TOTAL_FRAMES;

        serial_println!(
            "[frame] Pool: {:#x} - {:#x} ({} frames, {} KiB)",
            self.base,
            self.base + (TOTAL_FRAMES * PAGE_SIZE) as u64,
            TOTAL_FRAMES,
            (TOTAL_FRAMES * PAGE_SIZE) / 1024,
        );
    }

    /// Allocate a single frame. Returns frame number (0..TOTAL_FRAMES-1).
    pub fn alloc_frame(&mut self) -> Option<usize> {
        if self.free_count == 0 {
            return None;
        }

        // Scan bitmap for first free bit
        for word_idx in 0..BITMAP_WORDS {
            let word = self.bitmap[word_idx];
            if word != u64::MAX {
                // Found a word with at least one free bit
                let bit_idx = word.trailing_ones() as usize;
                let frame = word_idx * 64 + bit_idx;
                if frame < TOTAL_FRAMES {
                    self.bitmap[word_idx] |= 1 << bit_idx;
                    self.free_count -= 1;
                    return Some(frame);
                }
            }
        }
        None
    }

    /// Free a previously allocated frame.
    pub fn free_frame(&mut self, frame: usize) {
        assert!(frame < TOTAL_FRAMES);
        let word_idx = frame / 64;
        let bit_idx = frame % 64;
        if self.bitmap[word_idx] & (1 << bit_idx) != 0 {
            self.bitmap[word_idx] &= !(1 << bit_idx);
            self.free_count += 1;
        }
    }

    /// Convert frame number to physical address.
    pub fn frame_to_phys(&self, frame: usize) -> u64 {
        self.base + (frame * PAGE_SIZE) as u64
    }

    pub fn free_count(&self) -> usize {
        self.free_count
    }

    /// Convert a physical address back to a frame number (if in our pool).
    pub fn phys_to_frame(&self, phys: u64) -> Option<usize> {
        if phys < self.base || phys >= self.base + (TOTAL_FRAMES * PAGE_SIZE) as u64 {
            return None;
        }
        Some(((phys - self.base) / PAGE_SIZE as u64) as usize)
    }
}

// Global frame allocator — Mutex for interrupt safety
use spin::Mutex;
static FRAME_ALLOCATOR: Mutex<FrameAllocator> = Mutex::new(FrameAllocator::new());

pub fn init() {
    FRAME_ALLOCATOR.lock().init();
}

pub fn alloc_frame() -> Option<usize> {
    FRAME_ALLOCATOR.lock().alloc_frame()
}

pub fn free_frame(frame: usize) {
    FRAME_ALLOCATOR.lock().free_frame(frame)
}

pub fn free_count() -> usize {
    FRAME_ALLOCATOR.lock().free_count()
}

pub fn frame_to_phys(frame: usize) -> u64 {
    FRAME_ALLOCATOR.lock().frame_to_phys(frame)
}

pub fn phys_to_frame(phys: u64) -> Option<usize> {
    FRAME_ALLOCATOR.lock().phys_to_frame(phys)
}
