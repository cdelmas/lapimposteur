extern crate clap;
extern crate lapimposteur_lib as lapimposteur;
extern crate log;

use clap::{App, Arg};
use lapimposteur::server;

fn main() {
    env_logger::init();

    let matches = App::new("Lapimposteur")
        .version("0.1")
        .about("Lapimposteur is a generic ampq stub")
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("FILE")
                .help("Sets a custom config file")
                .takes_value(true),
        )
        .get_matches();

    let config = matches
        .value_of("config")
        .expect("No config file given. Use --help.");

    server::run(config);
}
