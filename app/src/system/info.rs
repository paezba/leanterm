use std::collections::VecDeque;
use std::ffi::OsStr;

use byte_unit::Byte;
use chrono::{DateTime, Utc};
use leanterm_core::channel::ChannelState;
use leanterm_ui::{App, AppContext, Entity, ModelContext, SingletonEntity};
use sysinfo::ProcessesToUpdate;

use crate::system::memory_footprint;

/// The threshold at which we emit a memory usage warning, in bytes.
const MEMORY_USAGE_WARNING_THRESHOLD_BYTES: u64 = Byte::GIGABYTE.as_u64() * 10;

/// The refresh interval for system information, in seconds.
const REFRESH_INTERVAL_S: usize = 5;
/// The refresh interval for system information.
const REFRESH_INTERVAL: std::time::Duration =
    std::time::Duration::from_secs(REFRESH_INTERVAL_S as u64);

/// The time window that a resource usage report covers, in seconds.
const REPORT_WINDOW_S: usize = 300;
/// The number of data points aggregated into a resource usage report.
const REPORT_SAMPLE_COUNT: usize = REPORT_WINDOW_S / REFRESH_INTERVAL_S;

// Make sure the refresh interval cleanly divides the report window into an
// integral number of samples.
static_assertions::const_assert_eq!(REPORT_WINDOW_S % REFRESH_INTERVAL_S, 0);

pub enum SystemInfoEvent {
    /// There is new system info available for consumers to query.
    Refreshed,
    /// The application is using a large quantity of memory.
    MemoryUsageHigh,
}

pub struct SystemInfo {
    /// A structure we can use to efficiently query system information.
    system: sysinfo::System,
    /// Whether or not we've already emitted an event due to high memory usage.
    has_emitted_memory_warning_event: bool,
    /// Set to the memory footprint that crossed `MEMORY_USAGE_WARNING_THRESHOLD_BYTES` on the
    /// previous poll tick, while we wait for the next tick to confirm the spike is sustained rather
    /// than a transient blip.  `None` when there is no pending confirmation.
    pending_excessive_memory_footprint_bytes: Option<u64>,
    /// A circular buffer storing resource usage data.
    stats: StatsBuffer,
    /// A helper structure for reporting resource usage via telemetry events.
    resource_usage_reporter: ResourceUsageReporter,
    /// The long OS version.
    long_os_version: Option<String>,
}

impl SystemInfo {
    /// Creates a new [`SystemInfo`] model and begins periodic fetching of
    /// system information.
    ///
    /// Currently only retrieves and exposes memory usage information for the
    /// current process.
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        let mut me = Self {
            system: sysinfo::System::new(),
            has_emitted_memory_warning_event: false,
            pending_excessive_memory_footprint_bytes: None,
            stats: Default::default(),
            resource_usage_reporter: Default::default(),
            long_os_version: sysinfo::System::long_os_version(),
        };

