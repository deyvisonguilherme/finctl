use crate::model::{Model, Tab};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs, Wrap},
    Frame,
};

pub fn view(model: &Model, frame: &mut Frame) {
    let size = frame.area();
    if size.width < 10 || size.height < 5 {
        return;
    }

    // Dividir a tela em 3 áreas verticais: Header, Main, Footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(5),    // Main content
            Constraint::Length(3), // Footer
        ])
        .split(size);

    render_header(model, frame, chunks[0]);
    render_main(model, frame, chunks[1]);
    render_footer(model, frame, chunks[2]);
}

fn render_header(model: &Model, frame: &mut Frame, area: Rect) {
    let clock_str = model.last_tick.format("%d/%m/%Y %H:%M:%S").to_string();
    let status_indicator = if model.is_loading {
        Span::styled(" [Sincronizando...]", Style::default().fg(Color::Yellow))
    } else {
        Span::styled(" [Online]", Style::default().fg(Color::Green))
    };

    let title_line = Line::from(vec![
        Span::styled(
            " finctl ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  Sistema de Controle Financeiro Pessoal "),
        status_indicator,
    ]);

    let clock_line = Line::from(vec![
        Span::styled(clock_str, Style::default().fg(Color::DarkGray)),
        Span::raw(" "),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

    let header_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(20), Constraint::Length(22)])
        .split(area);

    let title_paragraph = Paragraph::new(title_line).block(block.clone());
    let clock_paragraph = Paragraph::new(clock_line)
        .alignment(Alignment::Right)
        .block(block);

    frame.render_widget(title_paragraph, header_layout[0]);
    frame.render_widget(clock_paragraph, header_layout[1]);
}

fn render_main(model: &Model, frame: &mut Frame, area: Rect) {
    // Dividir a área principal em: Barra de Abas (altura 3) e Área de Visualização (restante)
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(4)])
        .split(area);

    // Renderizar Abas
    let tab_titles: Vec<Line> = Tab::ALL.iter().map(|t| Line::from(t.title())).collect();

    let tabs = Tabs::new(tab_titles)
        .select(model.active_tab.index())
        .block(Block::default().borders(Borders::ALL).title(" Navegação "))
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
                .bg(Color::DarkGray),
        );

    frame.render_widget(tabs, main_chunks[0]);

    // Renderizar Conteúdo da Aba Ativa
    match model.active_tab {
        Tab::Dashboard => render_dashboard_tab(model, frame, main_chunks[1]),
        Tab::Transactions => render_transactions_tab(model, frame, main_chunks[1]),
        Tab::Reports => render_reports_tab(model, frame, main_chunks[1]),
    }
}

fn render_dashboard_tab(model: &Model, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Dashboard - Visão Geral (Esqueleto F6-01) ")
        .border_style(Style::default().fg(Color::Cyan));

    let mut lines = vec![
        Line::from(vec![Span::styled(
            "Bem-vindo ao painel interativo do finctl!",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("• Arquitetura Elm: ", Style::default().fg(Color::Yellow)),
            Span::raw("Ciclo Model / Message / Update / View ativo e responsivo."),
        ]),
        Line::from(vec![
            Span::styled("• Canal Assíncrono: ", Style::default().fg(Color::Yellow)),
            Span::raw("Tarefas de I/O e banco executadas sem bloquear a interface."),
        ]),
        Line::from(vec![
            Span::styled("• Redimensionamento: ", Style::default().fg(Color::Yellow)),
            Span::raw("Layout dinâmico recalculado a cada mudança de tamanho do terminal."),
        ]),
        Line::from(""),
    ];

    if let Some(summary) = &model.data_summary {
        lines.push(Line::from(vec![
            Span::styled("Dados carregados: ", Style::default().fg(Color::Green)),
            Span::raw(summary),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("Estado: ", Style::default().fg(Color::DarkGray)),
            Span::raw("Aguardando carregamento de métricas iniciais (F6-02)."),
        ]));
    }

    let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn render_transactions_tab(_model: &Model, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Lançamentos e Transações ")
        .border_style(Style::default().fg(Color::Magenta));

    let text = vec![
        Line::from(Span::styled(
            "Tabela Paginada de Lançamentos",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("A listagem interativa, paginação e filtros serão ativados na etapa F6-03."),
    ];

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn render_reports_tab(_model: &Model, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Relatórios e Gráficos ")
        .border_style(Style::default().fg(Color::Green));

    let text = vec![
        Line::from(Span::styled(
            "Gráficos e Indicadores Financeiros",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Visualização gráfica por categoria e evolução mensal na etapa F6-04."),
    ];

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn render_footer(model: &Model, frame: &mut Frame, area: Rect) {
    let shortcuts = Line::from(vec![
        Span::styled(" [q/Ctrl+C] ", Style::default().fg(Color::Yellow)),
        Span::raw("Sair  "),
        Span::styled(" [Tab/1-3] ", Style::default().fg(Color::Yellow)),
        Span::raw("Mudar Aba  "),
        Span::styled(" [r] ", Style::default().fg(Color::Yellow)),
        Span::raw("Atualizar  "),
    ]);

    let status = Line::from(vec![
        Span::raw("Status: "),
        Span::styled(&model.status_message, Style::default().fg(Color::White)),
        Span::raw(" "),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let footer_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(35), Constraint::Length(35)])
        .split(area);

    let shortcuts_p = Paragraph::new(shortcuts).block(block.clone());
    let status_p = Paragraph::new(status)
        .alignment(Alignment::Right)
        .block(block);

    frame.render_widget(shortcuts_p, footer_layout[0]);
    frame.render_widget(status_p, footer_layout[1]);
}
