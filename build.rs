use std::path::PathBuf;

fn main() {
    // Locate the kernel binary artifact (from bindeps)
    let kernel_path = PathBuf::from(
        std::env::var_os("CARGO_BIN_FILE_KERNEL_kernel")
            .expect("kernel artifact not found — is [unstable] bindeps = true in .cargo/config.toml?"),
    );

    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());

    // --- BIOS disk image ---
    let bios_path = out_dir.join("bios.img");
    bootloader::BiosBoot::new(&kernel_path)
        .create_disk_image(&bios_path)
        .expect("failed to create BIOS disk image");
    println!("cargo:rustc-env=BIOS_IMAGE={}", bios_path.display());

    // --- UEFI disk image ---
    let uefi_path = out_dir.join("uefi.img");
    bootloader::UefiBoot::new(&kernel_path)
        .create_disk_image(&uefi_path)
        .expect("failed to create UEFI disk image");
    println!("cargo:rustc-env=UEFI_IMAGE={}", uefi_path.display());
}
