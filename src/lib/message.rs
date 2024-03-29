use crate::mach::{get_global_mach_port, mach_get_bs_port, mach_send_message};

pub fn message(message: &str) -> Option<String> {
    let mut global_mach_port = get_global_mach_port();
    if *global_mach_port == 0 {
        *global_mach_port = mach_get_bs_port();
    }

    let mut formatted_message = format_mach_message(message);
    let formatted_message_length = formatted_message.len();

    mach_send_message(
        *global_mach_port,
        &mut formatted_message,
        formatted_message_length,
    )
}

fn format_mach_message(message: &str) -> Vec<u8> {
    let mut formatted_message = vec!['\0'; message.len() + 2];

    let mut quote = '\0';
    let mut caret: usize = 0;

    // TODO: check if this can be replaced by normal string manipulation
    for c in message.chars() {
        if c == '"' || c == '\'' {
            if c == quote {
                quote = '\0';
            } else {
                quote = c;
            }
            continue;
        }

        formatted_message[caret] = c;

        if c == ' ' && quote == '\0' {
            formatted_message[caret] = '\0';
        }

        caret += 1;
    }

    if caret > 0 && formatted_message[caret] == '\0' && formatted_message[caret - 1] == '\0' {
        caret -= 1;
    }

    formatted_message[caret] = '\0';
    formatted_message[caret + 1] = '\0';

    formatted_message
        .iter()
        .map(|c| *c as u8)
        .collect::<Vec<_>>()
}
