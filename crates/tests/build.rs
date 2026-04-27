use cc;
use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    // Determine the build profile (debug/release)
    let profile = env::var("PROFILE").unwrap();
    let target_path: PathBuf = env::current_dir().unwrap()
        .join("..")
        .join("..")
        .join("target")
        .join(&profile);

    // Hunt CUDA include/lib paths
    let cuda_path = env::var("CUDA_HOME")
        .or_else(|_| env::var("CUDA_PATH"))
        .unwrap();

    let cuda_include = format!("{}/include", cuda_path);
    let cuda_lib64 = format!("{}/lib64", cuda_path);

    // Tell Cargo where to find native libraries if we link cudart
    println!("cargo:rustc-link-search=native={}", cuda_lib64);
    println!("cargo:rustc-link-search=native={}", target_path.to_str().unwrap());

    // Link with runtime
    println!("cargo:rustc-link-lib=dylib=cudart");

    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", cuda_lib64);
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", target_path.to_str().unwrap());

    // Include headers for safe measure
    println!("cargo:include={}", cuda_include);

    // Rerun build.rs if these paths change
    println!("cargo:rerun-if-changed={}", cuda_include);
    println!("cargo:rerun-if-changed={}", cuda_lib64);
    println!("cargo:rerun-if-changed={}", target_path.to_str().unwrap());
    
    let outdir = format!("out/");
    for entry in fs::read_dir(outdir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() {
            fs::remove_file(&path).unwrap();
        }
    }

    let test_files = std::fs::read_dir("tests").unwrap();
    for entry in test_files {
        let path = entry.unwrap().path();
        println!("cargo:rerun-if-changed={}", path.display());
        let mut status = Command::new("nvcc")
            .arg("-c")
            .arg(&path)
            .arg("-o")
            .arg(format!(
                "out/{}.o",
                path.file_stem().unwrap().to_str().unwrap()
            ))
            .arg("--compiler-options")
            .arg("-fPIC")
            .status()
            .unwrap();

        if !status.success() {
            panic!("Compilation failed for {}", path.display());
        }
        status = Command::new("nvcc")
                    .arg("-ptx")
                    .arg("-O0")
                    .arg(&path)
                    .arg("-o")
                    .arg(format!(
                        "out/{}.ptx",
                        path.file_stem().unwrap().to_str().unwrap()
                    ))
                    .arg("--compiler-options")
                    .arg("-fPIC")
                    .status()
                    .unwrap();
        
                if !status.success() {
                    panic!("Compilation failed for {}", path.display());
                }
        status = Command::new("g++")
            .arg(format!(
                "out/{}.o",
                path.file_stem().unwrap().to_str().unwrap()
            ))
            .arg("-L")
            .arg(target_path.to_str().unwrap())
            .arg("-L")
            .arg(cuda_lib64.as_str())
            .arg("-lgisor")
            // .arg("-lcuda")
            // .arg("-lcudart")
            .arg("-o")
            .arg(format!(
                "out/{}.run",
                path.file_stem().unwrap().to_str().unwrap()
            ))
            .status()
            .unwrap();
        fs::remove_file(format!("out/{}.o", path.file_stem().unwrap().to_str().unwrap())).ok();
        if !status.success() {
            panic!("Compilation failed for {}", path.display());
        }
    }
}
