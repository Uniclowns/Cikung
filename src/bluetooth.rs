//! Read-only connection snapshots; pairing and radio state are never changed.
use std::{collections::BTreeMap, sync::mpsc, time::Duration};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Device {
    #[serde(skip_serializing)]
    pub id: String,
    pub name: String,
}

pub struct Change {
    pub device: Device,
    pub connected: bool,
}

#[derive(Default)]
struct Connections {
    current: Option<BTreeMap<String, Device>>,
    candidate: Option<BTreeMap<String, Device>>,
}
impl Connections {
    fn update(&mut self, devices: Vec<Device>) -> Vec<Change> {
        let next: BTreeMap<_, _> = devices.into_iter().map(|d| (d.id.clone(), d)).collect();
        let Some(current) = &self.current else {
            self.current = Some(next);
            return vec![];
        };
        if *current == next {
            self.candidate = None;
            return vec![];
        }
        // Two matching polls avoid announcements for a brief Bluetooth handover.
        if self.candidate.as_ref() != Some(&next) {
            self.candidate = Some(next);
            return vec![];
        }
        let mut changes = Vec::new();
        for (id, device) in &next {
            if !current.contains_key(id) {
                changes.push(Change {
                    device: device.clone(),
                    connected: true,
                });
            }
        }
        for (id, device) in current {
            if !next.contains_key(id) {
                changes.push(Change {
                    device: device.clone(),
                    connected: false,
                });
            }
        }
        self.current = Some(next);
        self.candidate = None;
        changes
    }
}

pub struct Monitor {
    _stop: mpsc::Sender<()>,
    rx: mpsc::Receiver<Result<Vec<Device>, String>>,
    connections: Connections,
    pub error: Option<String>,
}
impl Monitor {
    pub fn start(ctx: eframe::egui::Context) -> Self {
        let (stop, commands) = mpsc::channel::<()>();
        let (tx, rx) = mpsc::sync_channel(1);
        let started = std::thread::Builder::new()
            .name("Cikung Bluetooth".into())
            .spawn(move || {
                loop {
                    if matches!(
                        tx.try_send(probe()),
                        Err(mpsc::TrySendError::Disconnected(_))
                    ) {
                        break;
                    }
                    ctx.request_repaint();
                    if commands.recv_timeout(Duration::from_secs(3))
                        != Err(mpsc::RecvTimeoutError::Timeout)
                    {
                        break;
                    }
                }
            });
        Self {
            _stop: stop,
            rx,
            connections: Connections::default(),
            error: started
                .err()
                .map(|e| format!("Could not start Bluetooth monitoring: {e}")),
        }
    }
    pub fn poll(&mut self) -> Vec<Change> {
        let mut changes = Vec::new();
        for snapshot in self.rx.try_iter() {
            match snapshot {
                Ok(devices) => {
                    self.error = None;
                    changes.extend(self.connections.update(devices));
                }
                Err(error) => {
                    self.error = Some(error);
                    self.connections.candidate = None;
                }
            }
        }
        changes
    }
    pub fn devices(&self) -> impl Iterator<Item = &Device> {
        self.connections.current.iter().flat_map(|d| d.values())
    }
    pub fn ready(&self) -> bool {
        self.connections.current.is_some()
    }
}

