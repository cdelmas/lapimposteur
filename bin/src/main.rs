extern crate lapimposteur_lib as lapimposteur;
extern crate log;

use lapimposteur::server;

fn main() {
    env_logger::init();
    // TODO: command line to get the config file
    server::run(); // pass the config file to the server
}
