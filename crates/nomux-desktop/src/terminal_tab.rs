//! The terminal tab: one attach to a session, the keys, the mouse, the IME,
//! the selection and the size.

use std::ops::Range;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, ClipboardItem, Context, EntityInputHandler, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement, Pixels, Render, ScrollDelta, ScrollEvent, SharedString, Styled,
    Subscription, Task, UTF16Selection, Window, actions, div,
};
use nomux_client::AttachEnd;
use nomux_desktop_model::motion::{MotionContext, blink_phase};
use nomux_desktop_theme::{ColorToken, DesktopTheme, DurationToken, LengthToken, TextStyle};
use nomux_term::Size;
use nomux_term_view::input::{self, MouseAction, MouseReport};
use nomux_term_view::{
    GridSelection, TerminalCaches, TerminalElement, TerminalFrame, TerminalPalette,
    TerminalSnapshot, TerminalStyle,
};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use crate::attach::{AttachRequest, SharedScreen, TabInput, TabUpdate};
use crate::host_connection::{HostConnection, HostRequest};
use crate::measure::Measurement;

actions!(
    terminal,
    [
        /// Copies the selected cells to the clipboard.
        Copy,
        /// Sends the text of the clipboard to the program.
        Paste
    ]
);

/// The key context of the terminal, for the key bindings of [`Copy`] and
/// [`Paste`].
pub(crate) const KEY_CONTEXT: &str = "Terminal";

/// The time without a change of the size before the session gets the new
/// size: a drag of the window edge sends one size, not one for each frame.
const RESIZE_DEBOUNCE: Duration = Duration::from_millis(100);

/// What the tab shows besides the screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TabStatus {
    /// The tab waits for its first layout or for the first screen.
    Attaching,
    /// The tab shows the session.
    Live,
    /// The connection is lost; the attach connects again.
    ConnectionLost,
    /// The attach ended.
    Ended(AttachEnd),
    /// The attach failed.
    Failed(String),
}

/// One terminal tab, attached to one session.
pub struct TerminalTab {
    session_id: String,
    connection: HostConnection,
    style: TerminalStyle,
    focus: FocusHandle,
    caches: TerminalCaches,
    attach: Option<Attached>,
    snapshot: Arc<TerminalSnapshot>,
    generation: u64,
    /// The rows of history above the screen that the view shows.
    scrolled: usize,
    /// Take a snapshot at the next frame also without a change of the
    /// screen: the scroll changed.
    capture: bool,
    selection: Option<GridSelection>,
    selecting: bool,
    marked_text: Option<String>,
    status: TabStatus,
    resize: Option<Task<()>>,
    /// `false` while another tab is active: the attach feeds the terminal,
    /// but the tab asks for no frame.
    visible: bool,
    blink: CursorBlink,
    measurement: Option<Measurement>,
    _subscriptions: Vec<Subscription>,
}

/// The blink of a cursor that the program asked to blink. It runs while the
/// tab has the focus, the window is visible and the user did not ask for
/// reduced motion, and it ends [`nomux_desktop_model::motion::BLINK_CYCLES`]
/// cycles after the last change.
struct CursorBlink {
    /// One cycle: `motion.indicator`.
    cycle: Duration,
    shown: bool,
    /// The snapshot generation at the last restart.
    generation: u64,
    _clock: Option<Task<()>>,
}

struct Attached {
    screen: Arc<SharedScreen>,
    input: UnboundedSender<TabInput>,
    size: Size,
    _updates: Task<()>,
}

