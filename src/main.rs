use std::env;
use std::process::exit;
use crate::pl0_vm::{load_vm_from_file};
use rust_i18n::t;

const VERSION: &str = env!("CARGO_PKG_VERSION");

rust_i18n::i18n!("locales", fallback = "en");

mod pl0_vm;
mod opcodes;

fn main() {
    let mut analyze_only = false;
    let mut debug = false;
    let mut help = false;
    let mut filename_arg: Option<&str> = None;
    let args: Vec<String> = env::args().collect();

    let locale = sys_locale::get_locale().unwrap_or_else(|| "en".to_string());
    let lang = &locale[0..2];
    rust_i18n::set_locale(lang);

    args.iter().skip(1).for_each(|arg| {
        if arg == "--analyze" || arg == "-a" {
            analyze_only = true;
        } else if arg == "--debug" || arg == "-d" {
            debug = true;
        } else if arg == "--help" || arg == "-h" {
            help = true;
        } else if arg == "--lang=de" {
            rust_i18n::set_locale("de");
        } else if arg == "--lang=en" {
            rust_i18n::set_locale("en");
        } else {
            filename_arg = Some(arg);
        }
    });

    if args.len() <= 1 || help {
        print!("{}", t!("help", version = VERSION));
        exit(0);
    }

    let Some(filename) = filename_arg else {
        println!("{}", t!("no_filename"));
        return;
    };

    let pl0vm = match load_vm_from_file(filename, debug) {
        Ok(pl0vm) => pl0vm,
        Err(err) => {
            println!("{} - {}", t!("file_error", file = filename), err);
            return
        }
    };

    if analyze_only {
        pl0vm.print_analysis();
    } else {
        pl0vm.execute();
    }
}
