use std::{env::args_os, process::exit};

mod deb;

fn main() {
    let mut args = args_os().peekable();

    if args.len() == 2
        && args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--prepare-deb-dependencies"))
    {
        if let Err(error) = deb::prepare() {
            eprintln!("Could not determine Debian dependencies: {error}");
            exit(1);
        }
        return;
    }

    if args.len() != 1 {
        eprintln!("This command must be run as `cargo run --bin packager`");
        exit(1);
    }
    args.next(); // Skip the binary name

    cargo_packager::cli::run(args, None)
}
