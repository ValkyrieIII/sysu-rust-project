//! S3-FIFO page replacement algorithm (SOSP 2023, simplified).
//! Two queues: S (small probation queue) + M (main queue).
//! Each page has a 2-bit access counter (0..=3).

use heapless::{Deque, Vec as HVec};
use spin::Mutex;
use crate::frame_allocator::TOTAL_FRAMES;
use crate::events;

// ---- Global S3-FIFO for page fault handler ----

lazy_static::lazy_static! {
    static ref GLOBAL_S3FIFO: Mutex<S3Fifo> = Mutex::new(S3Fifo::new());
}

/// Called by page fault handler when a user page is accessed.
pub fn access_global(vpage: u64) {
    GLOBAL_S3FIFO.lock().access(vpage as usize);
}

/// Evict one page from the global S3-FIFO. Returns the evicted vpage.
/// Panics if the cache is empty.
pub fn evict_one_global() -> u64 {
    let mut cache = GLOBAL_S3FIFO.lock();
    let total = cache.s_queue.len() + cache.m_queue.len();
    if total == 0 {
        panic!("S3-FIFO: tried to evict from empty cache");
    }
    cache.evict_one() as u64
}

pub const S_CAP: usize = TOTAL_FRAMES / 8;        // 16
pub const M_CAP: usize = TOTAL_FRAMES - S_CAP;    // 112
/// Maximum page identifier we track (bigger than TOTAL_FRAMES for testing).
const MAX_PAGES: usize = 512;

pub struct S3Fifo {
    s_queue: Deque<usize, S_CAP>,
    m_queue: Deque<usize, M_CAP>,
    /// Access counter per page (2-bit: 0..=3)
    access_count: [u8; MAX_PAGES],
    /// Track whether a page is currently cached.
    in_cache: [bool; MAX_PAGES],
    /// Stats
    pub hits: u64,
    pub misses: u64,
}

impl S3Fifo {
    pub fn new() -> Self {
        S3Fifo {
            s_queue: Deque::new(),
            m_queue: Deque::new(),
            access_count: [0u8; MAX_PAGES],
            in_cache: [false; MAX_PAGES],
            hits: 0,
            misses: 0,
        }
    }

    /// Access a frame. Returns true if hit (already cached).
    pub fn access(&mut self, frame: usize) -> bool {
        if self.in_cache[frame] {
            // Hit: increment access counter, capped at 3
            if self.access_count[frame] < 3 {
                self.access_count[frame] += 1;
            }
            self.hits += 1;
            true
        } else {
            // Miss: trigger insertion
            self.misses += 1;
            self.insert(frame);
            false
        }
    }

    /// Insert a new page into the cache. Evicts if total cache is full.
    fn insert(&mut self, frame: usize) {
        let total = self.s_queue.len() + self.m_queue.len();

        if total >= TOTAL_FRAMES {
            // Cache full — must evict one
            let evicted = self.evict_one();
            self.in_cache[evicted] = false;
            self.access_count[evicted] = 0;
            events::log_event(events::EventType::PageEvict as u8, evicted as u64, 0, 0);
        } else if self.s_queue.is_full() {
            // S full but M has room — promote one from S to M
            self.promote_s_to_m();
        }

        self.s_queue.push_back(frame).ok();
        self.in_cache[frame] = true;
        self.access_count[frame] = 0;
    }

    /// Move one page from S queue to M queue (promotion).
    fn promote_s_to_m(&mut self) {
        if let Some(frame) = self.s_queue.pop_front() {
            // Don't check access_count — just promote directly since we need S room
            self.m_queue.push_back(frame).ok();
        }
    }

    /// Evict one page according to S3-FIFO rules.
    /// Returns the evicted frame number.
    fn evict_one(&mut self) -> usize {
        // Phase 1: drain S queue — move accessed pages to M
        while let Some(frame) = self.s_queue.pop_front() {
            if self.access_count[frame] > 0 {
                self.access_count[frame] -= 1;
                if self.m_queue.push_back(frame).is_err() {
                    // M is full — evict from M first, then retry
                    let evicted = self.evict_from_m();
                    self.in_cache[evicted] = false;
                    self.access_count[evicted] = 0;
                    self.m_queue.push_back(frame).ok();
                    return evicted;
                }
            } else {
                return frame; // evicted from S with access_count == 0
            }
        }

        // Phase 2: S is empty, evict from M
        self.evict_from_m()
    }

    /// Scan M queue for a page to evict.
    fn evict_from_m(&mut self) -> usize {
        loop {
            if let Some(frame) = self.m_queue.pop_front() {
                if self.access_count[frame] > 0 {
                    self.access_count[frame] -= 1;
                    // Reinsert at back — give it another chance
                    self.m_queue.push_back(frame).ok();
                } else {
                    return frame;
                }
            }
        }
    }

