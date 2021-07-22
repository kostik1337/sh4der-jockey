//#![feature(external_doc)]
//#![doc(include = "../README.md")]

#[allow(unused)]
#[macro_use]
extern crate error_chain;

#[macro_use]
mod util;
mod jockey;

use getopts::Options;
use jockey::Jockey;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bin_name = args.get(0).map(|s| s.as_str()).unwrap_or("sh4der-jockey");

    let mut opts = Options::new();
    opts.optflag("h", "help", "print this help message");
    opts.optflag("i", "init", "set up a simple example project");

    let matches = opts.parse(&args[1..]).expect("failed to parse args");
    if matches.opt_present("h") {
        println!(
            "{}\n\n{}",
            opts.short_usage(bin_name),
            opts.usage("A custom VJ tool written by sp4ghet and slerpy.")
        );
        return;
    }

    if matches.opt_present("i") {
        let plf = std::path::Path::new("./pipeline.yaml");
        let shf = std::path::Path::new("./scene.frag");

        if plf.exists() || shf.exists() {
            eprintln!(
                "Error: File with same name already exists.\n\n\
                Please make sure there are no files named \"pipeline.yaml\" or \"scene.frag\"\n\
                in your current working directory already. Try renaming or deleting these\n\
                files and running the command again.\n"
            );
            return;
        }

        std::fs::write(plf, include_str!("defaults/pipeline.yaml")).unwrap();
        std::fs::write(shf, include_str!("defaults/scene.frag")).unwrap();
    }

    // create the jockey
    let mut jockey = Jockey::init();

    loop {
        // do event stuff
        jockey.handle_events();

        // exit loop
        if jockey.done {
            break;
        }

        // run all shader stages
        jockey.draw();

        // update ui
        jockey.update_ui();
    }
}
