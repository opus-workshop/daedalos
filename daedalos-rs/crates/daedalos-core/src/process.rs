//! Process management utilities

use sysinfo::System;

/// Check if a process with the given name is running
pub fn is_running(name: &str) -> bool {
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    sys.processes().values().any(|p| {
        p.name().to_string_lossy().contains(name)
            || p.cmd().iter().any(|arg| arg.to_string_lossy().contains(name))
    })
}

/// Get PIDs of processes matching a name
pub fn find_pids(name: &str) -> Vec<u32> {
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    sys.processes()
        .iter()
        .filter(|(_, p)| {
            p.name().to_string_lossy().contains(name)
                || p.cmd().iter().any(|arg| arg.to_string_lossy().contains(name))
        })
        .map(|(pid, _)| pid.as_u32())
        .collect()
}
