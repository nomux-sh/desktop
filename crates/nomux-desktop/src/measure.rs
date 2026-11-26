//! The measurement mode: the hidden option `--measure <file>`.
//!
//! The client runs a fixed scenario on the first session and writes the
//! numbers to the file as JSON, then ends:
//!
//! 1. The time from the process start to the first frame of the window,
//!    and to the first frame with the screen of the session. The process
//!    start is `NOMUX_DESKTOP_LAUNCH_NS` (nanoseconds after the Unix epoch,
//!    set by the launcher before `exec`) or else the start of `main`.
//! 2. Four idle phases (the last two run after the steps 3 and 4). Each one gives, for its window, the wake-ups for
//!    each second (context switches of all threads), the CPU time of the
//!    process, and the renders of the terminal tab and of the sidebar rows:
//!    - static: after 60 s without input, over 10 s, with the resident
//!      memory at its start;
//!    - blinking cursor: the shell selects a blinking cursor, over 5 s,
//!      then a steady cursor again;
//!    - hidden output (after the steps 3 and 4): the client starts seven
//!      sessions in new tabs, as Cmd-T does, and each one prints 50 lines
//!      each second; the first tab is active again, over 10 s;
//!    - working row: the shell starts a second session that reports
//!      `working` with `nomux status set`, over 10 s.
//!
//!    The wake-ups are counted for all threads and for the main thread
//!    alone: a software renderer wakes many threads for each frame.
//!    Wake-ups and CPU time come from `/proc`: on other systems they are
//!    `null`, and the memory too.
//! 3. Key to paint: 20 keys through the key path of the window, each from
//!    the dispatch to the end of the paint of the frame with the echo.
//! 4. A flood (`yes | head`): the paint time of the terminal element in each
//!    frame.

use std::cell::{Cell, RefCell};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gpui::{AsyncWindowContext, Keystroke, WindowHandle};
use nomux_term_view::{PaintStats, TerminalCaches, TerminalSnapshot};

use crate::workbench::Workbench;

