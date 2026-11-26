//! The `nomux-desktop` binary.
//!
//! Options (not shown in a help text, for tests and measurements):
//!
//! - `--appearance dark|light`: a fixed theme instead of the theme of the
//!   system.
//! - `--measure <file>`: runs the measurement mode and writes its report
//!   to the file. `measure.rs` describes the scenario.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use gpui::App;
use nomux_desktop::measure::Measurement;
use nomux_desktop::{Appearance, fonts};

struct Options {
    appearance: Appearance,
    measure: Option<PathBuf>,
}

fn parse_options() -> Result<Options, String> {
    let mut options = Options {
        appearance: Appearance::System,
        measure: None,
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
            "--measure" => {
                let path = arguments.next().ok_or("--measure needs a file")?;
                options.measure = Some(PathBuf::from(path));
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
        if let Some(measurement) = measurement {
            let _ = handle.update(cx, |_, window, cx| {
                window
                    .spawn(cx, async move |cx| {
                        measurement
                            .run(
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
        cx.activate(true);
    });
    ExitCode::SUCCESS
}
