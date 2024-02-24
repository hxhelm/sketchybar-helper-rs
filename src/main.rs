use env::env_get_value_for_key;

mod env;
mod mach;

use crate::mach::message::sketchybar_message;
use crate::mach::server::event_server_begin;

fn handler(env: &str) {
    println!("{}", env);
    let name = env_get_value_for_key(env, "NAME");
    println!("{}", name.unwrap_or("".to_string()))
}

fn main() {
    // let message = "--query bar";
    //
    // let result = sketchybar_message(message);
    // if let Some(result) = result {
    //     println!("{}", result);
    // }

    event_server_begin(handler, "git.hxhelm.helper-rs");
}
