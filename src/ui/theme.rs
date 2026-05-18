use ratatui::style::{Color, Modifier, Style};

// ── ThemeColors ───────────────────────────────────────────────────────────────

/// A resolved color palette for a single theme.
///
/// Obtain an instance via [`Theme::colors`]. All UI renderers should prefer
/// these methods over the module-level constants so that theme switching works
/// at runtime.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeColors {
    /// Primary accent / highlight color (cyan-family).
    pub accent: Color,
    /// Color for positive values and up-trends.
    pub positive: Color,
    /// Color for negative values and down-trends.
    pub negative: Color,
    /// Color for neutral / warning values.
    pub neutral: Color,
    /// Color for de-emphasized text and borders in the Default theme.
    pub dim: Color,
    /// Border color for panels and modals.
    pub border: Color,
    /// Background color for the selected row.
    pub selected_bg: Color,
    /// Foreground color for table/list headers.
    pub header_fg: Color,
}

impl ThemeColors {
    /// Style using the accent color.
    pub fn accent_style(&self) -> Style {
        Style::default().fg(self.accent)
    }
    /// Style for positive numbers (green-family).
    pub fn positive_style(&self) -> Style {
        Style::default().fg(self.positive)
    }
    /// Style for negative numbers (red-family).
    pub fn negative_style(&self) -> Style {
        Style::default().fg(self.negative)
    }
    /// Style for de-emphasized / dimmed text.
    pub fn dim_style(&self) -> Style {
        Style::default().fg(self.dim)
    }
    /// Bold style (theme-independent weight; no color change).
    pub fn bold_style(&self) -> Style {
        Style::default().add_modifier(Modifier::BOLD)
    }
    /// Style for the currently selected table row.
    pub fn selected_style(&self) -> Style {
        Style::default()
            .bg(self.selected_bg)
            .add_modifier(Modifier::BOLD)
    }
    /// Bold + colored style for table / list column headers.
    pub fn header_style(&self) -> Style {
        Style::default()
            .fg(self.header_fg)
            .add_modifier(Modifier::BOLD)
    }
    /// Style for panel/modal borders.
    pub fn border_fg_style(&self) -> Style {
        Style::default().fg(self.border)
    }
    /// Green/red style for a P&L string; positive if value doesn't start with `'-'`.
    pub fn pnl_style(&self, value: &str) -> Style {
        if value.trim().starts_with('-') {
            self.negative_style()
        } else {
            self.positive_style()
        }
    }
}

// ── Theme ─────────────────────────────────────────────────────────────────────

/// Available UI color themes.
///
/// The active theme is stored in [`crate::app::App::current_theme`] and
/// persisted to `~/.config/alpaca-trader/config.toml` under `[ui] theme`.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Theme {
    /// Cyan/white/green palette — the original appearance.
    #[default]
    Default,
    /// Muted, reduced-contrast palette for low-light environments.
    Dark,
    /// Bold borders, bright whites — maximum readability / accessibility.
    HighContrast,
}

impl Theme {
    /// Advance to the next theme in the cycle: Default → Dark → HighContrast → Default.
    pub fn cycle(&self) -> Self {
        match self {
            Theme::Default => Theme::Dark,
            Theme::Dark => Theme::HighContrast,
            Theme::HighContrast => Theme::Default,
        }
    }

