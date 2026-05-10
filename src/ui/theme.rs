use ratatui::style::{Color, Modifier, Style};

pub const BRAND_CYAN: Color = Color::Cyan;
pub const BRAND_RED: Color = Color::Red;
pub const GREEN: Color = Color::Green;
pub const RED: Color = Color::Red;
pub const DIM: Color = Color::DarkGray;
pub const BORDER_COLOR: Color = Color::DarkGray;
pub const SELECTED_BG: Color = Color::Rgb(40, 40, 80);
pub const HEADER_FG: Color = Color::White;

pub fn style_positive() -> Style {
    Style::default().fg(GREEN)
}

pub fn style_negative() -> Style {
    Style::default().fg(RED)
}

pub fn style_dim() -> Style {
    Style::default().fg(DIM)
}

pub fn style_bold() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

pub fn style_selected() -> Style {
    Style::default()
        .bg(SELECTED_BG)
        .add_modifier(Modifier::BOLD)
}

pub fn style_header() -> Style {
    Style::default().fg(HEADER_FG).add_modifier(Modifier::BOLD)
}

/// Returns green style if value string starts with '+' or is positive, red if negative.
pub fn pnl_style(value: &str) -> Style {
    let trimmed = value.trim();
    if trimmed.starts_with('-') {
        style_negative()
    } else {
        style_positive()
    }
}
