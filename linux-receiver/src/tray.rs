use std::sync::mpsc;

use ksni::menu::*;
use ksni::{self, Icon, ToolTip, Tray};

use crate::AppEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiverState {
    Idle,
    Running,
    Receiving,
}

impl ReceiverState {
    fn label(self) -> &'static str {
        match self {
            ReceiverState::Idle => "Idle",
            ReceiverState::Running => "Waiting for stream...",
            ReceiverState::Receiving => "Receiving",
        }
    }

    fn is_active(self) -> bool {
        matches!(self, ReceiverState::Running | ReceiverState::Receiving)
    }
}

pub struct ReceiverTray {
    pub state: ReceiverState,
    pub port: u16,
    pub fullscreen: bool,
    pub jitter_latency: u32,
    event_tx: mpsc::Sender<AppEvent>,
}

impl ReceiverTray {
    pub fn new(
        event_tx: mpsc::Sender<AppEvent>,
        port: u16,
        fullscreen: bool,
        jitter_latency: u32,
    ) -> Self {
        Self {
            state: ReceiverState::Idle,
            port,
            fullscreen,
            jitter_latency,
            event_tx,
        }
    }

    fn tray_icon_pixmap(&self) -> Vec<Icon> {
        let size = 22;
        let mut argb = vec![0u8; (size * size * 4) as usize];

        let (monitor_color, indicator_color) = match self.state {
            ReceiverState::Idle => (0xFF_6C7086u32, 0xFF_585B70u32),
            ReceiverState::Running => (0xFF_89B4FAu32, 0xFF_F9E2AFu32),
            ReceiverState::Receiving => (0xFF_89B4FAu32, 0xFF_A6E3A1u32),
        };

        // Draw monitor outline (rows 2-15, cols 2-19)
        for y in 2..16 {
            for x in 2..20 {
                if y == 2 || y == 15 || x == 2 || x == 19 {
                    set_pixel(&mut argb, size, x, y, monitor_color);
                }
            }
        }

        // Draw screen fill (rows 3-14, cols 3-18)
        for y in 3..15 {
            for x in 3..19 {
                set_pixel(&mut argb, size, x, y, 0xFF_1E1E2E);
            }
        }

        // Draw stand (rows 16-17, cols 8-13)
        for x in 8..14 {
            set_pixel(&mut argb, size, x, 16, monitor_color);
        }
        // Base (row 18, cols 6-15)
        for x in 6..16 {
            set_pixel(&mut argb, size, x, 18, monitor_color);
        }

        // Draw indicator dot in screen center
        for dy in 0i32..3 {
            for dx in 0i32..3 {
                let x = 10 + dx;
                let y = 7 + dy;
                set_pixel(&mut argb, size, x as u32, y as u32, indicator_color);
            }
        }

        vec![Icon {
            width: size as i32,
            height: size as i32,
            data: argb,
        }]
    }
}

fn set_pixel(buf: &mut [u8], stride: u32, x: u32, y: u32, argb: u32) {
    let offset = ((y * stride + x) * 4) as usize;
    if offset + 3 < buf.len() {
        buf[offset] = ((argb >> 24) & 0xFF) as u8; // A
        buf[offset + 1] = ((argb >> 16) & 0xFF) as u8; // R
        buf[offset + 2] = ((argb >> 8) & 0xFF) as u8; // G
        buf[offset + 3] = (argb & 0xFF) as u8; // B
    }
}

impl Tray for ReceiverTray {
    fn id(&self) -> String {
        "virtual-display-receiver".into()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        self.tray_icon_pixmap()
    }

    fn title(&self) -> String {
        "Virtual Display Receiver".into()
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "Virtual Display Receiver".into(),
            description: format!("Status: {} | Port: {}", self.state.label(), self.port),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let status_label = match self.state {
            ReceiverState::Idle => "Status: Idle".into(),
            ReceiverState::Running => "Status: Waiting for stream...".into(),
            ReceiverState::Receiving => "Status: Receiving".into(),
        };

        let start_stop_label = if self.state.is_active() {
            "Stop Receiver"
        } else {
            "Start Receiver"
        };

        vec![
            // Title
            StandardItem {
                label: "Virtual Display Receiver".into(),
                enabled: false,
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            // Status
            StandardItem {
                label: status_label,
                enabled: false,
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            // Start/Stop
            StandardItem {
                label: start_stop_label.into(),
                icon_name: if self.state.is_active() {
                    "media-playback-stop".into()
                } else {
                    "media-playback-start".into()
                },
                activate: Box::new(|tray: &mut Self| {
                    tray.event_tx.send(AppEvent::StartStop).ok();
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            // Fullscreen toggle
            CheckmarkItem {
                label: "Fullscreen".into(),
                checked: self.fullscreen,
                enabled: !self.state.is_active(),
                activate: Box::new(|tray: &mut Self| {
                    tray.fullscreen = !tray.fullscreen;
                    tray.event_tx.send(AppEvent::FullscreenToggled).ok();
                }),
                ..Default::default()
            }
            .into(),
            // Port info
            StandardItem {
                label: format!("Port: {}", self.port),
                enabled: false,
                ..Default::default()
            }
            .into(),
            // Jitter info
            StandardItem {
                label: format!("Jitter Buffer: {}ms", self.jitter_latency),
                enabled: false,
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            // Quit
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|tray: &mut Self| {
                    tray.event_tx.send(AppEvent::Quit).ok();
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}
