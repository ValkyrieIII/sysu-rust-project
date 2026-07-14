use crate::events;

const MAX_TASKS: usize = 4;

/// Simple task structure
struct Task {
    pid: u64,
    name: &'static str,
    state: TaskState,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TaskState {
    Ready,
    Running,
    Terminated,
}

/// Static task table
static mut TASKS: [Task; MAX_TASKS] = [
    Task { pid: 0, name: "", state: TaskState::Terminated },
    Task { pid: 0, name: "", state: TaskState::Terminated },
    Task { pid: 0, name: "", state: TaskState::Terminated },
    Task { pid: 0, name: "", state: TaskState::Terminated },
];
static mut TASK_COUNT: usize = 0;
static mut CURRENT_TASK: usize = 0;

/// Spawn a new task
pub fn spawn(name: &'static str) -> u64 {
    let pid;
    unsafe {
        if TASK_COUNT >= MAX_TASKS {
            return 0;
        }
        let idx = TASK_COUNT;
        TASKS[idx] = Task {
            pid: (idx + 1) as u64,
            name,
            state: TaskState::Ready,
        };
        pid = TASKS[idx].pid;
        TASK_COUNT += 1;
    }
    events::log_event(events::EventType::ProcessCreate as u8, pid, 0, 0);
    serial_println!("[task] Spawned: pid={}, name={}", pid, name);
    pid
}

/// Yield CPU from current task to next ready task
pub fn yield_cpu() {
    let (from_pid, to_pid);
    unsafe {
        if TASK_COUNT < 2 {
            return;
        }

        let prev = CURRENT_TASK;
        TASKS[prev].state = TaskState::Ready;
        CURRENT_TASK = (CURRENT_TASK + 1) % TASK_COUNT;
        TASKS[CURRENT_TASK].state = TaskState::Running;

        from_pid = TASKS[prev].pid;
        to_pid = TASKS[CURRENT_TASK].pid;
    }

    events::log_context_switch(from_pid, to_pid);
    serial_println!(
        "[sched] Context switch: pid {} -> pid {}",
        from_pid, to_pid
    );
}

/// Terminate the current task
pub fn exit() {
    let pid;
    unsafe {
        let idx = CURRENT_TASK;
        pid = TASKS[idx].pid;
        TASKS[idx].state = TaskState::Terminated;
    }
    events::log_event(events::EventType::ProcessDestroy as u8, pid, 0, 0);
    serial_println!("[task] Task pid={} exiting", pid);
}

/// Main demo: run S3-FIFO comparison, then V1 scheduling demo.
pub fn demo() {
    // ---- V2: S3-FIFO comparison ----
    crate::s3fifo::run_comparison();

    // ---- V1: scheduling demo ----
    let a = spawn("TaskA");
    let b = spawn("TaskB");

    serial_println!();
    serial_println!("=== Scheduling Demo ===");

    for round in 0..4 {
        yield_cpu();
        for _ in 0..300_000 {
            core::hint::spin_loop();
        }
    }

    serial_println!();
    serial_println!("=== System Summary ===");
    serial_println!("Tasks created: 2 (pid {} and {})", a, b);
    serial_println!("Heap allocations: {}", crate::allocator::allocations());
}