impl TerminalTab {
    /// A tab for the session `session_id`. It attaches after its first
    /// layout, when it knows its size.
    pub fn new(
        session_id: String,
        connection: HostConnection,
        theme: &DesktopTheme,
        measurement: Option<Measurement>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        let subscriptions = vec![
            cx.on_focus_in(&focus, window, |tab, window, cx| {
                tab.report_focus(true, cx);
                tab.restart_blink(window, cx);
            }),
            cx.on_focus_out(&focus, window, |tab, _, window, cx| {
                tab.report_focus(false, cx);
                tab.restart_blink(window, cx);
            }),
            cx.observe_window_visibility(window, |tab, _, window, cx| {
                tab.restart_blink(window, cx)
            }),
        ];
        let blank = Size::new(1, Size::MIN_COLUMNS).expect("the minimum size is valid");
        Self {
            session_id,
            connection,
            style: terminal_style(theme),
            focus,
            caches: TerminalCaches::default(),
            attach: None,
            snapshot: Arc::new(TerminalSnapshot::capture(
                &nomux_term::Terminal::new(blank).screen(),
                0,
            )),
            generation: 0,
            scrolled: 0,
            capture: false,
            selection: None,
            selecting: false,
            marked_text: None,
            status: TabStatus::Attaching,
            resize: None,
            visible: true,
            blink: CursorBlink {
                cycle: theme.duration(DurationToken::MotionIndicator),
                shown: true,
                generation: 0,
                _clock: None,
            },
            measurement,
            _subscriptions: subscriptions,
        }
    }

    /// The session of the tab.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// The screen that the tab shows now.
    #[must_use]
    pub fn snapshot(&self) -> &TerminalSnapshot {
        &self.snapshot
    }

    /// Shows or hides the tab. A hidden tab keeps its attach: the
    /// connection thread feeds its terminal, and the tab paints nothing and
    /// runs no clock until it is shown again.
    pub fn set_visible(&mut self, visible: bool, window: &Window, cx: &mut Context<Self>) {
        if self.visible == visible {
            return;
        }
        self.visible = visible;
        self.restart_blink(window, cx);
        if visible {
            cx.notify();
        }
    }

    /// The size of the grid of the last layout, `None` before the first.
    #[must_use]
    pub fn grid_size(&self) -> Option<Size> {
        let layout = self.caches.layout.get();
        Size::new(layout.rows, layout.columns)
    }

    /// Attaches with `size` if the tab has no attach: a hidden tab has no
    /// layout of its own.
    pub fn ensure_attached(&mut self, size: Size, cx: &mut Context<Self>) {
        if self.attach.is_none() {
            self.start_attach(size, cx);
        }
    }

    /// Takes the colours and fonts of a new theme.
    pub fn set_theme(&mut self, theme: &DesktopTheme, cx: &mut Context<Self>) {
        self.style = terminal_style(theme);
        self.blink.cycle = theme.duration(DurationToken::MotionIndicator);
        self.caches.glyphs.borrow_mut().clear();
        cx.notify();
    }

    /// Sends bytes to the program of the session. Each input of the tab
    /// goes through this function.
    pub fn write(&mut self, bytes: Vec<u8>) {
        if bytes.is_empty() {
            return;
        }
        if let Some(measurement) = &self.measurement {
            measurement.input_sent(self.generation);
        }
        if self.scrolled > 0 {
            self.scrolled = 0;
            self.capture = true;
        }
        if let Some(attach) = &self.attach {
            let _ = attach.input.send(TabInput::Bytes(bytes));
        }
    }

    /// Attaches, or gives the session a new size after the debounce time.
    fn follow_layout(&mut self, cx: &mut Context<Self>) {
        let layout = self.caches.layout.get();
        // No layout yet: the element renders the view again after the first.
        let Some(size) = Size::new(layout.rows, layout.columns) else {
            return;
        };
        match &self.attach {
            None => self.start_attach(size, cx),
            Some(attach) if attach.size != size => {
                self.resize = Some(cx.spawn(async move |tab, cx| {
                    cx.background_executor().timer(RESIZE_DEBOUNCE).await;
                    let _ = tab.update(cx, |tab, _| tab.send_size(size));
                }));
            }
            Some(_) => {}
        }
    }

    fn send_size(&mut self, size: Size) {
        if let Some(attach) = &mut self.attach {
            attach.size = size;
            let _ = attach.input.send(TabInput::Resize(size));
        }
    }

