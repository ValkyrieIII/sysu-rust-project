use crate::ring_buffer;

/// Event type constants — kept as plain u8 for ABI simplicity
#[repr(u8)]
pub enum EventType {
    SystemBoot      = 0,
    ProcessCreate   = 1,
    ProcessDestroy  = 2,
    MemAlloc        = 3,
    MemFree         = 4,
    ContextSwitch   = 5,
    PageFault       = 6,
    PageEvict       = 7,
}

/// Log an event to the global ring buffer
///
/// # Arguments
/// * `event_type` - One of the EventType discriminants
/// * `arg1` - Primary argument (e.g. size for alloc, pid for process)
/// * `arg2` - Secondary argument (e.g. address for alloc)
/// * `arg3` - Tertiary argument (e.g. target pid for context switch)
pub fn log_event(event_type: u8, arg1: u64, arg2: u64, arg3: u64) {
    let event = ring_buffer::Event {
        event_type,
        _pad: [0; 7],
        arg1,
        arg2,
        arg3,
    };
    ring_buffer::push(event);
}

/// Log a context switch between two tasks
pub fn log_context_switch(from_pid: u64, to_pid: u64) {
    log_event(EventType::ContextSwitch as u8, from_pid, to_pid, 0);
}

/// Dump all pending events to serial, returning count dumped
pub fn dump_events() -> usize {
    let mut buf = [ring_buffer::Event::zero(); 64];
    let count = ring_buffer::read_events(&mut buf);

    for (i, event) in buf.iter().enumerate().take(count) {
        let label = match event.event_type {
            0 => "BOOT",
            1 => "PROC_CREATE",
            2 => "PROC_DESTROY",
            3 => "MEM_ALLOC",
            4 => "MEM_FREE",
            5 => "CTX_SWITCH",
            6 => "PAGE_FAULT",
            7 => "PAGE_EVICT",
            _ => "UNKNOWN",
        };
        serial_println!(
            "  [{:>3}] {:<12} arg1={:<8} arg2={:<8} arg3={:<8}",
            i, label, event.arg1, event.arg2, event.arg3,
        );
    }
    count
}
