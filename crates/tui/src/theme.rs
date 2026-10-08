use ratatui::style::Color;

/// Modo do tema visual da TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

impl ThemeMode {
    /// Alterna entre modo escuro e claro.
    pub fn toggle(&self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }
}

/// Conjunto de cores semânticas da interface TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub mode: ThemeMode,
    pub bg: Color,
    pub fg: Color,
    pub fg_muted: Color,
    pub border: Color,
    pub border_focus: Color,
    pub header_bg: Color,
    pub header_fg: Color,
    pub tab_active_fg: Color,
    pub tab_active_bg: Color,
    pub tab_inactive_fg: Color,
    pub table_header_fg: Color,
    pub table_selected_bg: Color,
    pub table_selected_fg: Color,
    pub shortcut_key: Color,
    pub shortcut_desc: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    pub info: Color,
    pub modal_bg: Color,
    pub modal_border: Color,
    pub input_active_border: Color,
    pub input_inactive_border: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    /// Constrói o tema a partir do modo informado.
    pub fn from_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self::dark(),
            ThemeMode::Light => Self::light(),
        }
    }

    /// Tema escuro (padrão).
    pub fn dark() -> Self {
        Self {
            mode: ThemeMode::Dark,
            bg: Color::Reset,
            fg: Color::White,
            fg_muted: Color::DarkGray,
            border: Color::Cyan,
            border_focus: Color::Yellow,
            header_bg: Color::Cyan,
            header_fg: Color::Black,
            tab_active_fg: Color::Yellow,
            tab_active_bg: Color::DarkGray,
            tab_inactive_fg: Color::White,
            table_header_fg: Color::Yellow,
            table_selected_bg: Color::DarkGray,
            table_selected_fg: Color::White,
            shortcut_key: Color::Yellow,
            shortcut_desc: Color::White,
            success: Color::Green,
            warning: Color::Yellow,
            danger: Color::Red,
            info: Color::Cyan,
            modal_bg: Color::Black,
            modal_border: Color::Cyan,
            input_active_border: Color::Yellow,
            input_inactive_border: Color::DarkGray,
        }
    }

    /// Tema claro.
    pub fn light() -> Self {
        Self {
            mode: ThemeMode::Light,
            bg: Color::Reset,
            fg: Color::Black,
            fg_muted: Color::Gray,
            border: Color::Blue,
            border_focus: Color::Magenta,
            header_bg: Color::Blue,
            header_fg: Color::White,
            tab_active_fg: Color::White,
            tab_active_bg: Color::Blue,
            tab_inactive_fg: Color::Black,
            table_header_fg: Color::Blue,
            table_selected_bg: Color::Gray,
            table_selected_fg: Color::Black,
            shortcut_key: Color::Blue,
            shortcut_desc: Color::Black,
            success: Color::Green,
            warning: Color::Rgb(180, 100, 0),
            danger: Color::Red,
            info: Color::Blue,
            modal_bg: Color::White,
            modal_border: Color::Blue,
            input_active_border: Color::Blue,
            input_inactive_border: Color::Gray,
        }
    }

    /// Alterna o tema atual entre claro e escuro.
    pub fn toggle(&self) -> Self {
        Self::from_mode(self.mode.toggle())
    }
}
