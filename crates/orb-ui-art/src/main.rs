//! `ui-art gen [DIR]` writes the art set (default `assets/ui`);
//! `ui-art check [DIR]` renders the committed set at 1x, 2x and 3x with resvg
//! and writes the panel and a sheet of all parts as PNGs to `DIR/preview`;
//! `ui-art iconset DIR` renders the app icon at the sizes of a macOS `.iconset` (task-110);
//! `ui-art ico [FILE]` writes the Windows icon (default `assets/app_icon.ico`, task-111).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use orb_ui_art::{
    SCALES, check_parts, contact_sheet, generate, iconset_files, parts, render, render_size,
};

fn default_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/ui")
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_default();
    let explicit_dir = args.next().map(PathBuf::from);
    let dir = explicit_dir.clone().unwrap_or_else(default_dir);
    match cmd.as_str() {
        "gen" => {
            fs::create_dir_all(&dir).unwrap();
            let files = generate();
            // Remove parts that no longer exist.
            for e in fs::read_dir(&dir).unwrap().flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if name.ends_with(".svg") && !files.iter().any(|(f, _)| *f == name) {
                    fs::remove_file(e.path()).unwrap();
                    println!("removed {name}");
                }
            }
            for (name, content) in &files {
                fs::write(dir.join(name), content).unwrap();
            }
            println!("wrote {} files to {}", files.len(), dir.display());
            ExitCode::SUCCESS
        }
        "check" => {
            let all = parts::all_parts();
            let problems = check_parts(&all);
            for p in &problems {
                eprintln!("{p}");
            }
            let preview = dir.join("preview");
            fs::create_dir_all(&preview).unwrap();
            let panel = fs::read_to_string(dir.join("panel.svg")).expect("panel.svg");
            let sheet = contact_sheet(&all);
            for scale in SCALES {
                for (name, svg) in [("panel", &panel), ("parts", &sheet)] {
                    let px = render(svg, scale).expect("render");
                    let path = preview.join(format!("{name}_{scale}x.png"));
                    px.save_png(&path).unwrap();
                    println!("{} ({}x{})", path.display(), px.width(), px.height());
                }
            }
            println!(
                "{} parts x {} scales checked, {} problems",
                all.len(),
                SCALES.len(),
                problems.len()
            );
            if problems.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        "iconset" => {
            // The macOS icon (task-110): `iconutil -c icns DIR` turns this folder into an .icns.
            // DIR is required: the default folder is assets/ui, which must not get PNGs.
            let Some(dir) = explicit_dir else {
                eprintln!("usage: ui-art iconset DIR");
                return ExitCode::from(2);
            };
            let svg = fs::read_to_string(default_dir().join("app_icon.svg")).expect("app_icon.svg");
            fs::create_dir_all(&dir).unwrap();
            for (name, size) in iconset_files() {
                let px = render_size(&svg, size).expect("render");
                let path = dir.join(name);
                px.save_png(&path).unwrap();
                println!("{} ({}x{})", path.display(), px.width(), px.height());
            }
            ExitCode::SUCCESS
        }
        "ico" => {
            // The Windows icon (task-111): assets/app_icon.ico, embedded by orb-ui's build.rs.
            // The optional argument is the output file.
            let out = explicit_dir.unwrap_or_else(|| default_dir().join("../app_icon.ico"));
            let svg = fs::read_to_string(default_dir().join("app_icon.svg")).expect("app_icon.svg");
            let bytes = orb_ui_art::ico(&svg).expect("render");
            fs::write(&out, &bytes).unwrap();
            println!("{} ({} bytes)", out.display(), bytes.len());
            ExitCode::SUCCESS
        }
        "mockup" => {
            // (this fork) the shapes of mockup 1, 2, top, 303 or fx (`ui-art mockup 1 [FILE]`);
            // with FILE, also the whole mockup written back from its shapes (a check of
            // the cut)
            let m = match explicit_dir.as_deref().and_then(|p| p.to_str()) {
                Some("2") => orb_ui_art::mockup::Mockup::panel_2(),
                Some("top") => orb_ui_art::mockup::Mockup::top_bar(),
                Some("303") => orb_ui_art::mockup::Mockup::tb303(),
                Some("fx") => orb_ui_art::mockup::Mockup::panel_fx(),
                Some("808f") => orb_ui_art::mockup::Mockup::flip_808(),
                Some("909f") => orb_ui_art::mockup::Mockup::flip_909(),
                Some("mm") => orb_ui_art::mockup::Mockup::comp_m(),
                Some("sc") => orb_ui_art::mockup::Mockup::comp_sc(),
                Some("sd") => orb_ui_art::mockup::Mockup::comp_sc2(),
                _ => orb_ui_art::mockup::Mockup::panel_1(),
            };
            print!("{}", orb_ui_art::mockup::dump(m));
            if let Some(out) = args.next() {
                let b = orb_ui_art::mockup::Bx::new(0.0, 0.0, 624.0, 510.0);
                let svg = orb_ui_art::mockup::cut(&b, &m.shapes);
                render(&svg, 2).expect("render").save_png(&out).unwrap();
                eprintln!("{out}");
            }
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("usage: ui-art gen|check [DIR] | iconset DIR | ico [FILE]");
            ExitCode::from(2)
        }
    }
}
