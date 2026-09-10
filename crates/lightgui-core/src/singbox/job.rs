use std::os::windows::io::RawHandle;

#[cfg(windows)]
pub fn assign_current_job(raw_handle: RawHandle) -> Result<(), String> {
    imp::assign(raw_handle)
}

#[cfg(not(windows))]
pub fn assign_current_job(_raw_handle: *mut std::ffi::c_void) -> Result<(), String> {
    Err("job objects are a Windows feature".into())
}

#[cfg(windows)]
mod imp {
    use std::os::windows::io::RawHandle;
    use std::ptr::null;
    use std::sync::OnceLock;

    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    struct Job(HANDLE);

    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}

    static JOB: OnceLock<Option<Job>> = OnceLock::new();

    fn create() -> Result<Job, String> {
        let handle = unsafe { CreateJobObjectW(null(), null()) };
        if handle.is_null() || handle == -1isize as *mut _ {
            return Err("CreateJobObjectW failed".into());
        }

        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

        let ok = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };

        if ok == 0 {
            return Err("SetInformationJobObject failed".into());
        }

        Ok(Job(handle))
    }

    fn job() -> Option<HANDLE> {
        JOB.get_or_init(|| match create() {
            Ok(job) => Some(job),
            Err(e) => {
                eprintln!("[lightgui] {e}; sing-box will not be killed on hard parent kill");
                None
            }
        })
        .as_ref()
        .map(|j| j.0)
    }

    pub fn assign(raw_handle: RawHandle) -> Result<(), String> {
        let job_handle = job().ok_or_else(|| "job object unavailable".to_string())?;
        let ok = unsafe { AssignProcessToJobObject(job_handle, raw_handle as HANDLE) };
        if ok == 0 {
            Err("AssignProcessToJobObject failed".into())
        } else {
            Ok(())
        }
    }

    #[cfg(test)]
    pub fn probe_handle() -> Option<usize> {
        job().map(|h| h as usize)
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::imp::*;

    #[test]
    fn job_handle_created_and_cached() {
        let first = probe_handle();
        let second = probe_handle();
        assert!(first.is_some());
        assert_eq!(first, second);
    }
}
