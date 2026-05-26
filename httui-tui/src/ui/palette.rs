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

/// Âmbar quente para sinalizar estado temporário/ephemero (badge
/// TEMP, valores de session override). Visível sobre fundos escuros
/// e não compete com Color::Red (que é exclusivo pra erros).
pub const AMBER: Color = Color::Rgb(255, 176, 0);
pub const AMBER_FG_ON_AMBER_BG: Color = Color::Rgb(20, 14, 0);

/// Muted gray para texto secundário (labels, atalhos inativos,
/// timestamps). `Color::DarkGray` da ratatui renderiza diferente
/// por tema de terminal (pode ficar branco em temas warm); este
/// RGB fixo garante leitura como dim cross-platform.
pub const MUTED: Color = Color::Rgb(120, 120, 120);
/// Mid-emphasis gray para conteúdo secundário ainda legível
/// (subjects de commits sem foco). Mais claro que MUTED.
pub const SECONDARY: Color = Color::Rgb(170, 170, 170);
