//! Plain-text rendering: aligned columns, and formatters that never invent a
//! number. A value the server did not send renders as `-`, not as zero.

/// How one column lines up. Numbers right, entities left, as everywhere else
/// in the product.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
}

/// Render one aligned table: a header row, a rule, one line per row. Column
/// widths come from the widest cell, so every column lines up down the page.
pub fn render(headers: &[&str], aligns: &[Align], rows: &[Vec<String>]) -> String {
    let cols = headers.len();
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate().take(cols) {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    let line = |cells: &[String]| -> String {
        let mut out = String::new();
        for i in 0..cols {
            let cell = cells.get(i).map(String::as_str).unwrap_or("");
            let pad = widths[i].saturating_sub(cell.chars().count());
            if i > 0 {
                out.push_str("  ");
            }
            match aligns.get(i).copied().unwrap_or(Align::Left) {
                Align::Left => {
                    out.push_str(cell);
                    // No trailing spaces on the last column.
                    if i + 1 < cols {
                        out.push_str(&" ".repeat(pad));
                    }
                }
                Align::Right => {
                    out.push_str(&" ".repeat(pad));
                    out.push_str(cell);
                }
            }
        }
        out
    };
    let header: Vec<String> = headers.iter().map(|h| h.to_string()).collect();
    let mut out = line(&header);
    out.push('\n');
    out.push_str(&"-".repeat(widths.iter().sum::<usize>() + 2 * (cols.saturating_sub(1))));
    for row in rows {
        out.push('\n');
        out.push_str(&line(row));
    }
    out
}

pub const DASH: &str = "-";

/// Group an unsigned integer part with commas: 1204 -> "1,204".
fn group(int_part: &str) -> String {
    let digits: Vec<char> = int_part.chars().collect();
    let mut out = String::new();
    for (i, c) in digits.iter().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*c);
    }
    out
}

/// A USD figure with 2 decimals and thousands separators; `-` when absent.
pub fn usd(v: Option<f64>) -> String {
    match v {
        Some(v) if v.is_finite() => {
            let neg = v < 0.0;
            let s = format!("{:.2}", v.abs());
            let (int_part, frac) = s.split_once('.').unwrap_or((&s, "00"));
            format!("{}{}.{}", if neg { "-" } else { "" }, group(int_part), frac)
        }
        _ => DASH.to_string(),
    }
}

/// A USD figure the server sent as a decimal string, regrouped; `-` otherwise.
pub fn usd_str(v: Option<&str>) -> String {
    usd(v.and_then(|s| s.parse::<f64>().ok()))
}

/// A percent with `dp` decimals; `-` when absent.
pub fn pct(v: Option<f64>, dp: usize) -> String {
    match v {
        Some(v) if v.is_finite() => format!("{:.*}%", dp, v),
        _ => DASH.to_string(),
    }
}

/// A price with enough precision to be meaningful at any magnitude.
pub fn price(v: Option<f64>) -> String {
    match v {
        Some(v) if v.is_finite() && v > 0.0 => {
            if v >= 1.0 {
                format!("{v:.4}")
            } else {
                format!("{v:.8}")
            }
        }
        _ => DASH.to_string(),
    }
}

/// Lamports as SOL, exact: 1 SOL = 1_000_000_000 lamports.
pub fn sol(lamports: Option<u64>) -> String {
    match lamports {
        Some(l) => {
            let whole = l / 1_000_000_000;
            let frac = l % 1_000_000_000;
            if frac == 0 {
                format!("{whole} SOL")
            } else {
                let frac = format!("{frac:09}");
                format!("{whole}.{} SOL", frac.trim_end_matches('0'))
            }
        }
        None => DASH.to_string(),
    }
}

/// A string field, or `-`.
pub fn dash(v: Option<&str>) -> String {
    match v {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => DASH.to_string(),
    }
}

/// Age from an epoch-milliseconds creation time: "3h", "2d", "41d"; `-` when
/// the venue did not report one.
pub fn age(created_at_ms: Option<i64>, now_ms: i64) -> String {
    match created_at_ms {
        Some(t) if t > 0 && now_ms >= t => {
            let mins = (now_ms - t) / 60_000;
            if mins < 60 {
                format!("{mins}m")
            } else if mins < 48 * 60 {
                format!("{}h", mins / 60)
            } else {
                format!("{}d", mins / (60 * 24))
            }
        }
        _ => DASH.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_line_up_whatever_the_cell_widths() {
        let out = render(
            &["PAIR", "TVL"],
            &[Align::Left, Align::Right],
            &[
                vec!["SOL-USDC".into(), "1,204.18".into()],
                vec!["X".into(), "7.76".into()],
            ],
        );
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "PAIR           TVL");
        assert_eq!(lines[2], "SOL-USDC  1,204.18");
        // The right-aligned column's last characters line up down the page.
        assert_eq!(lines[3], "X             7.76");
        assert_eq!(lines[2].len(), lines[3].len());
    }

    #[test]
    fn money_groups_thousands_and_never_fabricates() {
        assert_eq!(usd(Some(1204.183)), "1,204.18");
        assert_eq!(usd(Some(-98765.4)), "-98,765.40");
        assert_eq!(usd(Some(0.0)), "0.00");
        assert_eq!(usd(None), "-");
        assert_eq!(usd(Some(f64::NAN)), "-");
        assert_eq!(usd_str(Some("48.21")), "48.21");
        assert_eq!(usd_str(Some("not a number")), "-");
        assert_eq!(usd_str(None), "-");
    }

    #[test]
    fn lamports_render_as_exact_sol() {
        assert_eq!(sol(Some(1_000_000_000)), "1 SOL");
        assert_eq!(sol(Some(1_500_000_000)), "1.5 SOL");
        assert_eq!(sol(Some(123)), "0.000000123 SOL");
        assert_eq!(sol(None), "-");
    }

    #[test]
    fn absent_values_are_a_dash_everywhere() {
        assert_eq!(pct(None, 2), "-");
        assert_eq!(price(None), "-");
        assert_eq!(dash(None), "-");
        assert_eq!(dash(Some("")), "-");
        assert_eq!(age(None, 0), "-");
    }

    #[test]
    fn age_is_computed_from_real_clocks_only() {
        let now = 1_761_000_000_000i64;
        assert_eq!(age(Some(now - 5 * 60_000), now), "5m");
        assert_eq!(age(Some(now - 3 * 3_600_000), now), "3h");
        assert_eq!(age(Some(now - 10 * 86_400_000), now), "10d");
        // A creation time in the future is not an age; say nothing.
        assert_eq!(age(Some(now + 60_000), now), "-");
    }
}
