extern crate lapimposteur_lib as lapimposteur;
extern crate log;

use lapimposteur::server;

fn main() {
    env_logger::init();
    server::run();
}
