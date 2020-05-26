mod bot;
mod connection;
mod constants;

use std::error::Error;
use rand::seq::{SliceRandom};
use std::env;
use regex::Regex;
use connection::Connection;

#[macro_use]
extern crate lazy_static;


fn main() {
    let token = env::var("RUSTY_BOT_TOKEN").unwrap();
    let connection = Connection::new(&token).listen();
}
