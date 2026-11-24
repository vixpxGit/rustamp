use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

include!("src/logo_render.rs");

fn encode_ico_entry(size: u32) -> Vec<u8> {
    let rgba = render_winampr_rgba(size);
    let xor_bytes = (size * size * 4) as usize;
    let and_row_bytes = (((size + 31) / 32) * 4) as usize;
    let and_bytes = and_row_bytes * size as usize;

    let mut data = Vec::with_capacity(40 + xor_bytes + and_bytes);
    // BITMAPINFOHEADER (40 bytes)
    data.extend_from_slice(&40u32.to_le_bytes()); // biSize
    data.extend_from_slice(&(size as i32).to_le_bytes()); // biWidth
    data.extend_from_slice(&((size * 2) as i32).to_le_bytes()); // biHeight (doubled for XOR + AND)
    data.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    data.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    data.extend_from_slice(&0u32.to_le_bytes()); // biCompression (BI_RGB)
    data.extend_from_slice(&(xor_bytes as u32).to_le_bytes()); // biSizeImage
    data.extend_from_slice(&0i32.to_le_bytes()); // biXPelsPerMeter
    data.extend_from_slice(&0i32.to_le_bytes()); // biYPelsPerMeter
    data.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
    data.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant

    // Bottom-up BGRA pixels
    for y in (0..size).rev() {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;
            let r = rgba[idx];
            let g = rgba[idx + 1];
            let b = rgba[idx + 2];
            let a = rgba[idx + 3];
            data.extend_from_slice(&[b, g, r, a]);
        }
    }

    // 1-bit AND mask (all 0s; 32-bit alpha channel controls transparency)
    data.resize(data.len() + and_bytes, 0u8);
    data
}

fn write_multi_ico(path: &PathBuf, sizes: &[u32]) -> std::io::Result<()> {
    let entries: Vec<(u32, Vec<u8>)> = sizes.iter().map(|&s| (s, encode_ico_entry(s))).collect();
    let mut file = File::create(path)?;

    // ICONDIR (6 bytes)
    file.write_all(&0u16.to_le_bytes())?; // idReserved
    file.write_all(&1u16.to_le_bytes())?; // idType (1 = ICON)
    file.write_all(&(entries.len() as u16).to_le_bytes())?; // idCount

    let mut offset = 6u32 + (entries.len() as u32) * 16u32;
    for (size, data) in &entries {
        let dim = if *size >= 256 { 0u8 } else { *size as u8 };
        file.write_all(&[dim, dim, 0, 0])?; // width, height, colorCount, reserved
        file.write_all(&1u16.to_le_bytes())?; // planes
        file.write_all(&32u16.to_le_bytes())?; // bitCount
        file.write_all(&(data.len() as u32).to_le_bytes())?; // bytesInRes
        file.write_all(&offset.to_le_bytes())?; // imageOffset
        offset += data.len() as u32;
    }

    for (_, data) in &entries {
        file.write_all(data)?;
    }

    Ok(())
}

fn main() {
    println!("cargo:rerun-if-changed=src/logo_render.rs");
    println!("cargo:rerun-if-changed=build.rs");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let ico_path = out_dir.join("winamp.ico");
    write_multi_ico(&ico_path, &[16, 32, 48, 64]).expect("Failed to write winamp.ico");

    let rc_path = out_dir.join("app.rc");
    let ico_escaped = ico_path.to_string_lossy().replace('\\', "/");
    fs::write(&rc_path, format!("1 ICON \"{}\"\n", ico_escaped)).expect("Failed to write app.rc");

    let res_obj = out_dir.join("app_res.o");
    let windres_candidates = ["x86_64-w64-mingw32-windres", "windres"];
    for tool in windres_candidates {
        if let Ok(status) = Command::new(tool)
            .args([
                rc_path.to_str().unwrap(),
                "-O",
                "coff",
                "-o",
                res_obj.to_str().unwrap(),
            ])
            .status()
        {
            if status.success() {
                println!("cargo:rustc-link-arg={}", res_obj.display());
                return;
            }
        }
    }

    println!("cargo:warning=windres not found; Windows .exe resource icon was not embedded");
}