    /// Return current queue lengths.
    pub fn stats(&self) -> (usize, usize) {
        (self.s_queue.len(), self.m_queue.len())
    }

    /// Hit rate as percentage × 10 (e.g. 718 = 71.8%).
    pub fn hit_rate(&self) -> usize {
        let total = self.hits + self.misses;
        if total == 0 {
            return 0;
        }
        ((self.hits as f64 / total as f64) * 1000.0) as usize
    }

    pub fn reset_stats(&mut self) {
        self.hits = 0;
        self.misses = 0;
    }
}

// ---- Self-test (runs without hardware page tables) ----

/// Run S3-FIFO algorithm tests. Panics on failure.
pub fn run_tests() {
    serial_println!("=== S3-FIFO Unit Tests ===");

    test_basic_insert_evict();
    test_hotspot_survival();
    test_sequential_scan();
    test_fifo_baseline();

    serial_println!("All S3-FIFO tests passed.");
}

fn test_basic_insert_evict() {
    let mut cache = S3Fifo::new();

    // Fill cache completely — pages without access get promoted from S to M
    // (no eviction until total reaches TOTAL_FRAMES).
    for i in 0..TOTAL_FRAMES {
        assert!(!cache.access(i)); // all cold misses
    }
    // S + M should now be full
    assert_eq!(cache.s_queue.len() + cache.m_queue.len(), TOTAL_FRAMES);

    // One more access triggers eviction
    cache.access(TOTAL_FRAMES);
    assert_eq!(cache.s_queue.len() + cache.m_queue.len(), TOTAL_FRAMES);

    serial_println!("  [OK] basic insert/evict (total cached: {})", TOTAL_FRAMES);
}

fn test_hotspot_survival() {
    let mut cache = S3Fifo::new();

    // Fill cache, accessing pages 0-3 twice each time to build up access count
    for i in 0..TOTAL_FRAMES {
        cache.access(i);
        if i < 4 {
            cache.access(i); // double-access hot pages
        }
    }

    // Frames 0-3 should have survived (promoted to M)
    assert!(cache.in_cache[0]);
    assert!(cache.in_cache[1]);
    assert!(cache.in_cache[2]);
    assert!(cache.in_cache[3]);

    // Keep accessing hot pages while flooding with cold pages
    for i in 128..256 {
        cache.access(i);
        for hot in 0..4 {
            cache.access(hot);
        }
    }

    // Hot pages should still be in cache
    assert!(cache.in_cache[0]);
    assert!(cache.in_cache[1]);
    assert!(cache.in_cache[2]);
    assert!(cache.in_cache[3]);

    serial_println!("  [OK] hotspot survival");
}

fn test_sequential_scan() {
    let mut cache = S3Fifo::new();

    // Sequential scan: each page accessed exactly once
    for i in 0..200 {
        cache.access(i);
    }

    // After filling the cache, evictions keep the total at TOTAL_FRAMES.
    let misses_after_fill = cache.misses - TOTAL_FRAMES as u64;
    assert!(misses_after_fill > 0);
    assert_eq!(cache.s_queue.len() + cache.m_queue.len(), TOTAL_FRAMES);

    serial_println!("  [OK] sequential scan");
}

fn test_fifo_baseline() {
    let mut fifo = FifoCache::new();
    let pattern = hotspot_pattern(4, 128, 200);
    for &page in &pattern {
        fifo.access(page);
    }
    serial_println!(
        "  [OK] FIFO baseline: {}/{} hits ({:.1}%)",
        fifo.hits, fifo.hits + fifo.misses,
        fifo.hit_rate_pct() * 100.0,
    );
}

// ---- Comparison Experiments ----

/// Trait for page replacement algorithms (for side-by-side comparison).
trait PageCache {
    fn access(&mut self, page: usize) -> bool;
    fn hit_rate_pct(&self) -> f64;
    fn reset_stats(&mut self);
}

impl PageCache for S3Fifo {
    fn access(&mut self, page: usize) -> bool { self.access(page) }
    fn hit_rate_pct(&self) -> f64 {
        let t = self.hits + self.misses;
        if t == 0 { 0.0 } else { self.hits as f64 / t as f64 }
    }
    fn reset_stats(&mut self) { self.hits = 0; self.misses = 0; }
}

/// Simple FIFO cache for comparison.
struct FifoCache {
    queue: Deque<usize, TOTAL_FRAMES>,
    hits: u64,
    misses: u64,
    in_cache: [bool; MAX_PAGES],
}

