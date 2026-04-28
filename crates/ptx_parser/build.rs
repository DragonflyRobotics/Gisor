use std::fs::read_dir;

fn main() {
    println!("cargo:rerun-if-changed=parser");
    let mut build = cc::Build::new();
    for entry in read_dir("parser").unwrap() {
        if entry.as_ref().unwrap().file_name().into_string().unwrap().ends_with(".c") {
            build.file(entry.unwrap().path());
        }
    }
    build.compile("ptx_parser");
}
