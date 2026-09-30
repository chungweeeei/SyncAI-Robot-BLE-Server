/// 解析 /proc/net/wireless，格式：
/// ` wlP1p1s0: 0000   60.  -50.  -256 ...`（第 4 個欄位是 signal level）
pub fn parse_rssi_dbm(text: &str, iface: &str) -> Option<i32> {
    let prefix = format!("{iface}:");
    text.lines()
        .find(|line| line.trim_start().starts_with(&prefix))?
        .split_whitespace()
        .nth(3)?
        .trim_end_matches('.')
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::parse_rssi_dbm;

    const SAMPLE: &str = "\
Inter-| sta-|   Quality        |   Discarded packets               | Missed | WE
 face | tus | link level noise |  nwid  crypt   frag  retry   misc | beacon | 22
wlP1p1s0: 0000   60.  -50.  -256        0      0      0      3      0        0
";

    #[test]
    fn parses_signal_level() {
        assert_eq!(parse_rssi_dbm(SAMPLE, "wlP1p1s0"), Some(-50));
    }

    #[test]
    fn unknown_interface_is_none() {
        assert_eq!(parse_rssi_dbm(SAMPLE, "wlan0"), None);
    }
}
