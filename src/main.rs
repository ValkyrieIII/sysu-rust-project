fn main() {
    let bios_path = env!("BIOS_IMAGE");

    // QEMU paths on Windows
    let qemu = if cfg!(windows) {
        r"C:\Program Files\qemu\qemu-system-x86_64.exe"
    } else {
        "qemu-system-x86_64"
    };

    let mut cmd = std::process::Command::new(qemu);
    cmd.arg("-drive")
        .arg(format!("format=raw,file={bios_path}"))
        .arg("-serial")
        .arg("stdio")            // redirect serial port to stdio
        .arg("-m")
        .arg("128M")             // 128 MiB RAM
        .arg("-no-reboot")       // don't reboot on triple fault
        .arg("-no-shutdown");    // don't quit on shutdown

    println!("Starting QEMU with BIOS image: {bios_path}");
    let status = cmd.status().expect("failed to run QEMU — is it installed?");
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
}
