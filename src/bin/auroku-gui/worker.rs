//! Background USB worker.
//!
//! All libusb calls (and the possible blocking waits) run on a dedicated
//! thread. The UI and the worker communicate exclusively through channels, so
//! the egui event loop never blocks and no USB types need to be `Sync`.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use auroku::{
    driver, run_task, scan_devices, scan_devices_quick, AuRokkuError, DeviceConfig, DeviceSummary,
    LogSink, Task,
};
use eframe::egui;

use crate::permissions::grant_selected_device;

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
    GrantAccess(DeviceSummary),
    Run {
        config: DeviceConfig,
        task: Task,
        label: String,
    },
    /// Install the WinUSB driver for the device (Windows only).
    InstallDriver {
        vid: u16,
        pid: u16,
        restore_on_exit: bool,
    },
    /// Restore the original driver (Windows only).
    RestoreDriver {
        vid: u16,
        pid: u16,
    },
    /// Query the current function driver service of the device (Windows only).
    DriverStatus {
        vid: u16,
        pid: u16,
    },
    /// Lightweight USB presence poll (no PnP fallback).
    PollDevices,
    /// Block new Ports-class driver installs so usbser cannot take over.
    BlockUsbSerInstall,
    /// Restore the device installation policy saved by the block action.
    RestoreUsbSerPolicy,
    Shutdown,
}

pub enum Event {
    ScanStarted,
    Scanned(Vec<DeviceSummary>),
    PolledDevices(Vec<DeviceSummary>),
    PollFailed,
    ScanFailed(String),
    AccessStarted(DeviceSummary),
    AccessGranted {
        device: DeviceSummary,
        acl_state: String,
    },
    AccessFailed {
        device: DeviceSummary,
        message: String,
    },
    Started {
        label: String,
    },
    Log(String),
    Finished {
        label: String,
        result: Result<serde_json::Value, ApiError>,
    },
    /// Outcome of an install/restore request.
    Driver {
        operation: &'static str,
        vid: u16,
        pid: u16,
        result: Result<(), String>,
    },
    /// Current function driver service of the device (Windows only).
    DriverStatus {
        vid: u16,
        pid: u16,
        service: Option<String>,
    },
    /// Outcome of a usbser.sys service action.
    UsbSerService {
        operation: &'static str,
        result: Result<(), String>,
    },
}

/// Handle to the USB worker thread.
pub struct Worker {
    requests: Sender<Request>,
    events: Receiver<Event>,
    event_tx: Sender<Event>,
    ctx: egui::Context,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    pub fn new(ctx: egui::Context) -> Self {
        let (request_tx, request_rx) = channel::<Request>();
        let (event_tx, event_rx) = channel::<Event>();

        let thread_events = event_tx.clone();
        let thread_ctx = ctx.clone();

        let worker_thread = thread::Builder::new()
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
                        Request::GrantAccess(device) => {
                            let _ = thread_events.send(Event::AccessStarted(device.clone()));
                            thread_ctx.request_repaint();
                            let event = match grant_selected_device(&device) {
                                Ok(acl_state) => Event::AccessGranted { device, acl_state },
                                Err(message) => Event::AccessFailed { device, message },
                            };
                            let _ = thread_events.send(event);
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
                        Request::InstallDriver {
                            vid,
                            pid,
                            restore_on_exit,
                        } => {
                            let result =
                                driver::install(vid, pid, restore_on_exit, None, false).map(|_| ());
                            let _ = thread_events.send(Event::Driver {
                                operation: "install",
                                vid,
                                pid,
                                result,
                            });
                        }
                        Request::RestoreDriver { vid, pid } => {
                            let result = driver::restore(vid, pid, None, false);
                            let _ = thread_events.send(Event::Driver {
                                operation: "restore",
                                vid,
                                pid,
                                result,
                            });
                        }
                        Request::DriverStatus { vid, pid } => {
                            let service = driver::driver_service(vid, pid);
                            let _ = thread_events.send(Event::DriverStatus { vid, pid, service });
                        }
                        Request::PollDevices => match scan_devices_quick(None, None) {
                            Ok(devices) => {
                                let _ = thread_events.send(Event::PolledDevices(devices));
                            }
                            Err(_) => {
                                let _ = thread_events.send(Event::PollFailed);
                            }
                        },
                        Request::BlockUsbSerInstall => {
                            let result = driver::block_usbser_install(None, false);
                            let _ = thread_events.send(Event::UsbSerService {
                                operation: "block",
                                result,
                            });
                        }
                        Request::RestoreUsbSerPolicy => {
                            let result = driver::restore_usbser_policy(None, false);
                            let _ = thread_events.send(Event::UsbSerService {
                                operation: "restore-policy",
                                result,
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
            thread: Some(worker_thread),
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

    pub fn shutdown_and_wait(&mut self) {
        let _ = self.requests.send(Request::Shutdown);
        if let Some(worker_thread) = self.thread.take() {
            if worker_thread.join().is_err() {
                crate::diag::log("USB worker thread panicked during shutdown");
            }
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.shutdown_and_wait();
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
