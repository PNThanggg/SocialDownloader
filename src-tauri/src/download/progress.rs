#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

fn num(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Ok(n) = s.parse::<u64>() {
        return Some(n);
    }
    let f: f64 = s.parse().ok()?;
    (f.is_finite() && f >= 0.0).then_some(f as u64)
}

/// Parses `LUMI|downloaded|total|total_estimate` lines from `--progress-template`.
pub fn parse_progress(line: &str) -> Option<Progress> {
    let rest = line.trim().strip_prefix("LUMI|")?;
    let mut parts = rest.split('|');
    let downloaded = num(parts.next()?)?;
    let total = parts.next().and_then(num);
    let total = total.or_else(|| parts.next().and_then(num));
    Some(Progress { downloaded, total })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_parses() {
        assert_eq!(parse_progress("LUMI|1024|2048|NA"), Some(Progress { downloaded: 1024, total: Some(2048) }));
        assert_eq!(parse_progress("LUMI|10|NA|500.0"), Some(Progress { downloaded: 10, total: Some(500) }));
        assert_eq!(parse_progress("LUMI|10|None|None"), Some(Progress { downloaded: 10, total: None }));
        assert_eq!(parse_progress("LUMI|NA|NA|NA"), None);
        assert_eq!(parse_progress("[download] x"), None);
    }
}
