mod mach;

use crate::mach::message::sketchybar_message;

fn main() {
    let message = "--query bar";

    let result = sketchybar_message(message);
    if let Some(result) = result {
        println!("{}", result);
    }
}