        // Initialize the underlying system info.  This is necessary in order
        // for our first read of CPU stats to be accurate, as they are computed
        // as a delta between the previous refresh and now.
        me.system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[Self::current_pid()]),
            false, /* refresh_dead_processes */
            Self::refresh_kind(),
        );

        // If we're doing automated heap usage tracking, set up periodic
        // refreshes of the memory usage data.
        Self::schedule_refresh(ctx);

        me
    }

    pub fn handle_block_created(&mut self) {
        self.resource_usage_reporter.handle_block_created();
    }

    /// Returns the amount of memory being used by the current process, in
    /// bytes.
    pub fn used_memory(&self) -> Byte {
        self.system
            .process(Self::current_pid())
            .expect("current process should exist")
            .memory()
            .into()
    }

    /// Returns the full memory footprint of the current process, in bytes.
    ///
    /// Unlike [`used_memory`] (RSS), this includes memory that has been
    /// swapped out or compressed by the OS.  On macOS this matches the value
    /// shown by Activity Monitor.
    pub fn memory_footprint(&self) -> Byte {
        memory_footprint::memory_footprint_bytes().into()
    }

    /// Returns the average CPU usage over the refresh interval.
    ///
    /// If one CPU core is utilized at 100%, this will return 1.  It may return
    /// a value >1 on multi-core machines.
    pub fn cpu_usage(&self) -> f32 {
        let total_usage = self
            .system
            .process(Self::current_pid())
            .expect("current process should exist")
            .cpu_usage();
        total_usage / 100.
    }

    fn schedule_refresh(ctx: &mut ModelContext<Self>) {
        ctx.spawn(
            async {
                leanterm_ui::r#async::Timer::after(REFRESH_INTERVAL).await;
            },
            |me, _, ctx| {
                me.refresh(ctx);
                Self::schedule_refresh(ctx);
            },
        );
    }

    fn refresh(&mut self, ctx: &mut ModelContext<Self>) {
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[Self::current_pid()]),
            false, /* refresh_dead_processes */
            Self::refresh_kind(),
        );
        ctx.emit(SystemInfoEvent::Refreshed);

        // Add resource usage information to our circular buffer.
        self.stats.push(Sample {
            cpu: self.cpu_usage(),
        });

        let rss = self.used_memory();
        let footprint = self.memory_footprint();
        self.check_for_excessive_memory_usage(rss, footprint, ctx);

        // Once we have a full buffer of statistics, consider sending a report
        // each time we store new resource usage data.
        if self.stats.is_full() {
            self.resource_usage_reporter.maybe_send_report(ctx);
        }
    }

    /// Checks for excessive memory usage.  This may send a telemetry event
    /// and trigger a Sentry heap profile dump if excessive usage is detected.
    ///
    /// The threshold check uses `memory_footprint` (which includes swapped
    /// and compressed pages) so we actually detect high memory situations.
    /// The Rudderstack telemetry event still reports `rss` so existing
    /// dashboards are unaffected.
    ///
    /// A crossing of the threshold is only reported once it's confirmed still excessive on the next
    /// poll tick, rather than on the tick that first observed it, so a short-lived spike that's
    /// freed moments later is skipped instead of producing a worthless Sentry event and heap
    /// profile.  A skip does not consume `has_emitted_memory_warning_event`, so an early transient
    /// spike doesn't silence the process for the rest of its lifetime.
    fn check_for_excessive_memory_usage(
        &mut self,
        _rss: Byte,
        memory_footprint: Byte,
        ctx: &mut ModelContext<Self>,
    ) {
        if self.has_emitted_memory_warning_event {
            return;
        }

        // Use footprint (not RSS) for the threshold so we catch memory
        // that has been swapped out or compressed by the OS.
        let footprint_bytes = memory_footprint.as_u64();
        let is_excessive = footprint_bytes >= MEMORY_USAGE_WARNING_THRESHOLD_BYTES;

        let Some(triggering_footprint_bytes) = self.pending_excessive_memory_footprint_bytes else {
            self.pending_excessive_memory_footprint_bytes = is_excessive.then_some(footprint_bytes);
            return;
        };
        self.pending_excessive_memory_footprint_bytes = None;

        if !is_excessive {
            log::info!(
                "Memory footprint returned to {footprint_bytes} bytes, back under the \
                 excessive-usage threshold, before confirming a spike that had crossed it at \
                 {triggering_footprint_bytes} bytes; skipping the excessive-memory-usage report \
                 for what looks like a transient spike."
            );
            return;
        }

        ctx.emit(SystemInfoEvent::MemoryUsageHigh);
        self.has_emitted_memory_warning_event = true;
    }

    /// Returns the pid of the current process.
    fn current_pid() -> sysinfo::Pid {
        sysinfo::get_current_pid().expect("Platform should support process IDs")
    }

    /// Returns the [`sysinfo::ProcessRefreshKind`] that should be used when
    /// retrieving information about the current process.
    fn refresh_kind() -> sysinfo::ProcessRefreshKind {
        sysinfo::ProcessRefreshKind::nothing()
            .with_memory()
            .with_cpu()
    }

    /// Returns the [`sysinfo::ProcessRefreshKind`] that should be used when enumerating the entire
    /// process table.
    ///
    /// This samples neither CPU nor memory: on Windows each per-process CPU sample issues an
    /// `NtQueryInformationProcess(ProcessCycleTime)` call, which forces a
    /// `KeFlushProcessWriteBuffers` inter-processor interrupt across every logical core. Across the
    /// whole process table that can pin all cores at `DISPATCH_LEVEL` long enough to trip the DPC
    /// watchdog and bugcheck high-core-count machines.
    #[cfg_attr(not(windows), allow(dead_code))]
    fn all_processes_refresh_kind() -> sysinfo::ProcessRefreshKind {
        sysinfo::ProcessRefreshKind::nothing()
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn refresh_all_processes(&mut self) {
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true, /* remove_dead_processes */
            Self::all_processes_refresh_kind(),
        );
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn processes_by_name<'a>(
        &'a self,
        name: &'a str,
    ) -> impl Iterator<Item = &'a sysinfo::Process> {
        self.system.processes_by_name(OsStr::new(name))
    }
}

impl Entity for SystemInfo {
    type Event = SystemInfoEvent;
}

impl SingletonEntity for SystemInfo {}

/// Helper structure for making resource usage reports.
struct ResourceUsageReporter {
    /// The number of blocks created since we last reported on resource usage
    /// statistics.
    blocks_created_since_last_report: usize,

    /// The time at which we sent the last report.
    time_last_report_sent: DateTime<Utc>,
}

