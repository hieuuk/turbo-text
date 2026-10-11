// Release builds are GUI-subsystem (no console flash on "Open with"); CLI subcommands attach to the parent console.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod assoc;
mod editor;
mod find;
mod format;
mod highlight;
mod io;
mod settings;
mod telemetry;
mod views;

use std::path::PathBuf;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn AttachConsole(process_id: u32) -> i32;
}

/// Prints to the parent console even in the GUI subsystem.
fn console_print(s: &str) {
    use std::io::Write;
    unsafe { AttachConsole(u32::MAX) };
    match std::fs::OpenOptions::new().write(true).open("CONOUT$") {
        Ok(mut f) => {
            let _ = f.write_all(s.as_bytes());
        }
        Err(_) => print!("{s}"),
    }
}

fn run_cli(args: &[String]) -> Option<i32> {
    let exts = || {
        args[1..]
            .iter()
            .map(|e| assoc::normalize_ext(e))
            .collect::<Vec<_>>()
    };
    match args.first()?.as_str() {
        "--associate" | "--unassociate" => {
            let on = args[0] == "--associate";
            let list = exts();
            let r = if on {
                assoc::associate(&list)
            } else {
                assoc::unassociate(&list)
            };
            match r {
                Ok(()) => {
                    console_print(&format!(
                        "{} {}\n",
                        if on { "associated" } else { "unassociated" },
                        list.join(" ")
                    ));
                    if on {
                        assoc::open_default_apps();
                    }
                    Some(0)
                }
                Err(e) => {
                    console_print(&format!("error: {e}\n"));
                    Some(1)
                }
            }
        }
        "--stats" => {
            if args.iter().any(|a| a == "--clear") {
                telemetry::clear(&settings::dir());
                console_print("usage log cleared\n");
            } else {
                console_print(&telemetry::stats(&settings::dir()));
            }
            Some(0)
        }
        _ => None,
    }
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = run_cli(&args) {
        std::process::exit(code);
    }

    let enabled = settings::load().telemetry;
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        telemetry::log_panic(&settings::dir(), &info.to_string(), enabled);
        default_hook(info);
    }));

    let path = args.first().map(PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([900.0, 650.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "TurboText",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc, path)))),
    )
}
