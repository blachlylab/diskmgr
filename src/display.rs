pub fn fmt_size(bytes: u64) -> String {
    const TB: f64 = 1e12;
    const GB: f64 = 1e9;
    if bytes as f64 >= 0.95 * TB {
        format!("{:.1} TB", bytes as f64 / TB)
    } else if bytes as f64 >= 0.95 * GB {
        format!("{:.1} GB", bytes as f64 / GB)
    } else {
        format!("{bytes} B")
    }
}

pub fn dash(value: Option<&str>) -> &str {
    match value {
        Some(s) if !s.is_empty() => s,
        _ => "—",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(fmt_size(8001563222016), "8.0 TB");
        assert_eq!(fmt_size(256060514304), "256.1 GB");
    }
}
