use std::{
    env,
    path::PathBuf,
};

use gisor::run::launch_test;

#[test]
fn test() {
    println!("Running all tests with emulator :)");
    // print current directory
    let binary_path: PathBuf = env::current_dir().unwrap().join("out");
    println!("Binary path: {:?}", binary_path);
    for binary in binary_path.read_dir().unwrap() {
        let binary = binary.unwrap();
        if !binary.file_name().to_str().unwrap().ends_with(".run") {
            continue;
        }
        let binary = binary.path();
        let binary = binary.to_str().unwrap();
        println!(
            "=============================={}=============================",
            binary
        );
        let ptx = binary.replace(".run", ".ptx");
        let code = launch_test(binary, &ptx);
        assert_eq!(code, 0);
    }
}