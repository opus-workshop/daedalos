use daedalos_core::{Paths, daemon, process};

fn main() {
    println!("=== Process Checks ===");
    for name in &["loopd", "daedalos_mcp", "mcp", "Python"] {
        println!("{}: {}", name, process::is_running(name));
    }
    
    println!("\n=== Daemon Status ===");
    let paths = Paths::new();
    let daemons = daemon::check_all_daemons(&paths);
    for d in daemons {
        println!("{}: {}", d.display_name, d.status.as_str());
    }
}
