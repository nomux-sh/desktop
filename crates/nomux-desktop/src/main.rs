//! The `nomux-desktop` binary.
//!
//! Options (not shown in a help text, for tests and measurements):
//!
//! - `--appearance dark|light`: a fixed theme instead of the theme of the
//!   system.
//! - `--reduce-motion`: no motion, also when the system setting allows it.
//! - `--window-size <width>x<height>`: the size of the window in logical
//!   pixels instead of 1280x800.
//! - `--measure <file>`: runs the measurement mode and writes its report
//!   to the file. `measure.rs` describes the scenario.
//! - `--capture <file>`: only in a build with the feature `frame-capture`.
//!   Writes the first frame with the text of a session to a PNG file and
//!   ends. `capture.rs` describes it.
//! - `--capture-menu session|tab|project`: with `--capture`, shows a context
//!   menu in the frame.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use gpui::App;
use nomux_desktop::measure::Measurement;
use nomux_desktop::{Appearance, fonts};

struct Options {
    appearance: Appearance,
    reduce_motion: bool,
    window_size: (f32, f32),
    measure: Option<PathBuf>,
    #[cfg(feature = "frame-capture")]
    capture: Option<PathBuf>,
    #[cfg(feature = "frame-capture")]
    capture_menu: Option<nomux_desktop::capture::CaptureMenu>,
}

fn parse_options() -> Result<Options, String> {
    let mut options = Options {
        appearance: Appearance::System,
        reduce_motion: false,
        window_size: nomux_desktop::WINDOW_SIZE,
        measure: None,
        #[cfg(feature = "frame-capture")]
        capture: None,
        #[cfg(feature = "frame-capture")]
        capture_menu: None,
    };
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--appearance" => {
                options.appearance = match arguments.next().as_deref() {
                    Some("dark") => Appearance::Dark,
                    Some("light") => Appearance::Light,
                    _ => return Err("--appearance needs `dark` or `light`".to_owned()),
                };
            }
            "--reduce-motion" => options.reduce_motion = true,
            "--window-size" => {
                options.window_size = arguments
                    .next()
                    .as_deref()
                    .and_then(parse_window_size)
                    .ok_or("--window-size needs <width>x<height>, for example 1100x700")?;
            }
            "--measure" => {
                let path = arguments.next().ok_or("--measure needs a file")?;
                options.measure = Some(PathBuf::from(path));
            }
            #[cfg(feature = "frame-capture")]
            "--capture" => {
                let path = arguments.next().ok_or("--capture needs a file")?;
                options.capture = Some(PathBuf::from(path));
            }
            #[cfg(feature = "frame-capture")]
            "--capture-menu" => {
                options.capture_menu = Some(
                    arguments
                        .next()
                        .as_deref()
                        .and_then(nomux_desktop::capture::CaptureMenu::parse)
                        .ok_or("--capture-menu needs `session`, `tab` or `project`")?,
                );
            }
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok(options)
}

/// `<width>x<height>` in logical pixels, each one at least 1.
fn parse_window_size(text: &str) -> Option<(f32, f32)> {
    let (width, height) = text.split_once('x')?;
    let width: u16 = width.parse().ok()?;
    let height: u16 = height.parse().ok()?;
    (width > 0 && height > 0).then(|| (f32::from(width), f32::from(height)))
}

fn main() -> ExitCode {
    let started = Instant::now();
    let options = match parse_options() {
        Ok(options) => options,
        Err(message) => {
            eprintln!("nomux-desktop: {message}");
            return ExitCode::FAILURE;
        }
    };
    let measurement = options
        .measure
        .map(|report| Measurement::new(report, started));
    gpui_platform::application().run(move |cx: &mut App| {
        if let Err(error) = fonts::load(cx) {
            eprintln!("nomux-desktop: the shipped fonts cannot be loaded: {error}");
        }
        nomux_desktop::bind_keys(cx);
        nomux_desktop::install_menus(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let handle = match nomux_desktop::open_window(
            options.appearance,
            options.window_size,
            measurement.clone(),
            cx,
        ) {
            Ok(window) => window,
            Err(error) => {
                eprintln!("nomux-desktop: the window cannot open: {error}");
                cx.quit();
                return;
            }
        };
        if options.reduce_motion {
            cx.set_reduce_motion(true);
        } else {
            nomux_desktop::reduced_motion::follow_system(handle, cx);
        }
        #[cfg(feature = "frame-capture")]
        if let Some(file) = options.capture.clone() {
            let _ = handle.update(cx, |_, window, cx| {
                window
                    .spawn(cx, async move |cx| {
                        nomux_desktop::capture::run(
                            file,
                            move |cx| {
                                handle
                                    .read_with(cx, |workbench, _| workbench.tab().cloned())
                                    .ok()
                                    .flatten()
                            },
                            move |cx| {
                                if let Some(menu) = options.capture_menu {
                                    let _ = handle.update(cx, |workbench, _, cx| {
                                        workbench.open_menu_for_capture(menu, cx);
                                    });
                                }
                            },
                            cx,
                        )
                        .await;
                    })
                    .detach();
            });
        }
        if let Some(measurement) = measurement {
            let _ = handle.update(cx, |_, window, cx| {
                window
                    .spawn(cx, async move |cx| {
                        measurement.run(handle, cx).await;
                    })
                    .detach();
            });
        }
        cx.activate(true);
    });
    ExitCode::SUCCESS
}
