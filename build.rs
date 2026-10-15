// Generates PEXT-indexed rook/bishop attack tables as a little-endian binary blob.
use std::env;
use std::fs;
use std::path::Path;

fn ray_attacks(sq: i32, occ: u64, dirs: &[(i32, i32)]) -> u64 {
    let (x0, y0) = (sq & 7, sq >> 3);
    let mut out = 0u64;
    for &(dx, dy) in dirs {
        let (mut x, mut y) = (x0 + dx, y0 + dy);
        while (0..8).contains(&x) && (0..8).contains(&y) {
            let b = 1u64 << (x + 8 * y);
            out |= b;
            if occ & b != 0 {
                break;
            }
            x += dx;
            y += dy;
        }
    }
    out
}

fn relevant_mask(sq: i32, dirs: &[(i32, i32)]) -> u64 {
    let (x0, y0) = (sq & 7, sq >> 3);
    let mut out = 0u64;
    for &(dx, dy) in dirs {
        let (mut x, mut y) = (x0 + dx, y0 + dy);
        // Edge squares never block anything beyond themselves.
        while (0..8).contains(&(x + dx)) && (0..8).contains(&(y + dy)) {
            out |= 1u64 << (x + 8 * y);
            x += dx;
            y += dy;
        }
    }
    out
}

fn pdep(mut src: u64, mut mask: u64) -> u64 {
    let mut out = 0u64;
    while mask != 0 {
        let low = mask & mask.wrapping_neg();
        if src & 1 != 0 {
            out |= low;
        }
        src >>= 1;
        mask &= mask - 1;
    }
    out
}

fn main() {
    let rook_dirs = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    let bishop_dirs = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
    let mut blob: Vec<u64> = Vec::new();
    // Layout: rook masks[64], rook offsets[64], bishop masks[64], bishop offsets[64], table...
    let mut header = vec![0u64; 256];
    let mut table: Vec<u64> = Vec::new();
    for (k, dirs) in [&rook_dirs[..], &bishop_dirs[..]].into_iter().enumerate() {
        for sq in 0..64 {
            let mask = relevant_mask(sq, dirs);
            header[k * 128 + sq as usize] = mask;
            header[k * 128 + 64 + sq as usize] = table.len() as u64;
            let n = mask.count_ones();
            for idx in 0..(1u64 << n) {
                let occ = pdep(idx, mask);
                table.push(ray_attacks(sq, occ, dirs));
            }
        }
    }
    blob.extend_from_slice(&header);
    blob.extend_from_slice(&table);
    let bytes: Vec<u8> = blob.iter().flat_map(|w| w.to_le_bytes()).collect();
    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("sliders.bin");
    fs::write(out, bytes).unwrap();
    println!("cargo:rerun-if-changed=build.rs");
}