    fn start_attach(&mut self, size: Size, cx: &mut Context<Self>) {
        let screen = Arc::new(SharedScreen::new(size));
        let (input, input_receiver) = unbounded_channel();
        let (update_sender, mut updates) = unbounded_channel();
        self.connection.send(HostRequest::Attach(AttachRequest {
            session_id: self.session_id.clone(),
            size,
            screen: screen.clone(),
            input: input_receiver,
            updates: update_sender,
        }));
        let updates_task = cx.spawn(async move |tab, cx| {
            while let Some(update) = updates.recv().await {
                if tab.update(cx, |tab, cx| tab.apply(update, cx)).is_err() {
                    return;
                }
            }
        });
        self.attach = Some(Attached {
            screen,
            input,
            size,
            _updates: updates_task,
        });
    }

    fn apply(&mut self, update: TabUpdate, cx: &mut Context<Self>) {
        self.status = match update {
            TabUpdate::Changed | TabUpdate::Reconnected => TabStatus::Live,
            TabUpdate::ConnectionLost => TabStatus::ConnectionLost,
            TabUpdate::Ended(end) => TabStatus::Ended(end),
            TabUpdate::Failed(message) => TabStatus::Failed(message),
        };
        if self.visible {
            cx.notify();
        }
    }

    /// Takes the new screen, once for each frame.
    fn take_snapshot(&mut self) {
        let always = std::mem::take(&mut self.capture);
        let Some(snapshot) = self
            .attach
            .as_ref()
            .and_then(|attach| attach.screen.take_snapshot(self.scrolled, always))
        else {
            return;
        };
        self.scrolled = snapshot.scrolled();
        self.snapshot = Arc::new(snapshot);
        self.generation += 1;
    }

    /// The setting of reduced motion changed.
    pub fn motion_changed(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.restart_blink(window, cx);
        cx.notify();
    }

    /// Shows the cursor and starts its blink again, if it blinks.
    fn restart_blink(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.blink.generation = self.generation;
        self.blink.shown = true;
        let blinking = self
            .snapshot
            .visible_cursor()
            .is_some_and(|cursor| cursor.blinking);
        let motion = MotionContext {
            window_visible: window.is_visible(),
            reduce_motion: cx.reduce_motion(),
        };
        if !(blinking && self.visible && self.focus.is_focused(window) && motion.allows_motion()) {
            self.blink._clock = None;
            return;
        }
        let started = Instant::now();
        let cycle = self.blink.cycle;
        self.blink._clock = Some(cx.spawn(async move |tab, cx| {
            loop {
                let phase = blink_phase(started.elapsed(), cycle);
                let updated = tab.update(cx, |tab, cx| {
                    if tab.blink.shown != phase.shown {
                        tab.blink.shown = phase.shown;
                        cx.notify();
                    }
                });
                let (Ok(()), Some(wait)) = (updated, phase.next_change) else {
                    return;
                };
                cx.background_executor().timer(wait).await;
            }
        }));
    }

