//! 二重起動（多重起動）防止および画面表示シグナル連携モジュール

#[cfg(windows)]
pub struct SingleInstanceGuard {
    handle: windows::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
unsafe impl Send for SingleInstanceGuard {}
#[cfg(windows)]
unsafe impl Sync for SingleInstanceGuard {}

#[cfg(windows)]
pub fn acquire_single_instance(app_id: &str) -> Option<SingleInstanceGuard> {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::CreateMutexW;

    let mutex_name = format!("Local\\{}_single_instance_mutex", app_id);
    let hstring = HSTRING::from(mutex_name);
    unsafe {
        let handle = CreateMutexW(None, true, windows::core::PCWSTR(hstring.as_ptr())).ok()?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            use windows::Win32::Foundation::CloseHandle;
            let _ = CloseHandle(handle);
            log::warn!("既存のインスタンスが既に実行中です。二重起動を防止して終了します。");
            None
        } else {
            log::info!("単一インスタンス Mutex を正常に取得しました。");
            Some(SingleInstanceGuard { handle })
        }
    }
}

#[cfg(windows)]
impl SingleInstanceGuard {
    pub fn cleanup(&self) {}
}

#[cfg(windows)]
impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        use windows::Win32::Foundation::CloseHandle;
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

#[cfg(windows)]
pub fn signal_existing_instance(app_id: &str) {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenEventW, SetEvent, EVENT_MODIFY_STATE};

    let event_name = format!("Local\\{}_show_event", app_id);
    let hstring = HSTRING::from(event_name);
    unsafe {
        if let Ok(handle) = OpenEventW(EVENT_MODIFY_STATE, false, windows::core::PCWSTR(hstring.as_ptr())) {
            let _ = SetEvent(handle);
            let _ = CloseHandle(handle);
            log::info!("既存インスタンスへ画面表示シグナルを送信しました。");
        }
    }
}

#[cfg(windows)]
pub struct ShowEventListener {
    handle: windows::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
unsafe impl Send for ShowEventListener {}
#[cfg(windows)]
unsafe impl Sync for ShowEventListener {}

#[cfg(windows)]
pub fn create_show_listener(app_id: &str) -> Option<ShowEventListener> {
    use windows::core::HSTRING;
    use windows::Win32::System::Threading::CreateEventW;

    let event_name = format!("Local\\{}_show_event", app_id);
    let hstring = HSTRING::from(event_name);
    unsafe {
        let handle = CreateEventW(None, false, false, windows::core::PCWSTR(hstring.as_ptr())).ok()?;
        Some(ShowEventListener { handle })
    }
}

#[cfg(windows)]
impl ShowEventListener {
    pub fn wait_next(&self) -> bool {
        use windows::Win32::Foundation::WAIT_OBJECT_0;
        use windows::Win32::System::Threading::{WaitForSingleObject, INFINITE};
        unsafe {
            WaitForSingleObject(self.handle, INFINITE) == WAIT_OBJECT_0
        }
    }
}

#[cfg(windows)]
impl Drop for ShowEventListener {
    fn drop(&mut self) {
        use windows::Win32::Foundation::CloseHandle;
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

#[cfg(not(windows))]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(not(windows))]
use std::path::PathBuf;
#[cfg(not(windows))]
use std::sync::Mutex;

#[cfg(not(windows))]
static PENDING_LISTENERS: Mutex<Vec<(String, UnixListener)>> = Mutex::new(Vec::new());

#[cfg(not(windows))]
fn get_socket_path(app_id: &str) -> PathBuf {
    let user = std::env::var("USER").unwrap_or_else(|_| "default".to_string());
    std::env::temp_dir().join(format!("{}_{}.sock", app_id, user))
}

#[cfg(not(windows))]
pub struct SingleInstanceGuard {
    app_id: String,
    socket_path: PathBuf,
}

#[cfg(not(windows))]
impl SingleInstanceGuard {
    pub fn cleanup(&self) {
        let _ = std::fs::remove_file(&self.socket_path);
        if let Ok(mut lock) = PENDING_LISTENERS.lock() {
            lock.retain(|(id, _)| id != &self.app_id);
        }
    }
}

#[cfg(not(windows))]
impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        self.cleanup();
    }
}

#[cfg(not(windows))]
pub fn acquire_single_instance(app_id: &str) -> Option<SingleInstanceGuard> {
    let socket_path = get_socket_path(app_id);

    // 既存のリスナーに接続可能かテスト
    if UnixStream::connect(&socket_path).is_ok() {
        log::warn!("既存のインスタンスが既に実行中です。二重起動を防止して終了します。");
        return None;
    }

    // 接続できない場合、古い残存ソケットファイルがあれば削除して新規バインド
    let _ = std::fs::remove_file(&socket_path);
    match UnixListener::bind(&socket_path) {
        Ok(listener) => {
            log::info!("単一インスタンス ソケット ({:?}) を正常にバインドしました。", socket_path);
            if let Ok(mut lock) = PENDING_LISTENERS.lock() {
                lock.retain(|(id, _)| id != app_id);
                lock.push((app_id.to_string(), listener));
            }
            Some(SingleInstanceGuard {
                app_id: app_id.to_string(),
                socket_path,
            })
        }
        Err(e) => {
            log::error!("単一インスタンス ソケットのバインドに失敗しました: {}", e);
            None
        }
    }
}

#[cfg(not(windows))]
pub fn signal_existing_instance(app_id: &str) {
    let socket_path = get_socket_path(app_id);
    use std::io::Write;
    if let Ok(mut stream) = UnixStream::connect(&socket_path) {
        let _ = stream.write_all(b"show\n");
        log::info!("既存インスタンスへ画面表示シグナルを送信しました。");
    }
}

#[cfg(not(windows))]
pub struct ShowEventListener {
    listener: UnixListener,
}

#[cfg(not(windows))]
pub fn create_show_listener(app_id: &str) -> Option<ShowEventListener> {
    if let Ok(mut lock) = PENDING_LISTENERS.lock() {
        if let Some(pos) = lock.iter().position(|(id, _)| id == app_id) {
            let (_, listener) = lock.swap_remove(pos);
            return Some(ShowEventListener { listener });
        }
    }
    None
}

#[cfg(not(windows))]
impl ShowEventListener {
    pub fn wait_next(&self) -> bool {
        match self.listener.accept() {
            Ok((mut stream, _)) => {
                use std::io::Read;
                let mut buf = [0u8; 16];
                let _ = stream.read(&mut buf);
                true
            }
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_instance_acquisition() {
        let app_id = format!("capsnav_unit_test_{}", std::process::id());
        let guard1 = acquire_single_instance(&app_id);
        assert!(guard1.is_some(), "最初のインスタンスは取得できる必要があります");

        let guard2 = acquire_single_instance(&app_id);
        assert!(guard2.is_none(), "2つ目のインスタンスはブロックされる必要があります");

        drop(guard1);

        let guard3 = acquire_single_instance(&app_id);
        assert!(guard3.is_some(), "解放後は再度取得できる必要があります");
    }

    #[test]
    fn test_single_instance_signal_flow() {
        let app_id = format!("capsnav_signal_test_{}", std::process::id());
        let guard = acquire_single_instance(&app_id);
        assert!(guard.is_some());

        let listener = create_show_listener(&app_id);
        assert!(listener.is_some());
        let listener = listener.unwrap();

        let app_id_clone = app_id.clone();
        let handle = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(50));
            signal_existing_instance(&app_id_clone);
        });

        assert!(listener.wait_next(), "シグナルを受信して wait_next が true を返す必要があります");
        let _ = handle.join();
        drop(guard);
    }
}
