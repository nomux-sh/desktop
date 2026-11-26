//! The measurement mode: the hidden option `--measure <file>`.
//!
//! The client runs a fixed scenario on the first session and writes the
//! numbers to the file as JSON, then ends:
//!
//! 1. The time from the process start to the first frame of the window,
//!    and to the first frame with the screen of the session. The process
//!    start is `NOMUX_DESKTOP_LAUNCH_NS` (nanoseconds after the Unix epoch,
//!    set by the launcher before `exec`) or else the start of `main`.
//! 2. After 60 s without input: the resident memory, and over 10 s the
//!    wake-ups for each second (context switches of all threads) and the
//!    CPU time of the process.
//! 3. Key to paint: 20 keys through the key path of the window, each from
//!    the dispatch to the end of the paint of the frame with the echo.
//! 4. A flood (`yes | head`): the paint time of the terminal element in each
//!    frame.

use std::cell::{Cell, RefCell};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gpui::{AsyncWindowContext, Entity, Keystroke};
use nomux_term_view::{PaintStats, TerminalCaches, TerminalSnapshot};

use crate::terminal_tab::TerminalTab;

const IDLE: Duration = Duration::from_secs(60);
const WAKE_UP_WINDOW: Duration = Duration::from_secs(10);
const KEYS: usize = 20;
const POLL: Duration = Duration::from_millis(2);
const FLOOD_COMMAND: &str = "yes 'nomux flood line 0123456789' | head -n 300000\r";
const FLOOD_QUIET: Duration = Duration::from_secs(2);

/// The state of the measurement mode, shared by the window and the terminal tab.
#[derive(Clone)]
pub struct Measurement(Rc<Inner>);

struct Inner {
    report: PathBuf,
    launched: Instant,
    window_frame: Cell<Option<Duration>>,
    content_frame: Cell<Option<Duration>>,
    last_paint: Cell<Option<Instant>>,
    pending_input: Cell<Option<(Instant, u64)>>,
    key_latencies: RefCell<Vec<Duration>>,
    recording: Cell<bool>,
    paints: RefCell<Vec<PaintStats>>,
}

impl Measurement {
    /// A measurement that writes to `report`. `main_started` is the start of
    /// `main`.
    #[must_use]
    pub fn new(report: PathBuf, main_started: Instant) -> Self {
        let launched = std::env::var("NOMUX_DESKTOP_LAUNCH_NS")
            .ok()
            .and_then(|nanoseconds| nanoseconds.parse::<u64>().ok())
            .and_then(|nanoseconds| {
                let since_launch = SystemTime::now()
                    .duration_since(UNIX_EPOCH + Duration::from_nanos(nanoseconds))
                    .ok()?;
                Instant::now().checked_sub(since_launch)
            })
            .unwrap_or(main_started);
        Self(Rc::new(Inner {
            report,
            launched,
            window_frame: Cell::new(None),
            content_frame: Cell::new(None),
            last_paint: Cell::new(None),
            pending_input: Cell::new(None),
            key_latencies: RefCell::default(),
            recording: Cell::new(false),
            paints: RefCell::default(),
        }))
    }

    /// The first frame of the window was painted.
    pub fn window_painted(&self) {
        if self.0.window_frame.get().is_none() {
            self.0.window_frame.set(Some(self.0.launched.elapsed()));
        }
    }

    /// The tab sent input when it showed the frame `generation`.
    pub fn input_sent(&self, generation: u64) {
        if self.0.pending_input.get().is_none() {
            self.0.pending_input.set(Some((Instant::now(), generation)));
        }
    }

