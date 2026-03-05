use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, TrayIconBuilder};

use gstreamer as gst;
use gstreamer::prelude::*;

use crate::config::StreamConfig;
use crate::pipeline;
use crate::virtual_display;

/// Generate a simple colored circle icon (16x16 RGBA).
fn make_circle_icon(r: u8, g: u8, b: u8) -> Icon {
    let size = 16u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let center = size as f32 / 2.0;
    let radius = center - 1.0;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();
            let idx = ((y * size + x) * 4) as usize;
            if dist <= radius {
                rgba[idx] = r;
                rgba[idx + 1] = g;
                rgba[idx + 2] = b;
                rgba[idx + 3] = 255;
            }
        }
    }

    Icon::from_rgba(rgba, size, size).expect("Failed to create icon")
}

pub struct TrayApp {
    config: StreamConfig,
    use_virtual_display: bool,
}

impl TrayApp {
    pub fn new(config: StreamConfig, use_virtual_display: bool) -> Self {
        Self {
            config,
            use_virtual_display,
        }
    }

    pub fn run(self) {
        // GStreamer must be initialized before we can build pipelines.
        gst::init().expect("[Tray] Failed to initialise GStreamer");
        println!("[Tray] GStreamer initialised");

        let event_loop = EventLoopBuilder::new().build();

        // Build tray menu.
        let menu = Menu::new();
        let item_start = MenuItem::new("Start Streaming", true, None);
        let item_stop = MenuItem::new("Stop Streaming", false, None);
        let item_kill = MenuItem::new("Kill Virtual Display", true, None);
        let item_settings = MenuItem::new(
            format!(
                "Settings: {}x{} @ {}fps → {}",
                self.config.width, self.config.height, self.config.fps, self.config.host,
            ),
            false,
            None,
        );
        let item_exit = MenuItem::new("Exit", true, None);

        menu.append(&item_start).unwrap();
        menu.append(&item_stop).unwrap();
        menu.append(&item_kill).unwrap();
        menu.append(&item_settings).unwrap();
        menu.append(&item_exit).unwrap();

        let icon_red = make_circle_icon(220, 50, 50);
        let icon_green = make_circle_icon(50, 200, 50);
        let icon_yellow = make_circle_icon(230, 200, 50);

        let _tray = TrayIconBuilder::new()
            .with_tooltip("Virtual Display Extender")
            .with_menu(Box::new(menu))
            .with_icon(icon_red.clone())
            .build()
            .expect("[Tray] Failed to create tray icon");

        // Shared state between event loop and streaming thread.
        let streaming = Arc::new(AtomicBool::new(false));
        let should_stop = Arc::new(AtomicBool::new(false));

        let id_start = item_start.id().clone();
        let id_stop = item_stop.id().clone();
        let id_kill = item_kill.id().clone();
        let id_exit = item_exit.id().clone();

        let menu_rx = MenuEvent::receiver().clone();

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::WaitUntil(
                std::time::Instant::now() + std::time::Duration::from_millis(100),
            );

            // Check for menu events.
            if let Ok(event) = menu_rx.try_recv() {
                if event.id == id_start {
                    if !streaming.load(Ordering::SeqCst) {
                        // Start streaming in a background thread.
                        let config = self.config.clone();
                        let streaming_clone = Arc::clone(&streaming);
                        let should_stop_clone = Arc::clone(&should_stop);
                        let use_vd = self.use_virtual_display;

                        should_stop.store(false, Ordering::SeqCst);
                        streaming.store(true, Ordering::SeqCst);

                        item_start.set_enabled(false);
                        item_stop.set_enabled(true);

                        // Update icon to yellow (starting).
                        _tray.set_icon(Some(icon_yellow.clone())).ok();

                        thread::spawn(move || {
                            run_streaming(config, use_vd, streaming_clone, should_stop_clone);
                        });

                        // Update icon to green after a short delay.
                        _tray.set_icon(Some(icon_green.clone())).ok();
                    }
                } else if event.id == id_stop {
                    should_stop.store(true, Ordering::SeqCst);
                    item_start.set_enabled(true);
                    item_stop.set_enabled(false);
                    _tray.set_icon(Some(icon_red.clone())).ok();
                } else if event.id == id_kill {
                    println!("[Tray] Killing virtual display ...");
                    virtual_display::destroy_virtual_monitor();
                    println!("[Tray] Virtual display killed");
                } else if event.id == id_exit {
                    should_stop.store(true, Ordering::SeqCst);
                    // Give the streaming thread a moment to clean up.
                    thread::sleep(std::time::Duration::from_millis(500));
                    *control_flow = ControlFlow::Exit;
                }
            }

            // Check if streaming thread finished on its own.
            if !streaming.load(Ordering::SeqCst) {
                item_start.set_enabled(true);
                item_stop.set_enabled(false);
            }

            match event {
                Event::NewEvents(StartCause::Init) => {
                    println!("[Tray] System tray icon active");
                }
                _ => {}
            }
        });
    }
}

/// Run the streaming pipeline in a background thread.
fn run_streaming(
    config: StreamConfig,
    use_virtual_display: bool,
    streaming: Arc<AtomicBool>,
    should_stop: Arc<AtomicBool>,
) {
    // Create or reuse virtual display if requested.
    let monitor_index = if use_virtual_display {
        match virtual_display::create_or_reuse_virtual_monitor(
            config.width,
            config.height,
            config.fps,
        ) {
            Ok(idx) => idx,
            Err(e) => {
                eprintln!("[Tray] Virtual display error: {e}");
                streaming.store(false, Ordering::SeqCst);
                return;
            }
        }
    } else {
        config.monitor_index
    };

    let mut config = config;
    config.monitor_index = monitor_index;

    let (pipeline, description) = match pipeline::build_pipeline(&config) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[Tray] Pipeline error: {e}");
            streaming.store(false, Ordering::SeqCst);
            return;
        }
    };

    println!("[Tray] Using: {description}");

    if let Err(e) = pipeline.set_state(gst::State::Playing) {
        eprintln!("[Tray] Failed to start pipeline: {e}");
        streaming.store(false, Ordering::SeqCst);
        return;
    }

    println!(
        "[Tray] Streaming monitor {monitor_index} to {}:{} ({}x{} @ {}fps, {} kbps)",
        config.host,
        config.port,
        config.width,
        config.height,
        config.fps,
        config.bitrate / 1000,
    );

    let bus = pipeline.bus().expect("[Tray] Pipeline has no bus");

    while !should_stop.load(Ordering::SeqCst) {
        if let Some(msg) = bus.timed_pop(gst::ClockTime::from_mseconds(100)) {
            use gst::MessageView;
            match msg.view() {
                MessageView::Eos(..) => {
                    println!("[Tray] End of stream");
                    break;
                }
                MessageView::Error(err) => {
                    eprintln!(
                        "[Tray] Error from {}: {}",
                        err.src()
                            .map(|s| s.path_string().to_string())
                            .unwrap_or_else(|| "unknown".into()),
                        err.error()
                    );
                    break;
                }
                _ => {}
            }
        }
    }

    println!("[Tray] Stopping pipeline ...");
    let _ = pipeline.set_state(gst::State::Null);

    if use_virtual_display {
        virtual_display::destroy_virtual_monitor();
    }

    streaming.store(false, Ordering::SeqCst);
    println!("[Tray] Pipeline stopped");
}
