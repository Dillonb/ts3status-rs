pub fn seconds_to_string(secs: u64) -> String {
    let units = [
        ("day", secs / 86400),
        ("hour", secs / 3600 % 24),
        ("minute", secs / 60 % 60),
        ("second", secs % 60),
    ];

    units
        .iter()
        .filter(|(_, count)| *count > 0)
        .map(|(unit, count)| format!("{} {}{}", count, unit, if *count == 1 { "" } else { "s" }))
        .collect::<Vec<_>>()
        .join(" ")
}
