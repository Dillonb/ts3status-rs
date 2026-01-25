pub fn seconds_to_string(secs: u64) -> String {
    let s = |n| if n == 1 { "" } else { "s" };
    let days = secs / 86400;
    let seconds = secs % 86400;
    let hours = seconds / 3600;
    let seconds = seconds % 3600;
    let minutes = seconds / 60;
    let seconds = seconds % 60;

    let mut result = String::new();
    if days > 0 {
        result.push_str(&format!("{} day{} ", days, s(days)));
    }
    if hours > 0 {
        result.push_str(&format!("{} hour{} ", hours, s(hours)));
    }
    if minutes > 0 {
        result.push_str(&format!("{} minute{} ", minutes, s(minutes)));
    }
    if seconds > 0 {
        result.push_str(&format!("{} second{} ", seconds, s(seconds)));
    }

    result
}
