//! Background USB worker.
//!
//! All libusb calls (and the possible blocking waits) run on a dedicated
//! thread. The UI and the worker communicate exclusively through channels, so
//! the egui event loop never blocks and no USB types need to be `Sync`.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use auroku::{run_task, scan_devices, AuRokkuError, DeviceConfig, DeviceSummary, LogSink, Task};
use eframe::egui;

/// Serializable form of `AuRokkuError` for the UI thread.
#[derive(Clone, Debug)]
pub struct ApiError {
    pub kind: String,
    pub message: String,
}

impl From<AuRokkuError> for ApiError {
    fn from(error: AuRokkuError) -> Self {
        Self {
            kind: error.kind().to_owned(),
            message: error.to_string(),
        }
    }
}

pub enum Request {
    Scan {
        vid: Option<u16>,
        pid: Option<u16>,
    },
    Run {
        config: DeviceConfig,
        task: Task,
        label: String,
    },
    Shutdown,
}

pub enum Event {
    ScanStarted,
    Scanned(Vec<DeviceSummary>),
    ScanFailed(String),
    Started {
        label: String,
    },
    Log(String),
    Finished {
        label: String,
        result: Result<serde_json::Value, ApiError>,
    },
}

/// Handle to the USB worker thread.
pub struct Worker {
    requests: Sender<Request>,
    events: Receiver<Event>,
    event_tx: Sender<Event>,
    ctx: egui::Context,
}

impl Worker {
    pub fn new(ctx: egui::Context) -> Self {
        let (request_tx, request_rx) = channel::<Request>();
        let (event_tx, event_rx) = channel::<Event>();

        let thread_events = event_tx.clone();
        let thread_ctx = ctx.clone();

        thread::Builder::new()
            .name("auroku-usb".to_owned())
            .stack_size(8 * 1024 * 1024)
            .spawn(move || {
                while let Ok(request) = request_rx.recv() {
                    match request {
                        Request::Scan { vid, pid } => {
                            let _ = thread_events.send(Event::ScanStarted);
                            thread_ctx.request_repaint();
                            match scan_devices(vid, pid) {
                                Ok(devices) => {
                                    let _ = thread_events.send(Event::Scanned(devices));
                                }
                                Err(error) => {
                                    let _ =
                                        thread_events.send(Event::ScanFailed(error.to_string()));
                                }
                            }
                        }
                        Request::Run {
                            config,
                            task,
                            label,
                        } => {
                            let _ = thread_events.send(Event::Started {
                                label: label.clone(),
                            });
                            thread_ctx.request_repaint();

                            let outcome =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    run_task(&config, &task)
                                }))
                                .map_err(|panic| ApiError {
                                    kind: "Panic".to_owned(),
                                    message: panic_message(panic),
                                })
                                .and_then(|result| result.map_err(ApiError::from));

                            let _ = thread_events.send(Event::Finished {
                                label,
                                result: outcome,
                            });
                        }
                        Request::Shutdown => break,
                    }
                    thread_ctx.request_repaint();
                }
            })
            .expect("failed to spawn USB worker thread");

        Self {
            requests: request_tx,
            events: event_rx,
            event_tx,
            ctx,
        }
    }

    /// Log sink that forwards backend diagnostics into the console screen.
    pub fn log_sink(&self) -> LogSink {
        let events = self.event_tx.clone();
        let ctx = self.ctx.clone();
        Arc::new(move |line: &str| {
            let _ = events.send(Event::Log(line.to_owned()));
            ctx.request_repaint();
        })
    }

    pub fn send(&self, request: Request) {
        let _ = self.requests.send(request);
    }

    pub fn try_recv(&self) -> Option<Event> {
        self.events.try_recv().ok()
    }

    /// Ask the UI to repaint periodically while an operation is running.
    pub fn keep_alive(&self) {
        self.ctx.request_repaint_after(Duration::from_millis(120));
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.requests.send(Request::Shutdown);
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "USB worker panicked".to_owned()
    }
}
