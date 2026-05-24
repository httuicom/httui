use ratatui::style::Color;

pub const SELECTION_BG: Color = Color::Rgb(60, 70, 110);

/// V4 (2026-05-23): cor de borda padrão pra modals/pages —
/// cinza-azulado sutil; menos agressivo que `LightMagenta`
/// (que vira vermelho/laranja em temas warm-light de terminal).
pub const BORDER: Color = Color::Rgb(110, 140, 175);

/// Accent cool pra títulos, headers de seção e atalhos numéricos.
/// Centralizar evita o uso ad-hoc de `LightMagenta`/`LightCyan`
/// que se traduz de forma diferente em cada tema.
pub const ACCENT: Color = Color::Rgb(130, 170, 220);
