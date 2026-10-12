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
mod settings_ui;
mod telemetry;
mod theme;
mod views;

use std::path::PathBuf;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn AttachConsole(process_id: u32) -> i32;
    fn GetStdHandle(which: u32) -> isize;
}

/// Prints to stdout; in the GUI subsystem with no stdout (launched from a console without
/// redirection) it attaches to the parent console first.
fn console_print(s: &str) {
    use std::io::Write;
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    let handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
    if handle == 0 || handle == -1 {
        unsafe { AttachConsole(u32::MAX) };
        if let Ok(mut f) = std::fs::OpenOptions::new().write(true).open("CONOUT$") {
            let _ = f.write_all(s.as_bytes());
        }
    } else {
        print!("{s}");
        let _ = std::io::stdout().flush();
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

/// The same artwork as the exe's embedded icon, for the window/taskbar (eframe would otherwise show its own).
fn window_icon() -> Option<eframe::egui::IconData> {
    eframe::icon_data::from_png_bytes(include_bytes!("../icon/icon-512.png")).ok()
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = run_cli(&args) {
        std::process::exit(code);
    }

    let settings = settings::load();
    let enabled = settings.telemetry;
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        telemetry::log_panic(&settings::dir(), &info.to_string(), enabled);
        default_hook(info);
    }));

    let path = args.first().map(PathBuf::from);
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size(
            settings
                .window_size()
                .map_or([900.0, 650.0], |(w, h)| [w, h]),
        )
        .with_drag_and_drop(true);
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "TurboText",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc, path)))),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn window_icon_decodes() {
        let icon = super::window_icon().expect("embedded icon must decode");
        assert_eq!((icon.width, icon.height), (512, 512));
        assert_eq!(icon.rgba.len(), 512 * 512 * 4);
        assert!(
            icon.rgba.chunks(4).any(|px| px[3] > 0),
            "icon must not be fully transparent"
        );
    }
}
