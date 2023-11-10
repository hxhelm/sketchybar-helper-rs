mod mach;

use crate::mach::message::sketchybar_message;

fn main() {
    let message = "--reload";

    let result = sketchybar_message(message);
    if let Some(result) = result {
        println!("{}", result);
    }
}
