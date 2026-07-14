use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;

/// Number of entries in the ring buffer (must be power of 2)
pub const RING_SIZE: usize = 256;

/// A single event entry — 40 bytes
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Event {
    pub event_type: u8,
    pub _pad: [u8; 7],
    pub arg1: u64,
    pub arg2: u64,
    pub arg3: u64,
}

impl Event {
    pub const fn zero() -> Self {
        Event {
            event_type: 0,
            _pad: [0; 7],
            arg1: 0,
            arg2: 0,
            arg3: 0,
        }
    }
}

/// Lock-free single-producer ring buffer using atomic head/tail (both u64).
pub struct EventRing {
    head: AtomicU64,
    tail: AtomicU64,
    buffer: [Event; RING_SIZE],
}

impl EventRing {
    pub const fn new() -> Self {
        EventRing {
            head: AtomicU64::new(0),
            tail: AtomicU64::new(0),
            buffer: [Event::zero(); RING_SIZE],
        }
    }

    /// Push an event. Overwrites oldest if full.
    /// # Safety
    /// Must be called with interrupts disabled or from a single context.
    pub unsafe fn push_unchecked(&mut self, event: Event) {
        let head = self.head.load(Ordering::Relaxed);
        let idx = (head as usize) % RING_SIZE;
        self.buffer[idx] = event;

        let new_head = head.wrapping_add(1);
        self.head.store(new_head, Ordering::Release);

        // If buffer is full, advance tail (drop oldest)
        let tail = self.tail.load(Ordering::Acquire);
        if new_head.wrapping_sub(tail) > RING_SIZE as u64 {
            self.tail.store(new_head.wrapping_sub(RING_SIZE as u64), Ordering::Release);
        }
    }

    /// Read available events into `out`. Returns count read.
    pub fn read(&mut self, out: &mut [Event]) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Relaxed);
        let available = head.wrapping_sub(tail) as usize;
        let count = available.min(out.len());

        for i in 0..count {
            let idx = (tail.wrapping_add(i as u64) as usize) % RING_SIZE;
            out[i] = self.buffer[idx];
        }

        if count > 0 {
            self.tail.store(tail.wrapping_add(count as u64), Ordering::Release);
        }
        count
    }

    /// Return number of unread events
    pub fn len(&self) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Relaxed);
        head.wrapping_sub(tail) as usize
    }

    /// Return total events ever written
    pub fn total_written(&self) -> u64 {
        self.head.load(Ordering::Relaxed)
    }
}

// Global ring buffer
static EVENT_RING: Mutex<EventRing> = Mutex::new(EventRing::new());

/// Push an event (interrupt-safe)
pub fn push(event: Event) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        unsafe {
            EVENT_RING.lock().push_unchecked(event);
        }
    });
}

/// Read events from the global ring buffer
pub fn read_events(out: &mut [Event]) -> usize {
    x86_64::instructions::interrupts::without_interrupts(|| {
        EVENT_RING.lock().read(out)
    })
}

/// Get current unread event count
pub fn pending_count() -> usize {
    EVENT_RING.lock().len()
}
