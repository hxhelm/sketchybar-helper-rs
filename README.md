# sketchybar-helper-rs
A helper library for [SketchyBar](https://github.com/FelixKratz/SketchyBar) written in Rust, for writing helper 
programs in Rust.

## Usage

### Sending a message to SketchyBar
```rust
use sketchybar_helper_rs::message::sketchybar_message;

fn main() {
    sketchybar_message("--set foo label=bar");
}
```

### Receiving events from SketchyBar
```rust
use sketchybar_helper_rs::server::event_server_begin;

fn main() {
    event_server_begin(
        |event| {
            println!("Received event: {:?}", event);
        },
        "git.hxhelm.helper-rs",
    );
}
```

This library is heavily inspired by the original [SketchyBarHelper](https://github.com/FelixKratz/SketchyBarHelper).
