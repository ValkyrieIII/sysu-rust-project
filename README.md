# Rust OS — 轻量 x86_64 内核

基于 Rust 从零构建的 x86_64 操作系统内核，在 QEMU 中运行。支持四级页表、S3-FIFO 页面淘汰算法、事件日志系统、协作式任务调度。

## 快速开始

### 环境要求

- Rust nightly（项目锁定 `nightly-2026-05-01`，见 `rust-toolchain.toml`）
- QEMU — 通过项目根目录的 `.env` 文件配置路径，或从 `PATH` 查找
- `bindeps` 功能（已在 `.cargo/config.toml` 中启用）

初次使用需安装工具链和组件：

```bash
rustup toolchain install nightly-2026-05-01
rustup target add x86_64-unknown-none --toolchain nightly-2026-05-01
rustup component add rust-src llvm-tools-preview --toolchain nightly-2026-05-01
```

### 构建与运行

```bash
cargo build
cargo run
```

### 串口输出

内核通过 **COM1 串口**（`0x3F8`）输出日志。QEMU 配置 `-serial stdio` 将串口重定向到终端，因此：

- **终端窗口** — 所有内核日志（初始化、测试结果、S3-FIFO 对比数据）
- **QEMU GUI 窗口** — VESA framebuffer（内核未使用 VGA 输出）

> 如需纯终端模式（不弹出 QEMU 窗口），可将 `src/main.rs` 中的 `-serial stdio` 替换为 `-nographic`。

## 项目结构

```
os/
├── Cargo.toml              # workspace root（artifact 依赖 kernel）
├── rust-toolchain.toml     # 锁定 nightly-2026-05-01
├── .cargo/config.toml      # 启用 bindeps
├── .env                    # QEMU 路径配置（通过 dotenvy 自动加载）
├── build.rs                # bootloader 生成 BIOS/UEFI 磁盘镜像
├── src/main.rs             # QEMU 启动器
├── run.ps1                 # PowerShell 一键构建+运行
├── kernel/                 # 内核 crate
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs         # 入口 kernel_main
│       ├── serial.rs       # COM1 串口驱动（38400 bps, 8N1）
│       ├── gdt.rs          # GDT + TSS（Double Fault IST）
│       ├── interrupts.rs   # IDT + CPU 异常处理 + 缺页分配
│       ├── memory.rs       # E820 物理内存检测
│       ├── allocator.rs    # Bump 堆分配器
│       ├── frame_allocator.rs  # 位图物理页框分配器
│       ├── paging.rs       # x86_64 四级页表（OffsetPageTable）
│       ├── ring_buffer.rs  # 无锁环形缓冲区（AtomicU64）
│       ├── events.rs       # 事件日志系统（8 种事件类型）
│       ├── s3fifo.rs       # S3-FIFO 页面淘汰 + FIFO 对比实验
│       └── task.rs         # 协作式任务调度
└── README.md
```

## 功能

### V1 — 基础内核
- BIOS 启动 → 64 位长模式（bootloader crate v0.11）
- 自写 COM1 串口驱动（38400 bps, 8N1）
- GDT + TSS（Double Fault 独立栈，IST）
- IDT：breakpoint / double fault / page fault 异常处理
- E820 物理内存检测（~121 MiB 可用）
- Bump allocator（2 MiB 内核堆）
- 无锁环形缓冲区（256 条目，AtomicU64）
- 8 种事件类型日志
- 协作式任务调度（spawn / yield / exit）

### V2 — 内存管理
- 位图物理页框分配器（128 frames × 4 KiB = 512 KiB 池）
- **S3-FIFO 页面淘汰算法**（SOSP 2023 论文实现，S + M 双队列，2-bit 访问计数）
- x86_64 四级页表（OffsetPageTable，`USER_ACCESSIBLE` 标志支持）
- 缺页中断处理（自动分配 → S3-FIFO 淘汰 → 重建映射）
- **FIFO vs S3-FIFO 对比实验**（随机 / 热点 / 顺序扫描三种访问模式）

## 运行输出

```
╔══════════════════════════════════════════╗
║     Rust OS v0.1 — Lightweight Kernel   ║
║     x86_64 | QEMU | BIOS Boot           ║
╚══════════════════════════════════════════╝

[init] Setting up GDT...
[init] GDT loaded.
[init] Setting up IDT and PIC...
[init] IDT loaded. Interrupts disabled.
[init] Probing memory...
[memory] Total usable: 121 MiB, Largest region: 106 MiB
[init] Initializing kernel heap...
[allocator] Heap: 2048 KiB
[init] Initializing frame allocator...
[frame] Pool: 128 frames, 512 KiB
[init] Initializing page tables...
[paging] PML4 ready, user area: 256 pages

All S3-FIFO tests passed.

=== Page Table Test ===
[ptest] Mapping vpage 0 → frame 0
[ptest] Write: 0xdeadbeefcafebabe
[ptest] Read:  0xdeadbeefcafebabe ✓
[ptest] Page table test passed.

╔══════════════════════════════════════════╗
║   V2: S3-FIFO vs FIFO Comparison        ║
╚══════════════════════════════════════════╝

[1] Random access (100 pages, 1000 accesses)
[2] Hotspot (80% on 8 pages, 20% random over 200)
[3] Sequential scan (0..200 repeated)

=== Task Demo ===
[task] Spawned: pid=1, name=TaskA
[task] Spawned: pid=2, name=TaskB
[sched] Context switch: pid 1 -> pid 2

=== System halted ===
```

## 环境

| 组件     | 版本/说明                                                                           |
| -------- | ----------------------------------------------------------------------------------- |
| 编译器   | Rust nightly-2026-05-01 (`x86_64-unknown-none`)                                     |
| 模拟器   | QEMU 11.0 (x86_64, BIOS)                                                            |
| 启动方式 | bootloader crate v0.11                                                              |
| 核心依赖 | `x86_64 0.14`, `bootloader_api 0.11`, `heapless 0.8`, `spin 0.9`, `lazy_static 1.4`, `dotenvy 0.15` |

## 常见问题

### `cargo build` 报 "offset is not a multiple of 16"

确保在 workspace 根目录（`os/`）执行 `cargo build`，而非在 `kernel/` 子目录。workspace 的 artifact 依赖会自动为 kernel 指定正确的交叉编译目标。

### 看不到串口输出

- Git Bash 中运行时终端即显示串口输出
- PowerShell 中如果输出不可见，将 `src/main.rs` 中的 `-serial stdio` 替换为 `-nographic`

### QEMU 不在默认路径

编辑项目根目录的 `.env` 文件，修改 `QEMU_PATH` 为实际安装路径即可。`.env` 由 `dotenvy` 自动加载，无需手动设置环境变量。


## 参考

- [Writing an OS in Rust](https://os.phil-opp.com/) — Philipp Oppermann
- [S3-FIFO: Scalable, Self-Adjusting FIFO](https://dl.acm.org/doi/10.1145/3600006.3613147) — SOSP 2023
- [bootloader crate](https://github.com/rust-osdev/bootloader) — v0.11
- [x86_64 crate](https://docs.rs/x86_64) — 类型安全的 x86_64 系统编程库