    /// The tab renders a frame. The paint of the last frame is in `caches`.
    pub fn frame(&self, caches: &TerminalCaches, snapshot: &TerminalSnapshot, generation: u64) {
        let Some(stats) = caches.stats.get() else {
            return;
        };
        if self.0.last_paint.get() == Some(stats.painted_at) {
            return;
        }
        self.0.last_paint.set(Some(stats.painted_at));
        if self.0.content_frame.get().is_none() && generation > 0 && stats.generation > 0 {
            let has_text = (0..snapshot.screen_rows())
                .any(|row| snapshot.row(row).iter().any(|cell| cell.character != ' '));
            if has_text {
                self.0
                    .content_frame
                    .set(Some(stats.painted_at.duration_since(self.0.launched)));
            }
        }
        if let Some((sent, sent_generation)) = self.0.pending_input.get()
            && stats.generation > sent_generation
            && stats.painted_at > sent
        {
            self.0
                .key_latencies
                .borrow_mut()
                .push(stats.painted_at.duration_since(sent));
            self.0.pending_input.set(None);
        }
        if self.0.recording.get() {
            self.0.paints.borrow_mut().push(stats);
        }
    }

    /// Runs the scenario on `tab` and ends the application.
    pub async fn run(
        self,
        tab: impl Fn(&mut AsyncWindowContext) -> Option<Entity<TerminalTab>>,
        cx: &mut AsyncWindowContext,
    ) {
        let timer =
            |cx: &mut AsyncWindowContext, duration| cx.background_executor().timer(duration);
        let tab = loop {
            if let Some(tab) = tab(cx)
                && tab.read_with(cx, |tab, _| {
                    tab.snapshot()
                        .row(0)
                        .iter()
                        .any(|cell| cell.character != ' ')
                })
            {
                break tab;
            }
            timer(cx, Duration::from_millis(50)).await;
        };
        timer(cx, IDLE).await;
        let resident = resident_kilobytes();
        let switches_before = context_switches();
        let cpu_before = cpu_ticks();
        timer(cx, WAKE_UP_WINDOW).await;
        let switches = context_switches().saturating_sub(switches_before);
        let idle = Idle {
            resident_kilobytes: resident,
            switches,
            cpu_ticks: cpu_ticks().saturating_sub(cpu_before),
        };

        for _ in 0..KEYS {
            self.0.pending_input.set(None);
            let _ = cx.update(|window, cx| {
                window.dispatch_keystroke(Keystroke::parse("x").expect("a valid key"), cx);
            });
            let started = Instant::now();
            while self.0.pending_input.get().is_some() && started.elapsed() < Duration::from_secs(1)
            {
                tab.update(cx, |_, cx| cx.notify());
                timer(cx, POLL).await;
            }
            timer(cx, Duration::from_millis(100)).await;
        }
        tab.update(cx, |tab, _| tab.write(b"\x15".to_vec()));
        timer(cx, Duration::from_millis(200)).await;

        self.0.recording.set(true);
        let flood_started = Instant::now();
        tab.update(cx, |tab, _| tab.write(FLOOD_COMMAND.as_bytes().to_vec()));
        let mut last_change = Instant::now();
        let mut last_count = 0;
        while last_change.elapsed() < FLOOD_QUIET {
            timer(cx, Duration::from_millis(100)).await;
            let count = self.0.paints.borrow().len();
            if count != last_count {
                last_count = count;
                last_change = Instant::now();
            }
        }
        self.0.recording.set(false);
        let flood = last_change.duration_since(flood_started);

        let report = self.report(&idle, flood);
        if let Err(error) = std::fs::write(&self.0.report, report) {
            eprintln!(
                "nomux-desktop: cannot write {}: {error}",
                self.0.report.display()
            );
        }
        let _ = cx.update(|_, cx| cx.quit());
    }

