use ratatui::style::Color;

use crate::ui::theme;

/// Convert a slice of u64 prices (in cents) to `(index, price_in_dollars)` pairs
/// suitable for use with ratatui's `Chart` widget.
pub fn price_points(history: &[u64]) -> Vec<(f64, f64)> {
    history
        .iter()
        .enumerate()
        .map(|(i, &v)| (i as f64, v as f64 / 100.0))
        .collect()
}

/// Compute y-axis bounds as `[min * 0.999, max * 1.001]` so the line
/// uses the full chart height without touching the edges.
/// Returns `[0.0, 1.0]` if the data slice is empty.
pub fn y_bounds(data: &[(f64, f64)]) -> [f64; 2] {
    if data.is_empty() {
        return [0.0, 1.0];
    }
    let min = data.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let max = data.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
    if (min - max).abs() < f64::EPSILON {
        // All values identical — add small padding so Chart renders something
        [min * 0.999, max * 1.001]
    } else {
        [min * 0.999, max * 1.001]
    }
}

/// Choose a line `Color` based on trend: green when last ≥ first, red otherwise.
pub fn trend_color(data: &[(f64, f64)]) -> Color {
    let first = data.first().map(|p| p.1).unwrap_or(0.0);
    let last = data.last().map(|p| p.1).unwrap_or(0.0);
    if last >= first {
        theme::GREEN
    } else {
        theme::RED
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── price_points ──────────────────────────────────────────────────────────

    #[test]
    fn price_points_empty() {
        assert!(price_points(&[]).is_empty());
    }

    #[test]
    fn price_points_converts_cents_to_dollars() {
        let pts = price_points(&[15000, 15050, 14950]);
        assert_eq!(pts.len(), 3);
        assert!((pts[0].0 - 0.0).abs() < f64::EPSILON);
        assert!((pts[0].1 - 150.00).abs() < 0.001);
        assert!((pts[1].0 - 1.0).abs() < f64::EPSILON);
        assert!((pts[1].1 - 150.50).abs() < 0.001);
        assert!((pts[2].0 - 2.0).abs() < f64::EPSILON);
        assert!((pts[2].1 - 149.50).abs() < 0.001);
    }

    #[test]
    fn price_points_single_value() {
        let pts = price_points(&[10000]);
        assert_eq!(pts.len(), 1);
        assert!((pts[0].0 - 0.0).abs() < f64::EPSILON);
        assert!((pts[0].1 - 100.00).abs() < 0.001);
    }

    // ── y_bounds ──────────────────────────────────────────────────────────────

    #[test]
    fn y_bounds_empty_returns_default() {
        let b = y_bounds(&[]);
        assert!((b[0] - 0.0).abs() < f64::EPSILON);
        assert!((b[1] - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn y_bounds_expands_by_padding() {
        let data = vec![(0.0, 100.0), (1.0, 200.0)];
        let b = y_bounds(&data);
        assert!((b[0] - 100.0 * 0.999).abs() < 0.001, "min bound: {}", b[0]);
        assert!((b[1] - 200.0 * 1.001).abs() < 0.001, "max bound: {}", b[1]);
    }

    #[test]
    fn y_bounds_single_point() {
        let data = vec![(0.0, 150.0)];
        let b = y_bounds(&data);
        assert!(b[0] < 150.0, "lower bound should be below value");
        assert!(b[1] > 150.0, "upper bound should be above value");
    }

    // ── trend_color ───────────────────────────────────────────────────────────

    #[test]
    fn trend_color_up_is_green() {
        let data = vec![(0.0, 100.0), (1.0, 110.0)];
        assert_eq!(trend_color(&data), theme::GREEN);
    }

    #[test]
    fn trend_color_down_is_red() {
        let data = vec![(0.0, 110.0), (1.0, 100.0)];
        assert_eq!(trend_color(&data), theme::RED);
    }

    #[test]
    fn trend_color_flat_is_green() {
        let data = vec![(0.0, 100.0), (1.0, 100.0)];
        assert_eq!(trend_color(&data), theme::GREEN);
    }

    #[test]
    fn trend_color_empty_is_green() {
        assert_eq!(trend_color(&[]), theme::GREEN);
    }
}