#[cfg(target_os = "windows")]
pub fn probe() -> Result<Vec<Device>, String> {
    use windows::{
        Devices::{
            Bluetooth::{BluetoothConnectionStatus, BluetoothDevice, BluetoothLEDevice},
            Enumeration::{DeviceInformation, DeviceInformationKind},
        },
        Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
    };
    unsafe { RoInitialize(RO_INIT_MULTITHREADED) }
        .map_err(|e| format!("Could not initialize Bluetooth access: {e}"))?;
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe { RoUninitialize() };
        }
    }
    let _apartment = Apartment;
    let query = || -> windows::core::Result<Vec<Device>> {
        let selectors = [
            BluetoothDevice::GetDeviceSelectorFromConnectionStatus(
                BluetoothConnectionStatus::Connected,
            )?,
            BluetoothLEDevice::GetDeviceSelectorFromConnectionStatus(
                BluetoothConnectionStatus::Connected,
            )?,
        ];
        let mut devices = BTreeMap::new();
        for selector in selectors {
            let operation =
                DeviceInformation::FindAllAsyncWithKindAqsFilterAndAdditionalProperties(
                    &selector,
                    None,
                    DeviceInformationKind::AssociationEndpoint,
                )?;
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while operation.Status()?.0 == 0 {
                // WinRT AsyncStatus::Started.
                if std::time::Instant::now() >= deadline {
                    let _ = operation.Cancel();
                    return Err(windows::core::Error::new(
                        windows::core::HRESULT(0x800705B4u32 as i32),
                        "Bluetooth enumeration timed out",
                    ));
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            let found = operation.GetResults()?;
            for i in 0..found.Size()? {
                let info = found.GetAt(i)?;
                let id = info.Id()?.to_string();
                let name = info.Name()?.to_string();
                devices.insert(
                    id.clone(),
                    Device {
                        id,
                        name: if name.trim().is_empty() {
                            "Bluetooth device".into()
                        } else {
                            name.chars().filter(|c| !c.is_control()).take(100).collect()
                        },
                    },
                );
            }
        }
        Ok(devices.into_values().collect())
    };
    query().map_err(|e| format!("Bluetooth status is unavailable: {e}"))
}

/// Paired Mac devices that are connected now, through IOBluetooth (classic and paired LE
/// devices such as AirPods, keyboards and mice).
#[cfg(target_os = "macos")]
pub fn probe() -> Result<Vec<Device>, String> {
    use objc2::{
        msg_send, msg_send_id,
        rc::Retained,
        runtime::{AnyClass, AnyObject},
    };
    use objc2_foundation::{NSArray, NSString};
    #[link(name = "IOBluetooth", kind = "framework")]
    unsafe extern "C" {}
    // Without the Bluetooth usage description macOS ends the app on first use, so a bare
    // binary (not the built Cikung.app) never asks.
    if !crate::native::bundle_declares("NSBluetoothAlwaysUsageDescription") {
        return Err("Bluetooth notifications need Cikung.app from the Mac build.".into());
    }
    let class =
        AnyClass::get("IOBluetoothDevice").ok_or("Bluetooth is unavailable on this Mac.")?;
    objc2::rc::autoreleasepool(|_| {
        let paired: Option<Retained<NSArray<AnyObject>>> =
            unsafe { msg_send_id![class, pairedDevices] };
        let mut devices = BTreeMap::new();
        for device in paired.iter().flat_map(|list| list.iter()) {
            let connected: bool = unsafe { msg_send![device, isConnected] };
            if !connected {
                continue;
            }
            let text = |name: Option<Retained<NSString>>| name.map(|s| s.to_string());
            let id = text(unsafe { msg_send_id![device, addressString] }).unwrap_or_default();
            let name = text(unsafe { msg_send_id![device, name] }).unwrap_or_default();
            devices.insert(
                id.clone(),
                Device {
                    id,
                    name: if name.trim().is_empty() {
                        "Bluetooth device".into()
                    } else {
                        name.chars().filter(|c| !c.is_control()).take(100).collect()
                    },
                },
            );
        }
        Ok(devices.into_values().collect())
    })
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn probe() -> Result<Vec<Device>, String> {
    Err("Bluetooth notifications currently require Windows or macOS.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn device(id: &str) -> Device {
        Device {
            id: id.into(),
            name: "Headset".into(),
        }
    }
    #[test]
    fn startup_is_silent_and_confirmed_connections_notify_once() {
        let mut state = Connections::default();
        assert!(state.update(vec![device("a")]).is_empty());
        assert!(state.update(vec![device("a"), device("b")]).is_empty());
        let changes = state.update(vec![device("a"), device("b")]);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].device.id, "b");
        assert!(changes[0].connected);
        assert!(state.update(vec![device("a"), device("b")]).is_empty());
    }
    #[test]
    fn brief_disconnect_is_ignored_and_same_named_devices_stay_distinct() {
        let mut state = Connections::default();
        state.update(vec![device("a"), device("b")]);
        assert!(state.update(vec![]).is_empty());
        assert!(state.update(vec![device("a"), device("b")]).is_empty());
        assert!(state.update(vec![device("b")]).is_empty());
        let changes = state.update(vec![device("b")]);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].device.id, "a");
        assert!(!changes[0].connected);
    }
    #[test]
    fn failed_poll_keeps_connections_and_resets_disconnect_confirmation() {
        let (tx, rx) = mpsc::sync_channel(1);
        let (stop, _commands) = mpsc::channel();
        let mut monitor = Monitor {
            _stop: stop,
            rx,
            connections: Connections::default(),
            error: None,
        };
        tx.send(Ok(vec![device("a")])).unwrap();
        monitor.poll();
        tx.send(Ok(vec![])).unwrap();
        assert!(monitor.poll().is_empty());
        tx.send(Err("Unavailable".into())).unwrap();
        assert!(monitor.poll().is_empty());
        assert_eq!(monitor.devices().count(), 1);
        assert!(monitor.error.is_some());
        tx.send(Ok(vec![])).unwrap();
        assert!(monitor.poll().is_empty());
        tx.send(Ok(vec![])).unwrap();
        assert_eq!(monitor.poll().len(), 1);
    }
}