impl ResourceUsageReporter {
    /// We won't produce a new report unless the user has created at least
    /// this many blocks since the last one.
    const MIN_BLOCKS_CREATED_PER_MEMORY_REPORT: usize = 5;
    /// We won't produce a new report unless at least this much time has
    /// passed since the last one.
    const MIN_DURATION_BETWEEN_MEMORY_REPORTS: chrono::Duration = chrono::Duration::hours(1);
    /// We won't produce a report unless the user has been active recently.
    const USER_RECENTLY_ACTIVE_INTERVAL: chrono::Duration = chrono::Duration::minutes(5);

    /// Handles creation of a block in a blocklist.
    fn handle_block_created(&mut self) {
        self.blocks_created_since_last_report += 1;
    }

    /// Sends a resource usage report if the required conditions are met.
    fn maybe_send_report(&mut self, ctx: &mut ModelContext<SystemInfo>) {
        if self.should_send_report() {
            // Immediately set the time at which we sent the last report, to
            // ensure we don't send two if it takes a little while to schedule
            // the background task below.
            self.time_last_report_sent = Utc::now();

            // We do this in a task callback to ensure that all terminal views
            // will be returned when iterating over the app context.  Without
            // this, we'll skip the active terminal view, as it has been
            // removed from the app context temporarily in order to provide
            // mutable access to it.
            ctx.spawn(futures::future::ready(()), |me, _, ctx| {
                me.refresh(ctx);
                let total_application_usage = me.used_memory();
                me.resource_usage_reporter.send_report(
                    total_application_usage,
                    me.stats.iter(),
                    ctx,
                );
            });
        }
    }

    /// Returns whether or not it's time to generate a report.
    fn should_send_report(&self) -> bool {
        // Don't send reports too frequently.
        if Utc::now().signed_duration_since(self.time_last_report_sent)
            < Self::MIN_DURATION_BETWEEN_MEMORY_REPORTS
        {
            return false;
        }

        // If we don't know when the user was last active, don't send a report.
        let Some(last_active_time) =
            DateTime::<Utc>::from_timestamp(App::last_active_timestamp(), 0)
        else {
            return false;
        };

        // Don't send a report unless the user has been active recently.
        if Utc::now().signed_duration_since(last_active_time) > Self::USER_RECENTLY_ACTIVE_INTERVAL
        {
            return false;
        }

        true
    }

    /// Sends a resource usage report.
    fn send_report<'a>(
        &mut self,
        _total_application_usage: Byte,
        _samples: impl Iterator<Item = &'a Sample>,
        _ctx: &mut AppContext,
    ) {
        // We send two different events at the moment, as one contains general
        // resource usage information, and one contains more detailed info
        // about memory consumption caused by the blocklist.
        //
        // TODO(vorporeal): Clean up the memory usage one, either eliminating it
        // or merging it into the general resource usage telemetry event.

        // Only send detailed memory usage reports in dogfood, for the time being.
        if ChannelState::channel().is_dogfood() {
            // Only send the detailed memory usage report if the user has created
            // enough blocks since the last detailed memory usage report.
            if self.blocks_created_since_last_report >= Self::MIN_BLOCKS_CREATED_PER_MEMORY_REPORT {
                self.blocks_created_since_last_report = 0;
            }
        }
    }
}

impl Default for ResourceUsageReporter {
    fn default() -> Self {
        Self {
            blocks_created_since_last_report: 0,
            time_last_report_sent: DateTime::UNIX_EPOCH,
        }
    }
}

/// A single resource usage sample point.
struct Sample {
    /// The CPU usage since the last sample, represented as a value in the
    /// range [0, num_cpus].
    cpu: f32,
}

/// A simple fixed-size circular buffer for storing resource usage sample
/// points.
struct StatsBuffer {
    stats: VecDeque<Sample>,
}

impl StatsBuffer {
    /// Constructs a new [`StatsBuffer`].
    fn new() -> Self {
        Self {
            stats: VecDeque::with_capacity(REPORT_SAMPLE_COUNT),
        }
    }

    /// Returns whether or not the buffer is full of samples.
    ///
    /// If true, adding a sample will replace the oldest sample in the buffer.
    fn is_full(&self) -> bool {
        self.stats.len() == self.stats.capacity()
    }

    /// Pushes a new sample into the buffer.  If the buffer is at capacity,
    /// the oldest sample will be removed to make room for the new one.
    fn push(&mut self, sample: Sample) {
        if self.is_full() {
            self.stats.pop_front();
        }
        self.stats.push_back(sample);
    }

    /// Returns an iterator over all samples in the buffer.
    fn iter(&self) -> impl Iterator<Item = &Sample> {
        self.stats.iter()
    }
}

impl Default for StatsBuffer {
    fn default() -> Self {
        Self::new()
    }
}
