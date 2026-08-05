//! Low-overhead resident-process memory evidence for Runtime Server telemetry.

pub(super) const RUNTIME_SERVER_MEMORY_BUDGET_BYTES: u64 = 1024 * 1024 * 1024;
pub(super) const MEMORY_WATERMARK_STEP_BYTES: u64 = 256 * 1024 * 1024;
pub(super) const RUNTIME_EVENT_LOOP_LAG_BUDGET_MICROS: u64 = 10_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ProcessMemoryObservation {
    pub resident_bytes: Option<u64>,
    pub peak_resident_bytes: Option<u64>,
    pub budget_bytes: u64,
    pub disk_read_bytes: Option<u64>,
    pub disk_write_bytes: Option<u64>,
    pub page_ins: Option<u64>,
    pub event_loop_lag_micros: u64,
    pub event_loop_lag_budget_micros: u64,
}

impl ProcessMemoryObservation {
    pub fn budget_exceeded(self) -> bool {
        self.resident_bytes
            .into_iter()
            .chain(self.peak_resident_bytes)
            .any(|bytes| bytes > self.budget_bytes)
    }

    pub fn budget_status(self) -> &'static str {
        if self.budget_exceeded() {
            "budget-exceeded"
        } else {
            "within-budget"
        }
    }

    pub fn event_loop_lag_budget_exceeded(self) -> bool {
        self.event_loop_lag_micros > self.event_loop_lag_budget_micros
    }

    pub fn event_loop_lag_budget_status(self) -> &'static str {
        if self.event_loop_lag_budget_exceeded() {
            "budget-exceeded"
        } else {
            "within-budget"
        }
    }
}

pub(super) fn observe_process_memory(
    event_loop_lag_micros: u64,
) -> Option<ProcessMemoryObservation> {
    let resident_bytes = current_resident_bytes();
    let peak_resident_bytes = peak_resident_bytes();
    let io = process_io_counters();
    Some(ProcessMemoryObservation {
        resident_bytes,
        peak_resident_bytes,
        budget_bytes: RUNTIME_SERVER_MEMORY_BUDGET_BYTES,
        disk_read_bytes: io.map(|io| io.disk_read_bytes),
        disk_write_bytes: io.map(|io| io.disk_write_bytes),
        page_ins: io.map(|io| io.page_ins),
        event_loop_lag_micros,
        event_loop_lag_budget_micros: RUNTIME_EVENT_LOOP_LAG_BUDGET_MICROS,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProcessIoCounters {
    disk_read_bytes: u64,
    disk_write_bytes: u64,
    page_ins: u64,
}

#[cfg(target_os = "macos")]
fn process_io_counters() -> Option<ProcessIoCounters> {
    #[repr(C)]
    #[derive(Default)]
    struct RUsageInfoV2 {
        uuid: [u8; 16],
        user_time: u64,
        system_time: u64,
        package_idle_wakeups: u64,
        interrupt_wakeups: u64,
        page_ins: u64,
        wired_size: u64,
        resident_size: u64,
        physical_footprint: u64,
        process_start_absolute_time: u64,
        process_exit_absolute_time: u64,
        child_user_time: u64,
        child_system_time: u64,
        child_package_idle_wakeups: u64,
        child_interrupt_wakeups: u64,
        child_page_ins: u64,
        child_elapsed_absolute_time: u64,
        disk_read_bytes: u64,
        disk_write_bytes: u64,
    }
    unsafe extern "C" {
        fn proc_pid_rusage(pid: i32, flavor: i32, buffer: *mut libc::c_void) -> i32;
    }
    const RUSAGE_INFO_V2: i32 = 2;
    let mut info = RUsageInfoV2::default();
    let status = unsafe {
        proc_pid_rusage(
            i32::try_from(std::process::id()).ok()?,
            RUSAGE_INFO_V2,
            (&mut info as *mut RUsageInfoV2).cast(),
        )
    };
    (status == 0).then_some(ProcessIoCounters {
        disk_read_bytes: info.disk_read_bytes,
        disk_write_bytes: info.disk_write_bytes,
        page_ins: info.page_ins,
    })
}

#[cfg(not(target_os = "macos"))]
fn process_io_counters() -> Option<ProcessIoCounters> {
    None
}

#[cfg(target_os = "macos")]
fn current_resident_bytes() -> Option<u64> {
    #[repr(C)]
    #[derive(Default)]
    struct ProcTaskInfo {
        virtual_size: u64,
        resident_size: u64,
        total_user: u64,
        total_system: u64,
        threads_user: u64,
        threads_system: u64,
        policy: i32,
        faults: i32,
        pageins: i32,
        cow_faults: i32,
        messages_sent: i32,
        messages_received: i32,
        syscalls_mach: i32,
        syscalls_unix: i32,
        csw: i32,
        threadnum: i32,
        numrunning: i32,
        priority: i32,
    }
    unsafe extern "C" {
        fn proc_pidinfo(
            pid: i32,
            flavor: i32,
            arg: u64,
            buffer: *mut libc::c_void,
            buffersize: i32,
        ) -> i32;
    }
    const PROC_PIDTASKINFO: i32 = 4;
    let mut info = ProcTaskInfo::default();
    let size = i32::try_from(std::mem::size_of::<ProcTaskInfo>()).ok()?;
    let read = unsafe {
        proc_pidinfo(
            i32::try_from(std::process::id()).ok()?,
            PROC_PIDTASKINFO,
            0,
            (&mut info as *mut ProcTaskInfo).cast(),
            size,
        )
    };
    (read == size).then_some(info.resident_size)
}

#[cfg(not(target_os = "macos"))]
fn current_resident_bytes() -> Option<u64> {
    None
}

#[cfg(unix)]
fn peak_resident_bytes() -> Option<u64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    let status = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    if status != 0 {
        return None;
    }
    let peak = unsafe { usage.assume_init() }.ru_maxrss;
    let peak = u64::try_from(peak).ok()?;
    #[cfg(target_os = "macos")]
    return Some(peak);
    #[cfg(not(target_os = "macos"))]
    return peak.checked_mul(1024);
}

#[cfg(not(unix))]
fn peak_resident_bytes() -> Option<u64> {
    None
}
