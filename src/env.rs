/// since we are using [`super::mach::read_double_nul_terminated_string_from_address()`] to read the
/// double nul terminated string into multiple lines, we search for the key and fetch the value from
/// the next line
pub fn env_get_value_for_key(env: &str, key: &str) -> Option<String> {
    let lines: Vec<&str> = env.lines().collect();

    for (index, line) in lines.iter().enumerate() {
        if line.trim() == key {
            return lines.get(index + 1).map(|s| s.to_string());
        }
    }

    None
}
