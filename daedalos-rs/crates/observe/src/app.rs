//! Application state and logic

use chrono::{DateTime, Local};
use daedalos_core::{daemon::DaemonInfo, Paths};

use crate::data;

/// Which panel is focused
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Panel {
    #[default]
    Daemons,
    Loops,
    Agents,
    Events,
}

impl Panel {
    pub fn next(self) -> Self {
        match self {
            Self::Daemons => Self::Loops,
            Self::Loops => Self::Agents,
            Self::Agents => Self::Events,
            Self::Events => Self::Daemons,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Daemons => Self::Events,
            Self::Loops => Self::Daemons,
            Self::Agents => Self::Loops,
            Self::Events => Self::Agents,
        }
    }
}

/// A loop's status
#[derive(Debug, Clone)]
pub struct LoopInfo {
    pub id: String,
    pub task: String,
    pub status: String,
    pub iteration: u32,
    pub duration: f64,
}

/// An agent's status
#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub slot: u32,
    pub name: String,
    pub template: String,
    pub status: String,
    pub uptime: f64,
}

/// An event in the log
#[derive(Debug, Clone)]
pub struct EventInfo {
    pub timestamp: DateTime<Local>,
    pub source: String,
    pub message: String,
}

/// Application state
pub struct App {
    pub paths: Paths,
    pub refresh_interval: f64,
    pub paused: bool,
    pub show_help: bool,
    pub focused_panel: Panel,
    pub scroll_offset: usize,

    // Data
    pub daemons: Vec<DaemonInfo>,
    pub loops: Vec<LoopInfo>,
    pub agents: Vec<AgentInfo>,
    pub events: Vec<EventInfo>,
}

impl App {
    pub fn new(refresh_interval: f64) -> Self {
        Self {
            paths: Paths::new(),
            refresh_interval,
            paused: false,
            show_help: false,
            focused_panel: Panel::default(),
            scroll_offset: 0,
            daemons: Vec::new(),
            loops: Vec::new(),
            agents: Vec::new(),
            events: vec![EventInfo {
                timestamp: Local::now(),
                source: "observe".to_string(),
                message: "Started".to_string(),
            }],
        }
    }

    pub fn refresh(&mut self) {
        self.daemons = daedalos_core::daemon::check_all_daemons(&self.paths);
        self.loops = data::fetch_loops(&self.paths);
        self.agents = data::fetch_agents(&self.paths);

        // Add refresh event
        self.events.push(EventInfo {
            timestamp: Local::now(),
            source: "observe".to_string(),
            message: "Refreshed".to_string(),
        });

        // Keep only last 100 events
        if self.events.len() > 100 {
            self.events.drain(0..self.events.len() - 100);
        }
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        let status = if self.paused { "Paused" } else { "Resumed" };
        self.events.push(EventInfo {
            timestamp: Local::now(),
            source: "observe".to_string(),
            message: status.to_string(),
        });
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    pub fn focus_panel(&mut self, panel: Panel) {
        self.focused_panel = panel;
        self.scroll_offset = 0;
    }

    pub fn next_panel(&mut self) {
        self.focused_panel = self.focused_panel.next();
        self.scroll_offset = 0;
    }

    pub fn prev_panel(&mut self) {
        self.focused_panel = self.focused_panel.prev();
        self.scroll_offset = 0;
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }
}
