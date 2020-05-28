mod bot;
mod connection;
mod constants;

use std::env;
use connection::Connection;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

fn main() {
    env_logger::init();
    let token = env::var("RUSTY_BOT_TOKEN").unwrap();
    Connection::new(&token).listen();
}