    fn report_focus(&mut self, focused: bool, cx: &mut Context<Self>) {
        if let Some(bytes) = input::focus_bytes(focused, self.snapshot.modes()) {
            self.write(bytes.to_vec());
        }
        cx.notify();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(bytes) = input::key_bytes(&event.keystroke, self.snapshot.modes()) {
            self.selection = None;
            self.write(bytes);
            cx.stop_propagation();
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(selection) = &self.selection {
            cx.write_to_clipboard(ClipboardItem::new_string(selection.text(&self.snapshot)));
        }
    }

    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.write(input::paste_bytes(&text, self.snapshot.modes()));
        }
    }

    fn mouse_report(
        &self,
        action: MouseAction,
        position: gpui::Point<Pixels>,
        modifiers: gpui::Modifiers,
    ) -> Option<Vec<u8>> {
        // Shift gives the mouse to the selection, as in xterm.
        if modifiers.shift {
            return None;
        }
        let report = MouseReport {
            action,
            cell: self.caches.layout.get().cell_at(position),
            modifiers,
        };
        input::mouse_bytes(report, self.snapshot.modes())
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        if let Some(bytes) = button_code(event.button).and_then(|button| {
            self.mouse_report(MouseAction::Press(button), event.position, event.modifiers)
        }) {
            self.write(bytes);
            return;
        }
        if event.button == MouseButton::Left {
            let cell = self.caches.layout.get().cell_at(event.position);
            self.selection = Some(GridSelection {
                anchor: cell,
                head: cell,
            });
            self.selecting = true;
            cx.notify();
        }
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let button = event.pressed_button.and_then(button_code);
        if let Some(bytes) =
            self.mouse_report(MouseAction::Motion(button), event.position, event.modifiers)
        {
            self.write(bytes);
            return;
        }
        if self.selecting
            && let Some(selection) = &mut self.selection
        {
            selection.head = self.caches.layout.get().cell_at(event.position);
            cx.notify();
        }
    }

    fn on_mouse_up(&mut self, event: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(bytes) = button_code(event.button).and_then(|button| {
            self.mouse_report(
                MouseAction::Release(button),
                event.position,
                event.modifiers,
            )
        }) {
            self.write(bytes);
            return;
        }
        if self.selecting {
            self.selecting = false;
            if self
                .selection
                .is_some_and(|selection| selection.anchor == selection.head)
            {
                self.selection = None;
            }
            cx.notify();
        }
    }

    fn on_scroll(&mut self, event: &ScrollEvent, _: &mut Window, cx: &mut Context<Self>) {
        let cell_height = self.caches.layout.get().cell.height;
        let lines = match event.delta {
            ScrollDelta::Lines(lines) => lines.y,
            ScrollDelta::Pixels(pixels) if cell_height > Pixels::ZERO => pixels.y / cell_height,
            ScrollDelta::Pixels(_) => 0.0,
        };
        let up = lines > 0.0;
        let action = if up {
            MouseAction::WheelUp
        } else {
            MouseAction::WheelDown
        };
        // One step for each whole line, at least one for a small trackpad move.
        let steps = lines.abs().round().max(1.0) as usize;
        let modes = self.snapshot.modes();
        if input::reports_mouse(modes) && !event.modifiers.shift {
            for _ in 0..steps {
                if let Some(bytes) = self.mouse_report(action, event.position, event.modifiers) {
                    self.write(bytes);
                }
            }
        } else if let Some(bytes) = input::alternate_scroll_bytes(up, modes) {
            self.write(bytes.repeat(steps));
        } else {
            // The history: the snapshot clamps the scroll to its rows.
            self.scrolled = if up {
                self.scrolled.saturating_add(steps)
            } else {
                self.scrolled.saturating_sub(steps)
            };
            self.capture = true;
            cx.notify();
        }
    }

    /// The bounds of the cursor cell, for the window of an IME.
    fn cursor_bounds(&self) -> Bounds<Pixels> {
        let layout = self.caches.layout.get();
        let cursor = self.snapshot.cursor().position;
        let row = cursor
            .row
            .saturating_add(u16::try_from(self.scrolled).unwrap_or(u16::MAX));
        Bounds::new(
            layout.origin
                + gpui::point(
                    layout.cell.width * f32::from(cursor.column),
                    layout.cell.height * f32::from(row),
                ),
            layout.cell,
        )
    }

    fn status_text(&self) -> Option<String> {
        match &self.status {
            TabStatus::Attaching | TabStatus::Live => None,
            TabStatus::ConnectionLost => {
                Some("The connection is lost. Connecting again…".to_owned())
            }
            TabStatus::Ended(AttachEnd::Exited { code }) => Some(format!(
                "The command of the session ended with the exit code {code}."
            )),
            TabStatus::Ended(AttachEnd::Signaled { signal }) => Some(format!(
                "A signal ended the command of the session: {signal}."
            )),
            TabStatus::Ended(AttachEnd::Detached) => Some("The session was detached.".to_owned()),
            TabStatus::Ended(AttachEnd::Lost | AttachEnd::Unknown) => {
                Some("The session ended.".to_owned())
            }
            TabStatus::Failed(message) => Some(message.clone()),
        }
    }
}

