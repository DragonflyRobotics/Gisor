use std::ops::Range;

pub fn bytes_to_intarr(bytes: Vec<u8>) -> Vec<i32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

// https://users.rust-lang.org/t/what-is-the-idiomatic-way-to-make-a-cartesian-product-iterator/37372/6
pub fn triple_zip(x: Range<u32>, y: Range<u32>, z: Range<u32>) -> Vec<(u32, u32, u32)> {
    x.flat_map(|x| {
        y.clone().flat_map({
            let zends = &z;
            move |y| zends.clone().map(move |z| (x, y, z))
        })
    })
    .collect::<Vec<_>>()
}