const IDLE: Duration = Duration::from_secs(60);
const STATIC_WINDOW: Duration = Duration::from_secs(10);
const BLINK_WINDOW: Duration = Duration::from_secs(5);
const WORKING_WINDOW: Duration = Duration::from_secs(10);
/// The time for a command of the shell to take effect.
const SETTLE: Duration = Duration::from_secs(3);
const BLINKING_CURSOR_COMMAND: &str = "printf '\\033[1 q'\r";
const STEADY_CURSOR_COMMAND: &str = "printf '\\033[2 q'\r";
/// The tabs that print while they are hidden.
const HIDDEN_TABS: usize = 7;
/// Prints 50 lines each second.
const OUTPUT_COMMAND: &str = "while :; do date; sleep 0.02; done\r";
const HIDDEN_OUTPUT_WINDOW: Duration = Duration::from_secs(10);
const WORKING_ROW_COMMAND: &str = "nomux new -- sh -c \
    'nomux status set working measuring; exec sleep 100000' >/dev/null\r";
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
    tab_renders: Cell<u64>,
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
            tab_renders: Cell::new(0),
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
        self.0.tab_renders.set(self.0.tab_renders.get() + 1);
        let Some(stats) = caches.stats.get() else {
            return;
        };
        if self.0.last_paint.get() == Some(stats.painted_at) {
            return;
        }
        self.0.last_paint.set(Some(stats.painted_at));
        if self.0.content_frame.get().is_none()
            && generation > 0
            && stats.generation > 0
            && has_text(snapshot)
        {
            self.0
                .content_frame
                .set(Some(stats.painted_at.duration_since(self.0.launched)));
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

    /// Runs the scenario in `window` and ends the application.
    pub async fn run(self, window: WindowHandle<Workbench>, cx: &mut AsyncWindowContext) {
        let timer =
            |cx: &mut AsyncWindowContext, duration| cx.background_executor().timer(duration);
        let tab = loop {
            let tab = window
                .read_with(cx, |workbench, _| workbench.tab().cloned())
                .ok()
                .flatten();
            if let Some(tab) = tab
                && tab.read_with(cx, |tab, _| has_text(tab.snapshot()))
            {
                break tab;
            }
            timer(cx, Duration::from_millis(50)).await;
        };
        timer(cx, IDLE).await;
        let resident = resident_kilobytes();
        let static_idle = self.idle(window, STATIC_WINDOW, cx).await;
        tab.update(cx, |tab, _| {
            tab.write(BLINKING_CURSOR_COMMAND.as_bytes().to_vec());
        });
        timer(cx, Duration::from_millis(500)).await;
        let blinking = self.idle(window, BLINK_WINDOW, cx).await;
        tab.update(cx, |tab, _| {
            tab.write(STEADY_CURSOR_COMMAND.as_bytes().to_vec());
        });
        timer(cx, Duration::from_millis(500)).await;

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

        for _ in 0..HIDDEN_TABS {
            self.open_output_tab(window, cx).await;
        }
        let _ = window.update(cx, |workbench, window, cx| {
            workbench.activate_tab(0, window, cx);
        });
        timer(cx, SETTLE).await;
        let tabs = window
            .read_with(cx, |workbench, _| workbench.tab_count())
            .unwrap_or(0);
        let hidden_output = self.idle(window, HIDDEN_OUTPUT_WINDOW, cx).await;

        tab.update(cx, |tab, _| {
            tab.write(WORKING_ROW_COMMAND.as_bytes().to_vec());
        });
        timer(cx, SETTLE).await;
        let working = self.idle(window, WORKING_WINDOW, cx).await;

        let report = self.report(
            resident,
            &[static_idle, blinking, hidden_output, working],
            tabs,
            flood,
        );
        if let Err(error) = std::fs::write(&self.0.report, report) {
            eprintln!(
                "nomux-desktop: cannot write {}: {error}",
                self.0.report.display()
            );
        }
        let _ = cx.update(|_, cx| cx.quit());
    }

    /// Starts a session in a new tab, as Cmd-T does, and starts the output
    /// in it when its shell shows text.
    async fn open_output_tab(&self, window: WindowHandle<Workbench>, cx: &mut AsyncWindowContext) {
        let count = |cx: &mut AsyncWindowContext| {
            window
                .read_with(cx, |workbench, _| workbench.tab_count())
                .unwrap_or(0)
        };
        let before = count(cx);
        let _ = window.update(cx, |workbench, _, _| workbench.new_session());
        let started = Instant::now();
        while started.elapsed() < SETTLE * 3 {
            cx.background_executor()
                .timer(Duration::from_millis(50))
                .await;
            let tab = window
                .read_with(cx, |workbench, _| workbench.tab().cloned())
                .ok()
                .flatten();
            if count(cx) > before
                && let Some(tab) = tab
                && tab.read_with(cx, |tab, _| has_text(tab.snapshot()))
            {
                tab.update(cx, |tab, _| tab.write(OUTPUT_COMMAND.as_bytes().to_vec()));
                return;
            }
        }
    }

    /// Counts what the client does in `duration` without input.
    async fn idle(
        &self,
        window: WindowHandle<Workbench>,
        duration: Duration,
        cx: &mut AsyncWindowContext,
    ) -> Idle {
        let row_paints = |cx: &mut AsyncWindowContext| {
            window
                .read_with(cx, |workbench, cx| {
                    workbench
                        .sidebar()
                        .read(cx)
                        .rows()
                        .iter()
                        .map(|row| row.read(cx).paints())
                        .sum::<u64>()
                })
                .unwrap_or(0)
        };
        let switches = context_switches();
        let main_switches = main_thread_switches();
        let cpu = cpu_ticks();
        let rows = row_paints(cx);
        let tab_renders = self.0.tab_renders.get();
        cx.background_executor().timer(duration).await;
        let seconds = duration.as_secs_f64();
        Idle {
            wake_ups_per_second: switches
                .zip(context_switches())
                .map(|(before, after)| after.saturating_sub(before) as f64 / seconds),
            main_thread_wake_ups_per_second: main_switches
                .zip(main_thread_switches())
                .map(|(before, after)| after.saturating_sub(before) as f64 / seconds),
            // /proc counts CPU time in ticks of 1/100 s.
            cpu_percent: cpu
                .zip(cpu_ticks())
                .map(|(before, after)| after.saturating_sub(before) as f64 / seconds),
            tab_renders: self.0.tab_renders.get() - tab_renders,
            row_paints: row_paints(cx).saturating_sub(rows),
        }
    }

    fn report(
        &self,
        resident_kilobytes: Option<u64>,
        idle: &[Idle; 4],
        tabs: usize,
        flood: Duration,
    ) -> String {
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
            resident_kilobytes.map_or_else(
                || "null".to_owned(),
                |kilobytes| format!("{:.1}", kilobytes as f64 / 1024.0)
            )
        );
        let number = |value: Option<f64>| {
            value.map_or_else(|| "null".to_owned(), |value| format!("{value:.2}"))
        };
        let _ = writeln!(json, "  \"tabs_in_hidden_output_phase\": {tabs},");
        for (name, idle) in ["static", "blinking_cursor", "hidden_output", "working_row"]
            .iter()
            .zip(idle)
        {
            let _ = writeln!(
                json,
                "  \"idle_{name}\": {{ \"wake_ups_per_second\": {}, \
                 \"main_thread_wake_ups_per_second\": {}, \"cpu_percent\": {}, \
                 \"tab_renders\": {}, \"row_paints\": {} }},",
                number(idle.wake_ups_per_second),
                number(idle.main_thread_wake_ups_per_second),
                number(idle.cpu_percent),
                idle.tab_renders,
                idle.row_paints
            );
        }
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

/// What the measurement mode measured in one idle phase.
struct Idle {
    wake_ups_per_second: Option<f64>,
    main_thread_wake_ups_per_second: Option<f64>,
    cpu_percent: Option<f64>,
    tab_renders: u64,
    row_paints: u64,
}

/// `true` if a row of the screen shows a character. The first row of a
/// shell can be empty.
fn has_text(snapshot: &TerminalSnapshot) -> bool {
    (0..snapshot.screen_rows())
        .any(|row| snapshot.row(row).iter().any(|cell| cell.character != ' '))
}

/// The user and system CPU time of the process: fields 14 and 15 of
/// `/proc/self/stat`, after the command name in parentheses.
fn cpu_ticks() -> Option<u64> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    let (_, fields) = stat.rsplit_once(')')?;
    Some(
        fields
            .split_whitespace()
            .skip(11)
            .take(2)
            .filter_map(|field| field.parse::<u64>().ok())
            .sum(),
    )
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

/// The context switches of the main thread: its task has the identifier
/// of the process.
fn main_thread_switches() -> Option<u64> {
    let status =
        std::fs::read_to_string(format!("/proc/self/task/{}/status", std::process::id())).ok()?;
    switches_of(&status)
}

/// The sum of the voluntary and involuntary context switches of a status
/// file of `/proc`.
fn switches_of(status: &str) -> Option<u64> {
    Some(
        status
            .lines()
            .filter(|line| line.contains("ctxt_switches:"))
            .filter_map(|line| line.split_whitespace().nth(1)?.parse::<u64>().ok())
            .sum(),
    )
}

/// The context switches of all threads of the process: each one is a
/// wake-up of a thread.
fn context_switches() -> Option<u64> {
    let tasks = std::fs::read_dir("/proc/self/task").ok()?;
    Some(
        tasks
            .filter_map(Result::ok)
            .filter_map(|task| std::fs::read_to_string(task.path().join("status")).ok())
            .filter_map(|status| switches_of(&status))
            .sum(),
    )
}