/// The button number of a mouse report.
fn button_code(button: MouseButton) -> Option<u8> {
    match button {
        MouseButton::Left => Some(0),
        MouseButton::Middle => Some(1),
        MouseButton::Right => Some(2),
        MouseButton::Navigate(_) => None,
    }
}

/// The look of a terminal from the tokens of the theme.
#[must_use]
pub(crate) fn terminal_style(theme: &DesktopTheme) -> TerminalStyle {
    use ColorToken::{
        TerminalAnsiBlack, TerminalAnsiBlue, TerminalAnsiBrightBlack, TerminalAnsiBrightBlue,
        TerminalAnsiBrightCyan, TerminalAnsiBrightGreen, TerminalAnsiBrightMagenta,
        TerminalAnsiBrightRed, TerminalAnsiBrightWhite, TerminalAnsiBrightYellow, TerminalAnsiCyan,
        TerminalAnsiGreen, TerminalAnsiMagenta, TerminalAnsiRed, TerminalAnsiWhite,
        TerminalAnsiYellow,
    };
    let ansi = [
        TerminalAnsiBlack,
        TerminalAnsiRed,
        TerminalAnsiGreen,
        TerminalAnsiYellow,
        TerminalAnsiBlue,
        TerminalAnsiMagenta,
        TerminalAnsiCyan,
        TerminalAnsiWhite,
        TerminalAnsiBrightBlack,
        TerminalAnsiBrightRed,
        TerminalAnsiBrightGreen,
        TerminalAnsiBrightYellow,
        TerminalAnsiBrightBlue,
        TerminalAnsiBrightMagenta,
        TerminalAnsiBrightCyan,
        TerminalAnsiBrightWhite,
    ]
    .map(|token| theme.color(token));
    TerminalStyle {
        palette: TerminalPalette {
            ansi,
            foreground: theme.color(ColorToken::TerminalForeground),
            background: theme.color(ColorToken::TerminalBackground),
            cursor: theme.color(ColorToken::TerminalCursor),
            selection: theme.color(ColorToken::TerminalSelection),
        },
        font: theme.font(TextStyle::Code),
        font_size: theme.font_size(TextStyle::Code),
        padding: theme.length(LengthToken::Space2),
        cursor_width: theme.length(LengthToken::FocusRingWidth),
        line_width: theme.length(LengthToken::LineWidth),
    }
}

impl Focusable for TerminalTab {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TerminalTab {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.follow_layout(cx);
        self.take_snapshot();
        if self.blink.generation != self.generation {
            self.restart_blink(window, cx);
        }
        if let Some(measurement) = &self.measurement {
            measurement.frame(&self.caches, &self.snapshot, self.generation);
        }
        let frame = TerminalFrame {
            snapshot: self.snapshot.clone(),
            generation: self.generation,
            selection: self.selection,
            marked_text: self.marked_text.clone().map(SharedString::from),
            cursor_shown: self.blink.shown,
        };
        let element = TerminalElement::new(
            frame,
            self.style.clone(),
            self.caches.clone(),
            self.focus.clone(),
            cx.entity(),
        );
        div()
            .id("terminal")
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .bg(self.style.palette.background)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::paste))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::on_mouse_up))
            .on_scroll(cx.listener(Self::on_scroll))
            .child(element)
            .children(self.status_text().map(|text| {
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .p(self.style.padding)
                    .text_color(self.style.palette.foreground)
                    .child(text)
            }))
    }
}

impl EntityInputHandler for TerminalTab {
    fn text_for_range(
        &mut self,
        _range: Range<usize>,
        _adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        None
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_text
            .as_ref()
            .map(|text| 0..text.encode_utf16().count())
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.marked_text = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.marked_text = None;
        self.selection = None;
        self.write(text.as_bytes().to_vec());
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.marked_text = (!new_text.is_empty()).then(|| new_text.to_owned());
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(self.cursor_bounds())
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}
