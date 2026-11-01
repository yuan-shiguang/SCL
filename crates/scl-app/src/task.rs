//! Bridge between the blocking engine threads and the GPUI async world.

use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;

use futures::channel::mpsc::{unbounded, UnboundedReceiver};
use scl_core::download::TaskEvent;

pub struct EventBridge {
    pub sender: Sender<TaskEvent>,
    pub receiver: UnboundedReceiver<TaskEvent>,
}

/// The engine talks `std::sync::mpsc`; the UI awaits a futures channel.
pub fn bridge() -> EventBridge {
    let (std_sender, std_receiver) = channel::<TaskEvent>();
    let (fut_sender, fut_receiver) = unbounded::<TaskEvent>();
    std::thread::spawn(move || {
        while let Ok(event) = std_receiver.recv() {
            if fut_sender.unbounded_send(event).is_err() {
                break;
            }
        }
    });
    EventBridge {
        sender: std_sender,
        receiver: fut_receiver,
    }
}

pub fn cancel_token() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

/// Human readable byte size, e.g. `1.4 GB`.
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Formats a duration in seconds as `2 小时 13 分`.
pub fn format_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    if hours > 0 {
        format!("{hours} 小时 {minutes} 分")
    } else if minutes > 0 {
        format!("{minutes} 分钟")
    } else {
        format!("{seconds} 秒")
    }
}