impl FifoCache {
    fn new() -> Self {
        FifoCache { queue: Deque::new(), hits: 0, misses: 0, in_cache: [false; MAX_PAGES] }
    }
}

impl PageCache for FifoCache {
    fn access(&mut self, page: usize) -> bool {
        if page >= MAX_PAGES { return false; }
        if self.in_cache[page] {
            self.hits += 1;
            return true;
        }
        self.misses += 1;
        if self.queue.len() >= TOTAL_FRAMES {
            if let Some(evicted) = self.queue.pop_front() {
                self.in_cache[evicted] = false;
            }
        }
        self.queue.push_back(page).ok();
        self.in_cache[page] = true;
        false
    }
    fn hit_rate_pct(&self) -> f64 {
        let t = self.hits + self.misses;
        if t == 0 { 0.0 } else { self.hits as f64 / t as f64 }
    }
    fn reset_stats(&mut self) { self.hits = 0; self.misses = 0; }
}

/// Simple LCG pseudo-random number generator.
struct Lcg { state: u64 }
impl Lcg {
    fn new(seed: u64) -> Self { Lcg { state: seed } }
    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.state
    }
}

/// Generate a random access pattern.
fn random_pattern(rng: &mut Lcg, n: usize, max_page: usize) -> HVec<usize, 1024> {
    let mut v: HVec<usize, 1024> = heapless::Vec::new();
    for _ in 0..n {
        v.push((rng.next() as usize) % max_page).ok();
    }
    v
}

/// Generate a hotspot pattern: 80% of accesses to `hot_count` hot pages.
fn hotspot_pattern(hot_count: usize, n: usize, max_page: usize) -> HVec<usize, 1024> {
    let mut v = HVec::new();
    let mut rng = Lcg::new(42);
    for _ in 0..n {
        if (rng.next() % 100) < 80 {
            v.push((rng.next() as usize) % hot_count).ok();
        } else {
            v.push(hot_count + (rng.next() as usize) % (max_page - hot_count)).ok();
        }
    }
    v
}

/// Generate a sequential scan pattern.
fn sequential_pattern(n: usize, max_page: usize) -> HVec<usize, 1024> {
    let mut v: HVec<usize, 1024> = heapless::Vec::new();
    for i in 0..n {
        v.push(i % max_page).ok();
    }
    v
}

/// Run the comparison experiments.
pub fn run_comparison() {
    serial_println!();
    serial_println!("╔══════════════════════════════════════════╗");
    serial_println!("║   V2: S3-FIFO vs FIFO Comparison        ║");
    serial_println!("╚══════════════════════════════════════════╝");
    serial_println!("Total frames: {}, S cap: {}, M cap: {}", TOTAL_FRAMES, S_CAP, M_CAP);

    let n = 1000;
    let max_page = 200;    // working set > cache (128) → some eviction inevitable
    let mut rng = Lcg::new(12345);

    // Pattern 1: Random with moderate working set
    serial_println!();
    serial_println!("[1] Random access (100 pages, {} accesses)", n);
    let random_pages = random_pattern(&mut rng, n, 100);
    compare("Random-100", &random_pages);

    // Pattern 2: Hotspot — 80% on 8 hot pages, 20% scattered
    serial_println!();
    serial_println!("[2] Hotspot (80% on 8 pages, 20% random over 200)");
    let hotspot = hotspot_pattern(8, n, 200);
    compare("Hotspot", &hotspot);

    // Pattern 3: Sequential scan (cyclic)
    serial_println!();
    serial_println!("[3] Sequential scan (0..{} repeated)", max_page);
    let seq = sequential_pattern(n, max_page);
    compare("Sequential", &seq);
}

fn compare(_label: &str, pages: &[usize]) {
    let mut fifo = FifoCache::new();
    let mut s3 = S3Fifo::new();

    for &page in pages {
        fifo.access(page);
        s3.access(page);
    }

    let total = pages.len() as u64;
    serial_println!(
        "  FIFO:     {:>4} / {} ({:>5.1}%)",
        fifo.hits, total, fifo.hit_rate_pct() * 100.0,
    );
    serial_println!(
        "  S3-FIFO:  {:>4} / {} ({:>5.1}%)",
        s3.hits, total, s3.hit_rate_pct() * 100.0,
    );
    if s3.hits > fifo.hits {
        serial_println!("  → S3-FIFO better by +{:.1}%", (s3.hit_rate_pct() - fifo.hit_rate_pct()) * 100.0);
    } else if fifo.hits > s3.hits {
        serial_println!("  → FIFO better by +{:.1}%", (fifo.hit_rate_pct() - s3.hit_rate_pct()) * 100.0);
    } else {
        serial_println!("  → Tie");
    }
}

