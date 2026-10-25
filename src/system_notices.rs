//! Battery, internet, Caps Lock and volume heads-ups in the notch, as Nusa 1.0 shows them.
//! Only changes are announced, never the state Cikung starts in. Windows only for now;
//! other platforms read nothing.
use std::{sync::mpsc, time::Duration};

pub struct Notice {
    pub title: &'static str,
    pub text: String,
}

pub enum Event {
    Notice(Notice),
    /// The system output level (0–1) or mute changed, from any app or the volume keys.
    Volume {
        level: f32,
        muted: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Power {
    pub plugged_in: bool,
    pub percent: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
// Only the Windows probe builds these.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub enum Network {
    /// Internet access, with the connection's name (a Wi-Fi network, "Ethernet").
    Internet(String),
    NoInternet,
    Offline,
}

#[derive(Default)]
struct State {
    power: Option<Power>,
    network: Option<Network>,
    network_candidate: Option<Network>,
    caps_lock: Option<bool>,
    volume: Option<(f32, bool)>,
}

impl State {
    /// A level change of more than half a percent, or mute, shows the volume HUD.
    fn volume(&mut self, level: f32, muted: bool) -> Option<Event> {
        let (previous, was_muted) = self.volume.replace((level, muted))?;
        ((previous - level).abs() > 0.005 || was_muted != muted)
            .then_some(Event::Volume { level, muted })
    }

    /// Charger changes, "fully charged", and the 20 % and 10 % warnings while on battery.
    fn power(&mut self, next: Power) -> Option<Notice> {
        let previous = self.power.replace(next)?;
        let percent = next.percent;
        let text = if next.plugged_in != previous.plugged_in {
            if next.plugged_in {
                format!("Charging · {percent}%")
            } else {
                format!("Charger unplugged · {percent}%")
            }
        } else if next.plugged_in && percent == 100 && previous.percent < 100 {
            "Fully charged · you can unplug".into()
        } else if !next.plugged_in && percent <= 10 && previous.percent > 10 {
            format!("Battery critical · {percent}% · plug in now")
        } else if !next.plugged_in && percent <= 20 && previous.percent > 20 {
            format!("Low battery · {percent}% · plug in soon")
        } else {
            return None;
        };
        Some(Notice {
            title: "Battery",
            text,
        })
    }

    /// A connection change has to hold for two checks, so a Wi-Fi handover stays quiet.
    fn network(&mut self, next: Network) -> Option<Notice> {
        let Some(current) = &self.network else {
            self.network = Some(next);
            return None;
        };
        if *current == next {
            self.network_candidate = None;
            return None;
        }
        if self.network_candidate.as_ref() != Some(&next) {
            self.network_candidate = Some(next);
            return None;
        }
        let text = match &next {
            Network::Internet(name) if !name.is_empty() => format!("Connected · {name}"),
            Network::Internet(_) => "Connected".into(),
            Network::NoInternet => "No internet access".into(),
            Network::Offline => "Disconnected".into(),
        };
        self.network = Some(next);
        self.network_candidate = None;
        Some(Notice {
            title: "Network",
            text,
        })
    }

    fn caps_lock(&mut self, on: bool) -> Option<Notice> {
        let previous = self.caps_lock.replace(on)?;
        (previous != on).then(|| Notice {
            title: "Caps Lock",
            text: if on { "Caps Lock on" } else { "Caps Lock off" }.into(),
        })
    }
}

pub struct Monitor {
    _stop: mpsc::Sender<()>,
    rx: mpsc::Receiver<Event>,
}

impl Monitor {
    pub fn start(ctx: eframe::egui::Context) -> Self {
        let (stop, stopped) = mpsc::channel::<()>();
        let (tx, rx) = mpsc::channel();
        let _ = std::thread::Builder::new()
            .name("Cikung system notices".into())
            .spawn(move || {
                probe::init();
                let mut state = State::default();
                for tick in 0u32.. {
                    // Caps Lock and volume every 150 ms so they answer the keys; battery and
                    // network every 3 s.
                    let mut events: Vec<Event> = state
                        .caps_lock(probe::caps_lock())
                        .map(Event::Notice)
                        .into_iter()
                        .chain(
                            probe::volume().and_then(|(level, muted)| state.volume(level, muted)),
                        )
                        .collect();
                    if tick % 20 == 0 {
                        let power = probe::power().and_then(|p| state.power(p));
                        let network = probe::network().and_then(|n| state.network(n));
                        events.extend(power.into_iter().chain(network).map(Event::Notice));
                    }
                    for event in events {
                        if tx.send(event).is_err() {
                            return;
                        }
                        ctx.request_repaint();
                    }
                    if stopped.recv_timeout(Duration::from_millis(150))
                        != Err(mpsc::RecvTimeoutError::Timeout)
                    {
                        return;
                    }
                }
            });
        Self { _stop: stop, rx }
    }

    pub fn poll(&self) -> Vec<Event> {
        self.rx.try_iter().collect()
    }
}

#[cfg(target_os = "windows")]
mod probe {
    use super::{Network, Power};

    pub fn init() {
        use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
        // The connection profile is a WinRT call; this thread lives as long as the monitor.
        let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
    }

    pub fn volume() -> Option<(f32, bool)> {
        crate::native::system_volume()
    }

    pub fn caps_lock() -> bool {
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CAPITAL};
        // The low bit is the toggle state, which Windows keeps for every thread.
        (unsafe { GetKeyState(i32::from(VK_CAPITAL.0)) } & 1) != 0
    }

    /// `None` without a battery (a desktop PC) or while Windows does not know its state.
    pub fn power() -> Option<Power> {
        use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
        let mut status = SYSTEM_POWER_STATUS::default();
        unsafe { GetSystemPowerStatus(&mut status) }.ok()?;
        // BatteryFlag 128: no system battery; 255: unknown. ACLineStatus 255: unknown.
        let known = status.BatteryFlag & 128 == 0
            && status.BatteryFlag != 255
            && status.BatteryLifePercent <= 100
            && status.ACLineStatus <= 1;
        known.then_some(Power {
            plugged_in: status.ACLineStatus == 1,
            percent: status.BatteryLifePercent,
        })
    }

    pub fn network() -> Option<Network> {
        use windows::Networking::Connectivity::{NetworkConnectivityLevel, NetworkInformation};
        // No internet profile at all means no connection.
        let Ok(profile) = NetworkInformation::GetInternetConnectionProfile() else {
            return Some(Network::Offline);
        };
        Some(match profile.GetNetworkConnectivityLevel().ok()? {
            NetworkConnectivityLevel::InternetAccess => Network::Internet(
                profile
                    .ProfileName()
                    .map(|name| name.to_string())
                    .unwrap_or_default(),
            ),
            NetworkConnectivityLevel::None => Network::Offline,
            _ => Network::NoInternet,
        })
    }
}

#[cfg(not(target_os = "windows"))]
mod probe {
    use super::{Network, Power};