    /// The config-file key string for this theme (round-trips through [`Theme::from_str`]).
    pub fn as_str(&self) -> &'static str {
        match self {
            Theme::Default => "default",
            Theme::Dark => "dark",
            Theme::HighContrast => "high-contrast",
        }
    }

    /// Human-readable display name shown in the status bar flash.
    pub fn display_name(&self) -> &'static str {
        match self {
            Theme::Default => "Default",
            Theme::Dark => "Dark",
            Theme::HighContrast => "High-contrast",
        }
    }

    /// Parse the config-file key; unrecognised values fall back to [`Theme::Default`].
    pub fn from_str(s: &str) -> Self {
        match s {
            "dark" => Theme::Dark,
            "high-contrast" => Theme::HighContrast,
            _ => Theme::Default,
        }
    }

    /// Returns the resolved [`ThemeColors`] for this theme.
    pub fn colors(&self) -> ThemeColors {
        match self {
            Theme::Default => ThemeColors {
                accent: Color::Cyan,
                positive: Color::Green,
                negative: Color::Red,
                neutral: Color::Yellow,
                dim: Color::DarkGray,
                border: Color::DarkGray,
                selected_bg: Color::Rgb(40, 40, 80),
                header_fg: Color::White,
            },
            Theme::Dark => ThemeColors {
                accent: Color::Rgb(0, 160, 200),
                positive: Color::Rgb(0, 180, 100),
                negative: Color::Rgb(190, 60, 60),
                neutral: Color::Rgb(170, 150, 0),
                dim: Color::Rgb(90, 90, 90),
                border: Color::Rgb(70, 70, 70),
                selected_bg: Color::Rgb(30, 30, 60),
                header_fg: Color::Rgb(190, 190, 190),
            },
            Theme::HighContrast => ThemeColors {
                accent: Color::Rgb(0, 255, 255),
                positive: Color::Rgb(0, 255, 128),
                negative: Color::Rgb(255, 80, 80),
                neutral: Color::Rgb(255, 255, 0),
                dim: Color::White,
                border: Color::White,
                selected_bg: Color::Rgb(60, 60, 120),
                header_fg: Color::Rgb(255, 255, 255),
            },
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Theme::cycle ──────────────────────────────────────────────────────────

    #[test]
    fn cycle_default_to_dark() {
        assert_eq!(Theme::Default.cycle(), Theme::Dark);
    }

    #[test]
    fn cycle_dark_to_high_contrast() {
        assert_eq!(Theme::Dark.cycle(), Theme::HighContrast);
    }

    #[test]
    fn cycle_high_contrast_wraps_to_default() {
        assert_eq!(Theme::HighContrast.cycle(), Theme::Default);
    }

    // ── Theme::from_str ───────────────────────────────────────────────────────

    #[test]
    fn from_str_dark() {
        assert_eq!(Theme::from_str("dark"), Theme::Dark);
    }

    #[test]
    fn from_str_high_contrast() {
        assert_eq!(Theme::from_str("high-contrast"), Theme::HighContrast);
    }

    #[test]
    fn from_str_default_explicit() {
        assert_eq!(Theme::from_str("default"), Theme::Default);
    }

    #[test]
    fn from_str_unknown_falls_back_to_default() {
        assert_eq!(Theme::from_str("neon"), Theme::Default);
    }

    // ── Theme::as_str / display_name ─────────────────────────────────────────

    #[test]
    fn as_str_round_trips() {
        for t in [Theme::Default, Theme::Dark, Theme::HighContrast] {
            assert_eq!(
                Theme::from_str(t.as_str()),
                t,
                "round-trip failed for {:?}",
                t
            );
        }
    }

    #[test]
    fn display_names_are_non_empty() {
        for t in [Theme::Default, Theme::Dark, Theme::HighContrast] {
            assert!(!t.display_name().is_empty());
        }
    }

    // ── ThemeColors helpers ───────────────────────────────────────────────────

    #[test]
    fn pnl_style_negative_for_minus_prefix() {
        let c = Theme::Default.colors();
        assert_eq!(c.pnl_style("-1.00"), c.negative_style());
    }

    #[test]
    fn pnl_style_positive_for_plus_value() {
        let c = Theme::Default.colors();
        assert_eq!(c.pnl_style("+2.00"), c.positive_style());
    }

    #[test]
    fn default_colors_accent_is_cyan() {
        assert_eq!(Theme::Default.colors().accent, Color::Cyan);
    }

    #[test]
    fn dark_colors_are_distinct_from_default() {
        assert_ne!(Theme::Dark.colors(), Theme::Default.colors());
    }

    #[test]
    fn high_contrast_colors_are_distinct_from_dark() {
        assert_ne!(Theme::HighContrast.colors(), Theme::Dark.colors());
    }
}
