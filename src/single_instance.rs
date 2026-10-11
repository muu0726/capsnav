//! 二重起動（多重起動）防止モジュール

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
            log::warn!("既存のインスタンスが既に実行中です。二重起動を防止して終了します。");
            None
        } else {
            log::info!("単一インスタンス Mutex を正常に取得しました。");
            Some(SingleInstanceGuard { handle })
        }
    }
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

#[cfg(not(windows))]
pub struct SingleInstanceGuard;

#[cfg(not(windows))]
pub fn acquire_single_instance(_app_id: &str) -> Option<SingleInstanceGuard> {
    Some(SingleInstanceGuard)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_instance_acquisition() {
        let guard1 = acquire_single_instance("capsnav_unit_test");
        assert!(guard1.is_some(), "最初のインスタンスは取得できる必要があります");

        #[cfg(windows)]
        {
            let guard2 = acquire_single_instance("capsnav_unit_test");
            assert!(guard2.is_none(), "2つ目のインスタンスはブロックされる必要があります");
        }

        drop(guard1);

        #[cfg(windows)]
        {
            let guard3 = acquire_single_instance("capsnav_unit_test");
            assert!(guard3.is_some(), "解放後は再度取得できる必要があります");
        }
    }
}
