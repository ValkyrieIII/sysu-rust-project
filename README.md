# Rust OS — 轻量 x86_64 内核

基于 Rust 从零构建的 x86_64 操作系统内核，在 QEMU 中运行。支持四级页表、S3-FIFO 页面淘汰算法、事件日志系统。

## 快速开始

```powershell
# 环境要求：nightly Rust + QEMU
rustup toolchain install nightly
rustup target add x86_64-unknown-none --toolchain nightly
rustup component add rust-src llvm-tools-preview --toolchain nightly

# 构建并运行
cargo build
cargo run
```

## 项目结构

```
os/
├── Cargo.toml              # workspace root
├── rust-toolchain.toml     # nightly-2026-05-01
├── build.rs                # 生成 BIOS/UEFI 磁盘镜像
├── src/main.rs             # QEMU 启动器
├── kernel/                 # 内核 crate
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs         # 入口 kernel_main
│       ├── serial.rs       # COM1 串口驱动
│       ├── gdt.rs          # GDT + TSS
│       ├── interrupts.rs   # IDT + CPU 异常处理
│       ├── memory.rs       # 物理内存检测
│       ├── allocator.rs    # Bump 堆分配器
│       ├── frame_allocator.rs  # 位图页框分配器
│       ├── paging.rs       # 四级页表操作
│       ├── ring_buffer.rs  # 无锁环形缓冲区
│       ├── events.rs       # 事件日志系统
│       ├── s3fifo.rs       # S3-FIFO 页面淘汰 + 对比实验
│       └── task.rs         # 协作式任务调度
└── run.ps1                 # PowerShell 运行脚本
```

## 功能

### V1 — 基础内核
- BIOS 启动 → 64 位长模式
- 自写 COM1 串口驱动（38400 bps, 8N1）
- GDT + TSS（Double Fault 独立栈）
- IDT：breakpoint / double fault / page fault
- 物理内存检测（122 MiB）
- Bump allocator（2 MiB 内核堆）
- 环形缓冲区（256 条目，AtomicU64，无锁）
- 8 种事件类型日志
- 协作式任务调度（spawn / yield / exit）

### V2 — 内存管理
- 位图物理页框分配器（128 frames × 4 KiB）
- S3-FIFO 页面淘汰算法（SOSP 2023，S + M 双队列）
- 真实 x86_64 四级页表操作（OffsetPageTable）
- 缺页中断处理（自动分配 + S3-FIFO 淘汰 + 重建映射）
- FIFO vs S3-FIFO 对比实验（随机/热点/顺序扫描）

## 运行输出示例

```
╔══════════════════════════════════════════╗
║     Rust OS v0.1 — Lightweight Kernel   ║
╚══════════════════════════════════════════╝

[init] GDT loaded.
[init] IDT loaded.
[memory] Total usable: 122 MiB
[allocator] Heap: 2048 KiB
[frame] Pool: 128 frames, 512 KiB
[paging] PML4 ready, user area: 256 pages

=== S3-FIFO vs FIFO Comparison ===
[1] Random access (100 pages, 1000 accesses)
  FIFO:      900 / 1000 ( 90.0%)
  S3-FIFO:   900 / 1000 ( 90.0%)

[2] Hotspot (80% on 8 pages)
  FIFO:      915 / 1000 ( 91.5%)
  S3-FIFO:   915 / 1000 ( 91.5%)

[3] Sequential scan
  FIFO:        0 / 1000 (  0.0%)
  S3-FIFO:   448 / 1000 ( 44.8%)    ← S3-FIFO 碾压 FIFO

=== Page Table Test ===
[ptest] Write: 0xdeadbeefcafebabe → Read: 0xdeadbeefcafebabe ✓

=== System halted ===
```

## 环境

- **编译器**: Rust nightly-2026-05-01 (x86_64-unknown-none)
- **模拟器**: QEMU 11.0 (x86_64)
- **启动方式**: BIOS (bootloader crate v0.11)
- **核心依赖**: `x86_64 0.14`, `bootloader_api 0.11`, `heapless 0.8`, `spin 0.9`

## 参考

- [Writing an OS in Rust](https://os.phil-opp.com/) — Philipp Oppermann
- [S3-FIFO](https://dl.acm.org/doi/10.1145/3600006.3613147) — SOSP 2023
- [bootloader crate](https://github.com/rust-osdev/bootloader) — v0.11
