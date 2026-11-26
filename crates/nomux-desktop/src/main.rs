//! The `nomux-desktop` binary.
//!
//! Options (not shown in a help text, for tests and measurements):
//!
//! - `--appearance dark|light`: a fixed theme instead of the theme of the
//!   system.
//! - `--reduce-motion`: no motion, also when the system setting allows it.
//! - `--measure <file>`: runs the measurement mode and writes its report
//!   to the file. `measure.rs` describes the scenario.
//! - `--capture <file>`: only in a build with the feature `frame-capture`.
//!   Writes the first frame with the text of a session to a PNG file and
//!   ends. `capture.rs` describes it.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use gpui::App;
use nomux_desktop::measure::Measurement;
use nomux_desktop::{Appearance, fonts};

struct Options {
    appearance: Appearance,
    reduce_motion: bool,
    measure: Option<PathBuf>,
    #[cfg(feature = "frame-capture")]
    capture: Option<PathBuf>,
}

fn parse_options() -> Result<Options, String> {
    let mut options = Options {
        appearance: Appearance::System,
        reduce_motion: false,
        measure: None,
        #[cfg(feature = "frame-capture")]
        capture: None,
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
            "--measure" => {
                let path = arguments.next().ok_or("--measure needs a file")?;
                options.measure = Some(PathBuf::from(path));
            }
            #[cfg(feature = "frame-capture")]
            "--capture" => {
                let path = arguments.next().ok_or("--capture needs a file")?;
                options.capture = Some(PathBuf::from(path));
            }
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok(options)
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
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let handle = match nomux_desktop::open_window(options.appearance, measurement.clone(), cx) {
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
