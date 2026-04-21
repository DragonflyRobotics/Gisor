use std::env;

use gisor::run::launch_debug;

fn main() {
    let args: Vec<String> = env::args().collect();
    assert_eq!(args.len(), 3);
    let run_file = &args[1];
    let ptx_file = &args[2];
    launch_debug(run_file, ptx_file);
}
