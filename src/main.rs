mod mach;

use crate::mach::message::sketchybar_message;
use rand::Rng;

fn main() {
    let message = "--reload";

    let result = sketchybar_message(message);
    if let Some(result) = result {
        println!("{}", result);
    }
}
