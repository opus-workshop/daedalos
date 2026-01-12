//! Process management utilities

use sysinfo::System;
use std::process::Command;

/// Check if a process with the given name is running
pub fn is_running(name: &str) -> bool {
    // First try sysinfo (works well on Linux)
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let found_sysinfo = sys.processes().values().any(|p| {
        p.name().to_string_lossy().contains(name)
            || p.cmd().iter().any(|arg| arg.to_string_lossy().contains(name))
    });

    if found_sysinfo {
        return true;
    }

    // Fallback: use pgrep -f on macOS/Unix (checks full command line)
    #[cfg(unix)]
    {
        if let Ok(output) = Command::new("pgrep").args(["-f", name]).output() {
            return output.status.success();
        }
    }

    false
}

/// Get PIDs of processes matching a name
pub fn find_pids(name: &str) -> Vec<u32> {
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let mut pids: Vec<u32> = sys.processes()
        .iter()
        .filter(|(_, p)| {
            p.name().to_string_lossy().contains(name)
                || p.cmd().iter().any(|arg| arg.to_string_lossy().contains(name))
        })
        .map(|(pid, _)| pid.as_u32())
        .collect();

    // Fallback: use pgrep on Unix
    #[cfg(unix)]
    if pids.is_empty() {
        if let Ok(output) = Command::new("pgrep").args(["-f", name]).output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if let Ok(pid) = line.trim().parse::<u32>() {
                        pids.push(pid);
                    }
                }
            }
        }
    }

    pids
}
