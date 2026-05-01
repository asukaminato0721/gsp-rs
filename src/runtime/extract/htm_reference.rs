pub(super) fn construction_lines_from_log(log: &str) -> Vec<String> {
    let mut lines = log.lines().skip_while(|line| *line != "Construction VALUE");
    lines.next();
    lines
        .take_while(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .collect()
}

pub(super) fn construction_lines_from_htm(htm: &str) -> Vec<String> {
    let marker = "<PARAM NAME=Construction VALUE=\"";
    let start = htm
        .find(marker)
        .expect("reference htm should contain Construction VALUE")
        + marker.len();
    let end = htm[start..]
        .find("\">")
        .expect("reference htm construction should close");
    htm[start..start + end]
        .replace("&#xD;", "\n")
        .replace("&quot;", "\"")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}