    pub fn init() {}
    pub fn volume() -> Option<(f32, bool)> {
        None
    }
    pub fn caps_lock() -> bool {
        false
    }
    pub fn power() -> Option<Power> {
        None
    }
    pub fn network() -> Option<Network> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn power(state: &mut State, plugged_in: bool, percent: u8) -> Option<String> {
        state
            .power(Power {
                plugged_in,
                percent,
            })
            .map(|n| n.text)
    }

    #[test]
    fn announces_changes_and_thresholds_once() {
        let mut state = State::default();
        // The first reading is quiet; then 21 → 20 warns once, 20 → 19 does not.
        assert_eq!(power(&mut state, false, 25), None);
        assert_eq!(power(&mut state, false, 21), None);
        assert_eq!(
            power(&mut state, false, 20).as_deref(),
            Some("Low battery · 20% · plug in soon")
        );
        assert_eq!(power(&mut state, false, 19), None);
        assert_eq!(
            power(&mut state, false, 10).as_deref(),
            Some("Battery critical · 10% · plug in now")
        );
        assert_eq!(
            power(&mut state, true, 10).as_deref(),
            Some("Charging · 10%")
        );
        assert_eq!(
            power(&mut state, true, 100).as_deref(),
            Some("Fully charged · you can unplug")
        );
        assert_eq!(
            power(&mut state, false, 100).as_deref(),
            Some("Charger unplugged · 100%")
        );

        // A network change needs two matching checks.
        assert!(state.network(Network::Internet("Home".into())).is_none());
        assert!(state.network(Network::Offline).is_none());
        assert!(state.network(Network::Internet("Home".into())).is_none());
        assert!(state.network(Network::Offline).is_none());
        assert_eq!(
            state.network(Network::Offline).map(|n| n.text).as_deref(),
            Some("Disconnected")
        );

        // Volume: quiet first reading and jitter, then a step or mute.
        assert!(state.volume(0.4, false).is_none());
        assert!(state.volume(0.402, false).is_none());
        assert!(matches!(
            state.volume(0.5, false),
            Some(Event::Volume { level, muted: false }) if level == 0.5
        ));
        assert!(state.volume(0.5, true).is_some());

        // Caps Lock: quiet first reading, then every toggle.
        assert!(state.caps_lock(false).is_none());
        assert!(state.caps_lock(false).is_none());
        assert_eq!(
            state.caps_lock(true).map(|n| n.text).as_deref(),
            Some("Caps Lock on")
        );
    }
}
