//! Low-overhead resident-process memory evidence for Runtime Server telemetry.

pub(super) const RUNTIME_SERVER_MEMORY_BUDGET_BYTES: u64 = 1024 * 1024 * 1024;
pub(super) const MEMORY_WATERMARK_STEP_BYTES: u64 = 256 * 1024 * 1024;
pub(super) const RUNTIME_EVENT_LOOP_LAG_BUDGET_MICROS: u64 = 10_000;
pub(super) const RUNTIME_SERVER_OPEN_DESCRIPTOR_BUDGET: u64 = 1_024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ProcessMemoryObservation {
    pub resident_bytes: Option<u64>,
    pub peak_resident_bytes: Option<u64>,
    pub budget_bytes: u64,
    pub disk_read_bytes: Option<u64>,
    pub disk_write_bytes: Option<u64>,
    pub page_ins: Option<u64>,
    pub open_descriptors: Option<u64>,
    pub open_descriptor_budget: u64,
    pub event_loop_lag_micros: u64,
    pub event_loop_lag_budget_micros: u64,
    pub runtime_worker_threads: u64,
    pub runtime_alive_tasks: u64,
    pub runtime_global_queue_depth: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RuntimeSchedulerObservation {
    pub worker_threads: u64,
    pub alive_tasks: u64,
    pub global_queue_depth: u64,
}

pub(super) fn observe_runtime_scheduler() -> RuntimeSchedulerObservation {
    let metrics = tokio::runtime::Handle::current().metrics();
    RuntimeSchedulerObservation {
        worker_threads: u64::try_from(metrics.num_workers()).unwrap_or(u64::MAX),
        alive_tasks: u64::try_from(metrics.num_alive_tasks()).unwrap_or(u64::MAX),
        global_queue_depth: u64::try_from(metrics.global_queue_depth()).unwrap_or(u64::MAX),
    }
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

    pub fn open_descriptor_budget_exceeded(self) -> bool {
        self.open_descriptors
            .is_some_and(|count| count > self.open_descriptor_budget)
    }

    pub fn open_descriptor_budget_status(self) -> &'static str {
        if self.open_descriptor_budget_exceeded() {
            "budget-exceeded"
        } else {
            "within-budget"
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_generation_resource_gate.rs"]
mod runtime_server_generation_resource_gate;

pub(super) fn observe_process_memory(
    event_loop_lag_micros: u64,
    scheduler: RuntimeSchedulerObservation,
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
        open_descriptors: open_descriptor_count(),
        open_descriptor_budget: RUNTIME_SERVER_OPEN_DESCRIPTOR_BUDGET,
        event_loop_lag_micros,
        event_loop_lag_budget_micros: RUNTIME_EVENT_LOOP_LAG_BUDGET_MICROS,
        runtime_worker_threads: scheduler.worker_threads,
        runtime_alive_tasks: scheduler.alive_tasks,
        runtime_global_queue_depth: scheduler.global_queue_depth,
    })
}

#[cfg(target_os = "macos")]
fn open_descriptor_count() -> Option<u64> {
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct ProcFdInfo {
        proc_fd: i32,
        proc_fdtype: u32,
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
    const PROC_PIDLISTFDS: i32 = 1;
    let pid = i32::try_from(std::process::id()).ok()?;
    let required = unsafe { proc_pidinfo(pid, PROC_PIDLISTFDS, 0, std::ptr::null_mut(), 0) };
    if required <= 0 {
        return None;
    }
    let item_size = std::mem::size_of::<ProcFdInfo>();
    let capacity = usize::try_from(required).ok()?.div_ceil(item_size);
    let mut descriptors = vec![ProcFdInfo::default(); capacity];
    let buffer_size = i32::try_from(descriptors.len().checked_mul(item_size)?).ok()?;
    let read = unsafe {
        proc_pidinfo(
            pid,
            PROC_PIDLISTFDS,
            0,
            descriptors.as_mut_ptr().cast(),
            buffer_size,
        )
    };
    (read >= 0).then(|| {
        u64::try_from(usize::try_from(read).unwrap_or_default() / item_size).unwrap_or(u64::MAX)
    })
}

#[cfg(target_os = "linux")]
fn open_descriptor_count() -> Option<u64> {
    std::fs::read_dir("/proc/self/fd")
        .ok()
        .map(|entries| entries.filter_map(Result::ok).count())
        .and_then(|count| u64::try_from(count).ok())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn open_descriptor_count() -> Option<u64> {
    None
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
