//! Serial port driver using raw x86 I/O port access (COM1 at 0x3F8).
//! Avoids external crate dependencies — just uses `x86_64::instructions::port::Port`.

use x86_64::instructions::port::{Port, PortWriteOnly, PortReadOnly};
use spin::Mutex;
use core::fmt;

const COM1_BASE: u16 = 0x3F8;

pub struct SerialPort {
    data: Port<u8>,              // base + 0 (DLAB=0)
    interrupt_enable: PortWriteOnly<u8>, // base + 1 (DLAB=0)
    fifo_control: PortWriteOnly<u8>,     // base + 2
    line_control: PortWriteOnly<u8>,     // base + 3
    modem_control: PortWriteOnly<u8>,    // base + 4
    line_status: PortReadOnly<u8>,       // base + 5
}

impl SerialPort {
    pub const unsafe fn new(base: u16) -> Self {
        SerialPort {
            data: Port::new(base),
            interrupt_enable: PortWriteOnly::new(base + 1),
            fifo_control: PortWriteOnly::new(base + 2),
            line_control: PortWriteOnly::new(base + 3),
            modem_control: PortWriteOnly::new(base + 4),
            line_status: PortReadOnly::new(base + 5),
        }
    }

    /// Initialize the serial port: 8N1, 38400 baud, FIFO enabled
    pub fn init(&mut self) {
        unsafe {
            // Disable interrupts
            self.interrupt_enable.write(0x00);

            // Set DLAB to configure baud rate
            self.line_control.write(0x80);
            // Divisor = 115200 / 38400 = 3
            self.data.write(0x03);       // low byte
            self.interrupt_enable.write(0x00); // high byte

            // 8 data bits, 1 stop bit, no parity, clear DLAB
            self.line_control.write(0x03);

            // Enable FIFO, clear Tx/Rx FIFOs, 14-byte trigger
            self.fifo_control.write(0xC7);

            // RTS/DSR set (ready to transmit)
            self.modem_control.write(0x0B);
        }
    }

    /// Check if the transmit buffer is empty
    pub fn is_tx_empty(&mut self) -> bool {
        unsafe { self.line_status.read() & 0x20 != 0 }
    }

    /// Write a single byte
    pub fn write_byte(&mut self, byte: u8) {
        while !self.is_tx_empty() {
            core::hint::spin_loop();
        }
        unsafe { self.data.write(byte); }
    }
}

impl core::fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.bytes() {
            self.write_byte(byte);
        }
        Ok(())
    }
}

static SERIAL: Mutex<SerialPort> = Mutex::new(unsafe { SerialPort::new(COM1_BASE) });

/// Initialize the serial port
pub fn init() {
    SERIAL.lock().init();
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    x86_64::instructions::interrupts::without_interrupts(|| {
        SERIAL.lock().write_fmt(args).ok();
    });
}

/// Print to serial console (like `print!`)
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*))
    };
}

/// Print to serial console with newline (like `println!`)
#[macro_export]
macro_rules! serial_println {
    () => { $crate::serial::_print(format_args!("\n")) };
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*));
        $crate::serial::_print(format_args!("\n"));
    };
}
