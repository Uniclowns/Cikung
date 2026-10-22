//! One Cikung window. Later launches request a page in the existing instance.
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};
pub fn data_dir() -> PathBuf {
    eframe::storage_dir("com.cikung.desktop")
        .unwrap_or_else(std::env::temp_dir)
        .join("connections")
}
fn request_path() -> PathBuf {
    data_dir().join("show.json")
}
#[derive(Serialize, Deserialize)]
struct Request {
    nonce: String,
    page: Option<String>,
}
fn read_request() -> Option<Request> {
    let path = request_path();
    if fs::metadata(&path).ok()?.len() > 2048 {
        return None;
    }
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}
pub fn show(page: Option<&str>) -> Result<(), String> {
    fs::create_dir_all(data_dir()).map_err(|e| e.to_string())?;
    let request = Request {
        nonce: format!(
            "{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ),
        page: page.map(str::to_owned),
    };
    let temporary = data_dir().join(format!("show-{}.tmp", std::process::id()));
    fs::write(
        &temporary,
        serde_json::to_vec(&request).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(temporary, request_path()).map_err(|e| e.to_string())
}
pub struct Requests {
    nonce: String,
    last_poll: Instant,
    diagnostic: bool,
}
impl Requests {
    pub fn new(diagnostic: bool) -> Self {
        Self {
            nonce: read_request().map(|r| r.nonce).unwrap_or_default(),
            last_poll: Instant::now(),
            diagnostic,
        }
    }
    pub fn poll(&mut self) -> Option<Option<String>> {
        if self.diagnostic || self.last_poll.elapsed() < Duration::from_millis(250) {
            return None;
        }
        self.last_poll = Instant::now();
        let request = read_request()?;
        if request.nonce == self.nonce {
            return None;
        }
        self.nonce = request.nonce;
        Some(request.page)
    }
}
#[cfg(windows)]
pub struct Instance(windows::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl Instance {
    pub fn acquire() -> Result<Option<Self>, String> {
        use windows::{
            Win32::{
                Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError},
                System::Threading::CreateMutexW,
            },
            core::w,
        };
        let handle = unsafe { CreateMutexW(None, false, w!("Local\\Cikung.desktop")) }
            .map_err(|e| e.to_string())?;
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            unsafe {
                let _ = CloseHandle(handle);
            }
            return Ok(None);
        }
        Ok(Some(Self(handle)))
    }
}
#[cfg(windows)]
impl Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
#[cfg(unix)]
pub struct Instance(std::fs::File);
#[cfg(unix)]
impl Instance {
    pub fn acquire() -> Result<Option<Self>, String> {
        use std::os::fd::AsRawFd;
        unsafe extern "C" {
            fn flock(fd: std::ffi::c_int, op: std::ffi::c_int) -> std::ffi::c_int;
        }
        fs::create_dir_all(data_dir()).map_err(|e| e.to_string())?;
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(data_dir().join("instance.lock"))
            .map_err(|e| e.to_string())?;
        if unsafe { flock(file.as_raw_fd(), 2 | 4) } == 0 {
            Ok(Some(Self(file)))
        } else {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::WouldBlock {
                Ok(None)
            } else {
                Err(error.to_string())
            }
        }
    }
}
#[cfg(unix)]
impl Drop for Instance {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        unsafe extern "C" {
            fn flock(fd: std::ffi::c_int, op: std::ffi::c_int) -> std::ffi::c_int;
        }
        unsafe {
            let _ = flock(self.0.as_raw_fd(), 8);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn request_is_bounded_plain_data() {
        let request = Request {
            nonce: "test".into(),
            page: Some("agents".into()),
        };
        let encoded = serde_json::to_string(&request).unwrap();
        let decoded: Request = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.page.as_deref(), Some("agents"));
        assert!(encoded.len() < 2048);
    }
    #[cfg(windows)]
    #[test]
    fn second_instance_cannot_acquire_the_same_window_guard() {
        let first = Instance::acquire().unwrap();
        if first.is_some() {
            assert!(Instance::acquire().unwrap().is_none());
            drop(first);
            assert!(Instance::acquire().unwrap().is_some());
        }
    }
}