    fn report(&self, idle: &Idle, flood: Duration) -> String {
        let milliseconds = |duration: Option<Duration>| {
            duration.map_or_else(
                || "null".to_owned(),
                |duration| format!("{:.1}", duration.as_secs_f64() * 1000.0),
            )
        };
        let mut latencies = self.0.key_latencies.borrow().clone();
        latencies.sort();
        let mut paints: Vec<Duration> = self
            .0
            .paints
            .borrow()
            .iter()
            .map(|stats| stats.duration)
            .collect();
        paints.sort();
        let percentile = |values: &[Duration], percent: usize| {
            values
                .get((values.len() * percent / 100).min(values.len().saturating_sub(1)))
                .copied()
        };
        let mut json = String::from("{\n");
        let _ = writeln!(
            json,
            "  \"first_window_frame_ms\": {},",
            milliseconds(self.0.window_frame.get())
        );
        let _ = writeln!(
            json,
            "  \"first_content_frame_ms\": {},",
            milliseconds(self.0.content_frame.get())
        );
        let _ = writeln!(
            json,
            "  \"idle_resident_mb\": {},",
            idle.resident_kilobytes.map_or_else(
                || "null".to_owned(),
                |kilobytes| format!("{:.1}", kilobytes as f64 / 1024.0)
            )
        );
        let _ = writeln!(
            json,
            "  \"idle_wake_ups_per_second\": {:.2},",
            idle.switches as f64 / WAKE_UP_WINDOW.as_secs_f64()
        );
        // /proc counts CPU time in ticks of 1/100 s.
        let _ = writeln!(
            json,
            "  \"idle_cpu_percent\": {:.2},",
            idle.cpu_ticks as f64 / WAKE_UP_WINDOW.as_secs_f64()
        );
        let _ = writeln!(json, "  \"key_samples\": {},", latencies.len());
        let _ = writeln!(
            json,
            "  \"key_to_paint_p50_ms\": {},",
            milliseconds(percentile(&latencies, 50))
        );
        let _ = writeln!(
            json,
            "  \"key_to_paint_max_ms\": {},",
            milliseconds(latencies.last().copied())
        );
        let _ = writeln!(json, "  \"flood_frames\": {},", paints.len());
        let _ = writeln!(json, "  \"flood_ms\": {},", milliseconds(Some(flood)));
        let _ = writeln!(
            json,
            "  \"flood_paint_p50_ms\": {},",
            milliseconds(percentile(&paints, 50))
        );
        let _ = writeln!(
            json,
            "  \"flood_paint_p95_ms\": {},",
            milliseconds(percentile(&paints, 95))
        );
        let _ = writeln!(
            json,
            "  \"flood_paint_max_ms\": {}",
            milliseconds(paints.last().copied())
        );
        json.push_str("}\n");
        json
    }
}

/// What the measurement mode measured without input.
struct Idle {
    resident_kilobytes: Option<u64>,
    switches: u64,
    cpu_ticks: u64,
}

/// The user and system CPU time of the process: fields 14 and 15 of
/// `/proc/self/stat`, after the command name in parentheses.
fn cpu_ticks() -> u64 {
    let Ok(stat) = std::fs::read_to_string("/proc/self/stat") else {
        return 0;
    };
    let Some((_, fields)) = stat.rsplit_once(')') else {
        return 0;
    };
    fields
        .split_whitespace()
        .skip(11)
        .take(2)
        .filter_map(|field| field.parse::<u64>().ok())
        .sum()
}

/// The resident memory of the process: `VmRSS` of `/proc/self/status`.
fn resident_kilobytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))?
        .trim()
        .trim_end_matches("kB")
        .trim()
        .parse()
        .ok()
}

/// The context switches of all threads of the process: each one is a
/// wake-up of a thread.
fn context_switches() -> u64 {
    let Ok(tasks) = std::fs::read_dir("/proc/self/task") else {
        return 0;
    };
    tasks
        .filter_map(Result::ok)
        .filter_map(|task| std::fs::read_to_string(task.path().join("status")).ok())
        .flat_map(|status| {
            status
                .lines()
                .filter(|line| line.contains("ctxt_switches:"))
                .filter_map(|line| line.split_whitespace().nth(1)?.parse::<u64>().ok())
                .collect::<Vec<_>>()
        })
        .sum()
}
