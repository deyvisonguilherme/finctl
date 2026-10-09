use crate::model::{
    ContributionFormField, ContributionModalState, DeleteConfirmState, FilterField,
    FilterModalState, FormField, FormMode, GoalsSubView, Model, PeriodModalState, ReportSubView,
    Tab, TransactionFormState,
};
use crate::shortcuts::{ShortcutCategory, ShortcutRegistry};
use app::{DashboardData, UpcomingKind};
use domain::{
    format_decimal_pt_br, BudgetIndicator, ForecastGranularity, GoalProgress, TransactionKind,
    TransactionStatus,
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Sparkline, Tabs},
    Frame,
};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

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

    // Modal de Ajuda sobreposto sobre a área de conteúdo
    if model.is_help_open {
        render_help_modal(model, frame, chunks[1]);
    }
}

fn render_header(model: &Model, frame: &mut Frame, area: Rect) {
    let theme = model.theme;
    let clock_str = model.last_tick.format("%d/%m/%Y %H:%M:%S").to_string();
    let status_indicator = if model.error_message.is_some() {
        Span::styled(
            " [Erro de Conexão]",
            Style::default()
                .fg(theme.danger)
                .add_modifier(Modifier::BOLD),
        )
    } else if model.is_loading {
        Span::styled(" [Sincronizando...]", Style::default().fg(theme.warning))
    } else {
        Span::styled(" [Online]", Style::default().fg(theme.success))
    };

    let title_desc = if area.width < 90 {
        " Controle Financeiro "
    } else {
        "  Sistema de Controle Financeiro Pessoal "
    };

    let title_line = Line::from(vec![
        Span::styled(
            " finctl ",
            Style::default()
                .fg(theme.header_fg)
                .bg(theme.header_bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(title_desc),
        status_indicator,
    ]);

    let clock_line = Line::from(vec![
        Span::styled(clock_str, Style::default().fg(theme.fg_muted)),
        Span::raw(" "),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border));

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
    let theme = model.theme;
    // Dividir a área principal em: Barra de Abas (altura 3) e Área de Visualização (restante)
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(4)])
        .split(area);

    // Renderizar Abas
    let tab_titles: Vec<Line> = Tab::ALL.iter().map(|t| Line::from(t.title())).collect();

    let tabs = Tabs::new(tab_titles)
        .select(model.active_tab.index())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Navegação ")
                .border_style(Style::default().fg(theme.border)),
        )
        .style(Style::default().fg(theme.tab_inactive_fg))
        .highlight_style(
            Style::default()
                .fg(theme.tab_active_fg)
                .add_modifier(Modifier::BOLD)
                .bg(theme.tab_active_bg),
        );

    frame.render_widget(tabs, main_chunks[0]);

    // Renderizar Conteúdo da Aba Ativa
    match model.active_tab {
        Tab::Dashboard => render_dashboard_tab(model, frame, main_chunks[1]),
        Tab::Transactions => render_transactions_tab(model, frame, main_chunks[1]),
        Tab::Reports => render_reports_tab(model, frame, main_chunks[1]),
        Tab::Budgets => render_budgets_tab(model, frame, main_chunks[1]),
        Tab::Goals => render_goals_tab(model, frame, main_chunks[1]),
    }
}

fn render_dashboard_tab(model: &Model, frame: &mut Frame, area: Rect) {
    if let Some(ref data) = model.dashboard_data {
        render_dashboard_grid(data, frame, area);
    } else if model.is_loading {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Dashboard - Visão Geral ")
            .border_style(Style::default().fg(Color::Yellow));

        let paragraph = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "Carregando dados financeiros...",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("Sincronizando saldos, relatórios e orçamentos com o PostgreSQL."),
        ])
        .block(block)
        .alignment(Alignment::Center);

        frame.render_widget(paragraph, area);
    } else if let Some(ref err) = model.error_message {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Dashboard - Falha na Sincronização ")
            .border_style(Style::default().fg(Color::Red));

        let paragraph = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "Não foi possível carregar as informações do banco de dados.",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::raw(format!("Detalhes: {err}"))),
            Line::from(""),
            Line::from(Span::styled(
                "Pressione 'r' para tentar reconectar ou 'q' para sair.",
                Style::default().fg(Color::Yellow),
            )),
        ])
        .block(block)
        .alignment(Alignment::Center);

        frame.render_widget(paragraph, area);
    } else {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Dashboard - Visão Geral ")
            .border_style(Style::default().fg(Color::Cyan));

        let paragraph = Paragraph::new(vec![
            Line::from("Aguardando carregamento de dados."),
            Line::from("Pressione 'r' para disparar a atualização."),
        ])
        .block(block);

        frame.render_widget(paragraph, area);
    }
}

fn render_dashboard_grid(data: &DashboardData, frame: &mut Frame, area: Rect) {
    // Dividir a área do Dashboard em 2 linhas verticais (Superior e Inferior)
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Linha Superior: Esquerda (Saldos por Conta) | Direita (Resumo do Mês)
    let top_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[0]);

    // Linha Inferior: Esquerda (Orçamentos) | Direita (Próximos Vencimentos)
    let bottom_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);

    render_accounts_panel(data, frame, top_cols[0]);
    render_monthly_summary_panel(data, frame, top_cols[1]);
    render_budgets_panel(data, frame, bottom_cols[0]);
    render_upcoming_panel(data, frame, bottom_cols[1]);
}

fn render_accounts_panel(data: &DashboardData, frame: &mut Frame, area: Rect) {
    let total_accounts = data.balance_report.accounts.len();
    let title = if total_accounts > 0 {
        format!(" Saldos por Conta ({total_accounts}) ")
    } else {
        " Saldos por Conta ".to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(Color::Cyan));

    let mut lines = Vec::new();

    if data.balance_report.accounts.is_empty() {
        lines.push(Line::from(Span::styled(
            "Nenhuma conta cadastrada.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        // Exibir até 4 contas para caber bem em terminal 80x24
        let max_items = (area.height.saturating_sub(4)).max(2) as usize;
        for acc in data.balance_report.accounts.iter().take(max_items) {
            let bal_str = format_decimal_pt_br(acc.current_balance);
            let bal_color = if acc.current_balance >= Decimal::ZERO {
                Color::Green
            } else {
                Color::Red
            };

            let name_display = if acc.account_name.chars().count() > 18 {
                let truncated: String = acc.account_name.chars().take(17).collect();
                format!("{truncated}…")
            } else {
                format!("{:18}", acc.account_name)
            };

            lines.push(Line::from(vec![
                Span::raw(format!("• {name_display} ")),
                Span::styled(bal_str, Style::default().fg(bal_color)),
            ]));
        }

        if data.balance_report.accounts.len() > max_items {
            let remaining = data.balance_report.accounts.len() - max_items;
            lines.push(Line::from(Span::styled(
                format!("  (+{remaining} outras contas)"),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    // Linha divisória e Totalizador Geral
    let total_str = format_decimal_pt_br(data.balance_report.total_balance);
    let total_color = if data.balance_report.total_balance >= Decimal::ZERO {
        Color::Green
    } else {
        Color::Red
    };

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            "Total Geral: ",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            total_str,
            Style::default()
                .fg(total_color)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn render_monthly_summary_panel(data: &DashboardData, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Resumo do Mês ({}) ", data.monthly_summary.month))
        .border_style(Style::default().fg(Color::Green));

    let income_str = format_decimal_pt_br(data.monthly_summary.total_income.as_decimal());
    let expense_str = format_decimal_pt_br(data.monthly_summary.total_expense.as_decimal());
    let net_str = format_decimal_pt_br(data.monthly_summary.net_balance);
    let net_color = if data.monthly_summary.net_balance >= Decimal::ZERO {
        Color::Green
    } else {
        Color::Red
    };
    let savings_rate_str = format!("{:.1}%", data.monthly_summary.savings_rate);

    let lines = vec![
        Line::from(vec![
            Span::styled("Receitas:     ", Style::default().fg(Color::White)),
            Span::styled(
                income_str,
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Despesas:     ", Style::default().fg(Color::White)),
            Span::styled(
                expense_str,
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from("─────────────────────────────"),
        Line::from(vec![
            Span::styled(
                "Saldo Líquido:",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                net_str,
                Style::default().fg(net_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Poupança:     ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                savings_rate_str,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn render_budgets_panel(data: &DashboardData, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Status dos Orçamentos ")
        .border_style(Style::default().fg(Color::Yellow));

    let mut lines = Vec::new();

    if data.budget_statuses.is_empty() {
        lines.push(Line::from(Span::styled(
            "Nenhum orçamento configurado para este mês.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let max_items = (area.height.saturating_sub(2)).max(2) as usize;
        for b in data.budget_statuses.iter().take(max_items) {
            let cat_name = if b.category_name.chars().count() > 12 {
                let truncated: String = b.category_name.chars().take(11).collect();
                format!("{truncated}…")
            } else {
                format!("{:12}", b.category_name)
            };

            let bar_color = match b.indicator {
                BudgetIndicator::Ok => Color::Green,
                BudgetIndicator::Warning => Color::Yellow,
                BudgetIndicator::Exceeded => Color::Red,
            };

            let bar = format_progress_bar(b.percentage, 8);
            let pct_label = format!("{:>3.0}%", b.percentage.round_dp(0));

            lines.push(Line::from(vec![
                Span::raw(format!("{cat_name} ")),
                Span::styled(bar, Style::default().fg(bar_color)),
                Span::raw(" "),
                Span::styled(
                    pct_label,
                    Style::default().fg(bar_color).add_modifier(Modifier::BOLD),
                ),
            ]));
        }

        if data.budget_statuses.len() > max_items {
            let remaining = data.budget_statuses.len() - max_items;
            lines.push(Line::from(Span::styled(
                format!("  (+{remaining} outros orçamentos)"),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn render_upcoming_panel(data: &DashboardData, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Próximos Vencimentos ")
        .border_style(Style::default().fg(Color::Magenta));

    let mut lines = Vec::new();

    if data.upcoming_items.is_empty() {
        lines.push(Line::from(Span::styled(
            "Nenhum vencimento pendente no momento.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let max_items = (area.height.saturating_sub(2)).max(2) as usize;
        for item in data.upcoming_items.iter().take(max_items) {
            let date_str = item.due_date.format("%d/%m").to_string();
            let amount_str = format_decimal_pt_br(item.amount.as_decimal());

            let (tag_str, tag_color) = if item.is_overdue {
                ("ATRASADO", Color::Red)
            } else {
                match item.kind {
                    UpcomingKind::CardInvoice => ("FATURA", Color::Yellow),
                    UpcomingKind::Expense => ("PENDENTE", Color::White),
                    UpcomingKind::Income => ("RECEITA", Color::Green),
                }
            };

            let max_desc_len = if area.width < 45 { 9 } else { 14 };
            let desc = if item.description.chars().count() > max_desc_len {
                let truncated: String = item
                    .description
                    .chars()
                    .take(max_desc_len.saturating_sub(1))
                    .collect();
                format!("{truncated}…")
            } else {
                format!("{:<width$}", item.description, width = max_desc_len)
            };

            lines.push(Line::from(vec![
                Span::styled(format!("{date_str} "), Style::default().fg(Color::DarkGray)),
                Span::raw(format!("{desc} ")),
                Span::styled(
                    format!("{amount_str} "),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("[{tag_str}]"),
                    Style::default().fg(tag_color).add_modifier(Modifier::BOLD),
                ),
            ]));
        }

        if data.upcoming_items.len() > max_items {
            let remaining = data.upcoming_items.len() - max_items;
            lines.push(Line::from(Span::styled(
                format!("  (+{remaining} outros vencimentos)"),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn format_progress_bar(percentage: Decimal, width: usize) -> String {
    let pct_f = percentage.to_f64().unwrap_or(0.0).clamp(0.0, 100.0);
    let filled = ((pct_f / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width.saturating_sub(filled);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

fn render_transactions_tab(model: &Model, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Barra de filtros / busca
            Constraint::Min(6),    // Tabela de lançamentos
            Constraint::Length(3), // Rodapé e paginação
        ])
        .split(area);

    render_transactions_filter_bar(model, frame, chunks[0]);
    render_transactions_table(model, frame, chunks[1]);
    render_transactions_pagination(model, frame, chunks[2]);

    // Modais sobrepostos
    if let Some(ref confirm) = model.transactions_state.delete_confirm {
        render_delete_modal(confirm, frame, area);
    } else if let Some(ref form) = model.transactions_state.form_modal {
        render_form_modal(form, frame, area);
    } else if let Some(ref filter) = model.transactions_state.filter_modal {
        render_filter_modal(filter, frame, area);
    }
}

fn render_transactions_filter_bar(model: &Model, frame: &mut Frame, area: Rect) {
    let state = &model.transactions_state;

    let content = if state.is_searching {
        Line::from(vec![
            Span::styled(
                "Buscar Descrição: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}_", state.search_query),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  [Enter] Aplicar  [Esc] Cancelar",
                Style::default().fg(Color::DarkGray),
            ),
        ])
    } else {
        let mut spans = vec![Span::styled(
            "Filtros Ativos: ",
            Style::default().fg(Color::Cyan),
        )];

        let mut has_filter = false;
        if !state.search_query.trim().is_empty() {
            spans.push(Span::styled(
                format!("Busca: \"{}\"  ", state.search_query.trim()),
                Style::default().fg(Color::Yellow),
            ));
            has_filter = true;
        }
        if let Some(ref m) = state.active_filters.month {
            spans.push(Span::raw(format!("Mês: {m}  ")));
            has_filter = true;
        }
        if let Some(ref a) = state.active_filters.account {
            spans.push(Span::raw(format!("Conta: {a}  ")));
            has_filter = true;
        }
        if let Some(ref c) = state.active_filters.category {
            spans.push(Span::raw(format!("Cat: {c}  ")));
            has_filter = true;
        }
        if let Some(ref k) = state.active_filters.kind {
            spans.push(Span::styled(
                format!("Tipo: {}  ", k.display_pt_br()),
                Style::default().fg(Color::Magenta),
            ));
            has_filter = true;
        }
        if let Some(ref s) = state.active_filters.status {
            spans.push(Span::styled(
                format!("Status: {}  ", s.display_pt_br()),
                Style::default().fg(Color::Green),
            ));
            has_filter = true;
        }
        if let Some(ref t) = state.active_filters.tag {
            spans.push(Span::raw(format!("Tag: #{t}  ")));
            has_filter = true;
        }

        if !has_filter {
            spans.push(Span::styled(
                "Nenhum (Todos os lançamentos)  ",
                Style::default().fg(Color::DarkGray),
            ));
        }

        spans.push(Span::styled(
            "[/] Buscar  [f] Filtrar",
            Style::default().fg(Color::DarkGray),
        ));
        Line::from(spans)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Filtros e Busca ")
        .border_style(if state.is_searching {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Blue)
        });

    let p = Paragraph::new(content).block(block);
    frame.render_widget(p, area);
}

fn render_transactions_table(model: &Model, frame: &mut Frame, area: Rect) {
    let state = &model.transactions_state;
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Lançamentos (Use Setas/j/k para navegar, Space para selecionar) ")
        .border_style(Style::default().fg(Color::Magenta));

    let mut lines = Vec::new();

    // Cabeçalho da tabela
    let header_line = Line::from(vec![
        Span::styled(
            "Sel ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Data       ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Tipo ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Conta     ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Categoria ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Descrição       ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "       Valor ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Status ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    lines.push(header_line);
    lines.push(Line::from(
        "────────────────────────────────────────────────────────────────────────────",
    ));

    if state.items.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Nenhum lançamento encontrado para os filtros atuais.",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            "  Pressione 'a' para adicionar um novo lançamento ou 'f' para ajustar os filtros.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (i, tx) in state.items.iter().enumerate() {
            let is_cursor = i == state.cursor_index;
            let is_selected = state.selected_ids.contains(&tx.id);

            let sel_span = if is_selected {
                Span::styled(
                    "[x] ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled("[ ] ", Style::default().fg(Color::DarkGray))
            };

            let date_str = format!("{} ", tx.date.format("%d/%m/%Y"));
            let (kind_str, kind_color) = match tx.kind {
                TransactionKind::Income => ("REC  ", Color::Green),
                TransactionKind::Expense => ("DESP ", Color::Red),
            };

            let acc_str = if tx.account_name.chars().count() > 9 {
                let s: String = tx.account_name.chars().take(8).collect();
                format!("{s}… ")
            } else {
                format!("{:<9} ", tx.account_name)
            };

            let cat_str = if tx.category_name.chars().count() > 9 {
                let s: String = tx.category_name.chars().take(8).collect();
                format!("{s}… ")
            } else {
                format!("{:<9} ", tx.category_name)
            };

            let desc_str = if tx.description.chars().count() > 15 {
                let s: String = tx.description.chars().take(14).collect();
                format!("{s}… ")
            } else {
                format!("{:<15} ", tx.description)
            };

            let val_str = format!("{:>12} ", format_decimal_pt_br(tx.amount.as_decimal()));
            let (status_str, status_color) = match tx.status {
                TransactionStatus::Paid => ("[Pago] ", Color::Green),
                TransactionStatus::Pending => ("[Pend] ", Color::Yellow),
            };

            let mut row_spans = vec![
                sel_span,
                Span::raw(date_str),
                Span::styled(kind_str, Style::default().fg(kind_color)),
                Span::raw(acc_str),
                Span::raw(cat_str),
                Span::raw(desc_str),
                Span::styled(
                    val_str,
                    Style::default().fg(kind_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(status_str, Style::default().fg(status_color)),
            ];

            let row_style = if is_cursor {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            if is_cursor {
                row_spans.insert(0, Span::styled("▶", Style::default().fg(Color::Cyan)));
            } else {
                row_spans.insert(0, Span::raw(" "));
            }

            lines.push(Line::from(row_spans).style(row_style));
        }
    }

    let p = Paragraph::new(lines).block(block);
    frame.render_widget(p, area);
}

fn render_transactions_pagination(model: &Model, frame: &mut Frame, area: Rect) {
    let state = &model.transactions_state;

    let info_str = format!(
        " Página {} de {} (Total: {} lançamentos)",
        state.page,
        state.total_pages.max(1),
        state.total_count
    );

    let shortcuts_str = " | [p] Pagar [a] Novo [e] Edit [d] Del [ ] Pág ";

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let line = Line::from(vec![
        Span::styled(
            info_str,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(shortcuts_str, Style::default().fg(Color::DarkGray)),
    ]);

    let p = Paragraph::new(line).block(block);
    frame.render_widget(p, area);
}

fn render_delete_modal(confirm: &DeleteConfirmState, frame: &mut Frame, area: Rect) {
    let popup_area = centered_rect(58, 8, area);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Confirmar Exclusão ")
        .border_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD));

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            &confirm.prompt_message,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                " [s] Confirmar Exclusão ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("    "),
            Span::styled(
                " [n / Esc] Cancelar ",
                Style::default().fg(Color::White).bg(Color::DarkGray),
            ),
        ]),
    ];

    let p = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center);
    frame.render_widget(p, popup_area);
}

fn render_form_modal(form: &TransactionFormState, frame: &mut Frame, area: Rect) {
    let popup_area = centered_rect(62, 17, area);
    frame.render_widget(Clear, popup_area);

    let title = match form.mode {
        FormMode::Add => " Adicionar Lançamento ",
        FormMode::Edit => " Editar Lançamento ",
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    let render_field_line = |label: &str, val: &str, field: FormField| {
        let is_focused = form.focused_field == field;
        let label_style = if is_focused {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let val_display = if is_focused {
            format!("{val}_")
        } else {
            val.to_string()
        };
        let val_style = if is_focused {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        Line::from(vec![
            Span::styled(format!("{:<14}", label), label_style),
            Span::styled(format!("[ {:<38} ]", val_display), val_style),
        ])
    };

    let kind_display = match form.kind {
        TransactionKind::Expense => "Despesa (use Espaço/Setas para alternar)",
        TransactionKind::Income => "Receita (use Espaço/Setas para alternar)",
    };

    let status_display = match form.status {
        TransactionStatus::Paid => "Pago (use Espaço/Setas para alternar)",
        TransactionStatus::Pending => "Pendente (use Espaço/Setas para alternar)",
    };

    let mut lines = vec![
        render_field_line("Tipo:", kind_display, FormField::Kind),
        render_field_line("Conta:", &form.account_input, FormField::Account),
        render_field_line("Categoria:", &form.category_input, FormField::Category),
        render_field_line("Valor (R$):", &form.amount_input, FormField::Amount),
        render_field_line("Data (AAAA-MM-DD):", &form.date_input, FormField::Date),
        render_field_line(
            "Descrição:",
            &form.description_input,
            FormField::Description,
        ),
        render_field_line("Status:", status_display, FormField::Status),
        Line::from(""),
    ];

    if let Some(ref err) = form.validation_error {
        lines.push(Line::from(Span::styled(
            format!("ERRO: {err}"),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            " [Tab] Navegar campos  [Enter] Salvar  [Esc] Cancelar ",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let p = Paragraph::new(lines).block(block);
    frame.render_widget(p, popup_area);
}

fn render_filter_modal(filter: &FilterModalState, frame: &mut Frame, area: Rect) {
    let popup_area = centered_rect(58, 15, area);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Filtrar Lançamentos ")
        .border_style(
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        );

    let render_filter_field = |label: &str, val: &str, field: FilterField| {
        let is_focused = filter.focused_field == field;
        let label_style = if is_focused {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let val_display = if is_focused {
            format!("{val}_")
        } else {
            val.to_string()
        };
        let val_style = if is_focused {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        Line::from(vec![
            Span::styled(format!("{:<14}", label), label_style),
            Span::styled(format!("[ {:<34} ]", val_display), val_style),
        ])
    };

    let kind_display = match filter.kind_selection {
        None => "Todos os tipos (Espaço para alterar)",
        Some(TransactionKind::Expense) => "Apenas Despesas",
        Some(TransactionKind::Income) => "Apenas Receitas",
    };

    let status_display = match filter.status_selection {
        None => "Todos os status (Espaço para alterar)",
        Some(TransactionStatus::Paid) => "Apenas Pagos",
        Some(TransactionStatus::Pending) => "Apenas Pendentes",
    };

    let lines = vec![
        render_filter_field("Mês (AAAA-MM):", &filter.month_input, FilterField::Month),
        render_filter_field("Conta:", &filter.account_input, FilterField::Account),
        render_filter_field("Categoria:", &filter.category_input, FilterField::Category),
        render_filter_field("Tipo:", kind_display, FilterField::Kind),
        render_filter_field("Status:", status_display, FilterField::Status),
        render_filter_field("Tag (#nome):", &filter.tag_input, FilterField::Tag),
        Line::from(""),
        Line::from(Span::styled(
            " [Tab] Navegar  [Enter] Aplicar Filtros  [Esc] Cancelar ",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let p = Paragraph::new(lines).block(block);
    frame.render_widget(p, popup_area);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let popup_width = width.min(area.width.saturating_sub(2));
    let popup_height = height.min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    Rect::new(x, y, popup_width, popup_height)
}

fn render_reports_tab(model: &Model, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Barra de controle de sub-visões e filtros
            Constraint::Min(6),    // Conteúdo da sub-visão ativa
        ])
        .split(area);

    render_reports_control_bar(model, frame, chunks[0]);

    match model.reports_state.active_subview {
        ReportSubView::Categories => render_reports_categories_view(model, frame, chunks[1]),
        ReportSubView::MonthlyEvolution => render_reports_evolution_view(model, frame, chunks[1]),
        ReportSubView::Comparison => render_reports_comparison_view(model, frame, chunks[1]),
    }

    if let Some(ref modal) = model.reports_state.period_modal {
        render_reports_period_modal(modal, frame, area);
    }
}

fn render_reports_control_bar(model: &Model, frame: &mut Frame, area: Rect) {
    let state = &model.reports_state;
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Relatórios Financeiros ")
        .border_style(Style::default().fg(Color::Blue));

    let subview_spans = [
        Span::styled(
            "[1] Categorias  ",
            if state.active_subview == ReportSubView::Categories {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        ),
        Span::styled(
            "[2] Evolução Mensal  ",
            if state.active_subview == ReportSubView::MonthlyEvolution {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        ),
        Span::styled(
            "[3] Comparativo",
            if state.active_subview == ReportSubView::Comparison {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        ),
    ];

    let pending_status = if state.include_pending {
        Span::styled(
            " [Previstos: ON] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(" [Previstos: OFF] ", Style::default().fg(Color::DarkGray))
    };

    let controls = Line::from(vec![
        Span::styled(
            "Sub-visões: ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        subview_spans[0].clone(),
        subview_spans[1].clone(),
        subview_spans[2].clone(),
        Span::raw(" | "),
        Span::styled(
            format!("◄ {} ►", state.reference_month),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        pending_status,
        Span::styled(
            "[ [ / ] ] Mês [i] Previstos [p] Período",
            Style::default().fg(Color::DarkGray),
        ),
    ]);

    let p = Paragraph::new(controls).block(block);
    frame.render_widget(p, area);
}

fn render_reports_categories_view(model: &Model, frame: &mut Frame, area: Rect) {
    let state = &model.reports_state;
    let total_str = state
        .data
        .as_ref()
        .map(|d| format_decimal_pt_br(d.category_report.total_amount.as_decimal()))
        .unwrap_or_else(|| "R$ 0,00".to_string());

    let title = format!(
        " Gastos por Categoria — {} (Total: {}) ",
        state.reference_month, total_str
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(Color::Magenta));

    let Some(data) = &state.data else {
        let p = Paragraph::new(Line::from(Span::styled(
            " Carregando dados ou pressione 'r' para atualizar...",
            Style::default().fg(Color::DarkGray),
        )))
        .block(block);
        frame.render_widget(p, area);
        return;
    };

    let items = &data.category_report.items;
    if items.is_empty() {
        let p = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Nenhum gasto registrado para as categorias neste período.",
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .block(block);
        frame.render_widget(p, area);
        return;
    }

    let mut lines = Vec::new();
    // Cabeçalho da tabela: Sel(2) + Cat(16) + Pct(7) + Valor(13) + Barra(36) = 74 caracteres <= 78!
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            "Categoria       ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "      % ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "        Valor ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Distribuição",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(
        "────────────────────────────────────────────────────────────────────────────",
    ));

    // Janela de exibição com rolagem
    let visible_rows = (area.height.saturating_sub(4)).max(1) as usize;
    let cursor = state.category_cursor.min(items.len().saturating_sub(1));
    let start_idx = if cursor >= visible_rows {
        cursor - visible_rows + 1
    } else {
        0
    };

    for (i, item) in items.iter().enumerate().skip(start_idx).take(visible_rows) {
        let is_cursor = i == cursor;
        let prefix = if is_cursor { "▶ " } else { "  " };

        let name = if item.category_name.chars().count() > 15 {
            let truncated: String = item.category_name.chars().take(14).collect();
            format!("{truncated}…")
        } else {
            format!("{:<15}", item.category_name)
        };

        let pct = item.percentage.to_f64().unwrap_or(0.0);
        let pct_str = format!("{:>6.1}% ", pct).replace('.', ",");
        let val_str = format!(
            "{:>12} ",
            format_decimal_pt_br(item.total_amount.as_decimal())
        );

        // Barra de progresso com 33 caracteres
        let bar_width = 33;
        let filled = ((pct / 100.0) * bar_width as f64).round() as usize;
        let filled = filled.min(bar_width);
        let empty = bar_width.saturating_sub(filled);
        let bar_str = format!("[{}{}]", "█".repeat(filled), "░".repeat(empty));

        let row_style = if is_cursor {
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let row_spans = vec![
            Span::styled(prefix, Style::default().fg(Color::Cyan)),
            Span::raw(format!("{name} ")),
            Span::styled(pct_str, Style::default().fg(Color::Yellow)),
            Span::styled(
                val_str,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(bar_str, Style::default().fg(Color::Green)),
        ];

        lines.push(Line::from(row_spans).style(row_style));
    }

    let p = Paragraph::new(lines).block(block);
    frame.render_widget(p, area);
}

fn render_reports_evolution_view(model: &Model, frame: &mut Frame, area: Rect) {
    let state = &model.reports_state;
    let Some(data) = &state.data else {
        let p = Paragraph::new(Line::from(Span::styled(
            " Carregando dados de evolução...",
            Style::default().fg(Color::DarkGray),
        )))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Evolução Mensal "),
        );
        frame.render_widget(p, area);
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Sparklines duplas (altura 3)
            Constraint::Min(9),    // Tabela resumo mês a mês (altura 9)
        ])
        .split(area);

    // Painel superior: Sparklines
    let spark_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);

    // Extrair dados numéricos para sparklines (em inteiros positivos)
    let income_data: Vec<u64> = data
        .monthly_history
        .iter()
        .map(|m| {
            m.total_income
                .as_decimal()
                .to_f64()
                .unwrap_or(0.0)
                .max(0.0)
                .round() as u64
        })
        .collect();

    let expense_data: Vec<u64> = data
        .monthly_history
        .iter()
        .map(|m| {
            m.total_expense
                .as_decimal()
                .to_f64()
                .unwrap_or(0.0)
                .max(0.0)
                .round() as u64
        })
        .collect();

    let income_sparkline = Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Receitas (Últimos 6 Meses) ")
                .border_style(Style::default().fg(Color::Green)),
        )
        .data(&income_data)
        .style(Style::default().fg(Color::Green));

    let expense_sparkline = Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Despesas (Últimos 6 Meses) ")
                .border_style(Style::default().fg(Color::Red)),
        )
        .data(&expense_data)
        .style(Style::default().fg(Color::Red));

    frame.render_widget(income_sparkline, spark_cols[0]);
    frame.render_widget(expense_sparkline, spark_cols[1]);

    // Painel inferior: Tabela detalhada mês a mês
    let table_block = Block::default()
        .borders(Borders::ALL)
        .title(" Histórico Mês a Mês ")
        .border_style(Style::default().fg(Color::Cyan));

    let mut table_lines = Vec::new();
    table_lines.push(Line::from(vec![
        Span::styled(
            " Mês       ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "       Receitas ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "       Despesas ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "    Saldo Líquido ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "    Poupança",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    for item in &data.monthly_history {
        let is_current = item.month == state.reference_month;
        let prefix = if is_current { "► " } else { "  " };

        let m_str = format!("{prefix}{:<7} ", item.month);
        let inc_str = format!(
            "{:>14} ",
            format_decimal_pt_br(item.total_income.as_decimal())
        );
        let exp_str = format!(
            "{:>14} ",
            format_decimal_pt_br(item.total_expense.as_decimal())
        );

        let net_prefix = if item.net_balance >= Decimal::ZERO {
            "+"
        } else {
            ""
        };
        let net_str = format!(
            "{:>15} ",
            format!("{net_prefix}{}", format_decimal_pt_br(item.net_balance))
        );
        let net_color = if item.net_balance >= Decimal::ZERO {
            Color::Green
        } else {
            Color::Red
        };

        let sav_str =
            format!("{:>11.1}%", item.savings_rate.to_f64().unwrap_or(0.0)).replace('.', ",");

        let row_style = if is_current {
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        table_lines.push(
            Line::from(vec![
                Span::styled(
                    m_str,
                    Style::default().fg(if is_current {
                        Color::Yellow
                    } else {
                        Color::White
                    }),
                ),
                Span::styled(inc_str, Style::default().fg(Color::Green)),
                Span::styled(exp_str, Style::default().fg(Color::Red)),
                Span::styled(
                    net_str,
                    Style::default().fg(net_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(sav_str, Style::default().fg(Color::Cyan)),
            ])
            .style(row_style),
        );
    }

    let p_table = Paragraph::new(table_lines).block(table_block);
    frame.render_widget(p_table, chunks[1]);
}

fn render_reports_comparison_view(model: &Model, frame: &mut Frame, area: Rect) {
    let state = &model.reports_state;
    let Some(data) = &state.data else {
        let p = Paragraph::new(Line::from(Span::styled(
            " Carregando dados comparativos...",
            Style::default().fg(Color::DarkGray),
        )))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Comparativo "),
        );
        frame.render_widget(p, area);
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Resumo dos Totais (Receitas, Despesas, Saldo)
            Constraint::Min(6),    // Tabela por Categorias de Despesa
        ])
        .split(area);

    // 1. Resumo dos Totais
    let totals_block = Block::default()
        .borders(Borders::ALL)
        .title(" Comparativo dos Totais Gerais ")
        .border_style(Style::default().fg(Color::Cyan));

    let mut tot_lines = Vec::new();
    if data.comparison_totals.len() >= 2 {
        let prev = &data.comparison_totals[0];
        let curr = &data.comparison_totals[1];

        let inc_diff = curr.total_income.as_decimal() - prev.total_income.as_decimal();
        let exp_diff = curr.total_expense.as_decimal() - prev.total_expense.as_decimal();
        let net_diff = curr.net_balance - prev.net_balance;

        let inc_sign = if inc_diff >= Decimal::ZERO { "+" } else { "" };
        let exp_sign = if exp_diff >= Decimal::ZERO { "+" } else { "" };
        let net_sign = if net_diff >= Decimal::ZERO { "+" } else { "" };

        tot_lines.push(Line::from(vec![
            Span::styled(
                "Receitas: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "{} (Ant.) vs {} (Atual)  ",
                format_decimal_pt_br(prev.total_income.as_decimal()),
                format_decimal_pt_br(curr.total_income.as_decimal())
            )),
            Span::styled(
                format!("Delta: {inc_sign}{}", format_decimal_pt_br(inc_diff)),
                Style::default().fg(if inc_diff >= Decimal::ZERO {
                    Color::Green
                } else {
                    Color::Red
                }),
            ),
        ]));

        tot_lines.push(Line::from(vec![
            Span::styled(
                "Despesas: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "{} (Ant.) vs {} (Atual)  ",
                format_decimal_pt_br(prev.total_expense.as_decimal()),
                format_decimal_pt_br(curr.total_expense.as_decimal())
            )),
            Span::styled(
                format!("Delta: {exp_sign}{}", format_decimal_pt_br(exp_diff)),
                Style::default().fg(if exp_diff <= Decimal::ZERO {
                    Color::Green
                } else {
                    Color::Red
                }),
            ),
        ]));

        tot_lines.push(Line::from(vec![
            Span::styled(
                "Saldo:    ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "{} (Ant.) vs {} (Atual)  ",
                format_decimal_pt_br(prev.net_balance),
                format_decimal_pt_br(curr.net_balance)
            )),
            Span::styled(
                format!("Delta: {net_sign}{}", format_decimal_pt_br(net_diff)),
                Style::default().fg(if net_diff >= Decimal::ZERO {
                    Color::Green
                } else {
                    Color::Red
                }),
            ),
        ]));
    } else {
        tot_lines.push(Line::from(Span::styled(
            "Dados insuficientes para comparação de totais de 2 meses.",
            Style::default().fg(Color::DarkGray),
        )));
    }
    let p_tot = Paragraph::new(tot_lines).block(totals_block);
    frame.render_widget(p_tot, chunks[0]);

    // 2. Tabela de Categorias
    let cat_block = Block::default()
        .borders(Borders::ALL)
        .title(" Variação de Despesas por Categoria ")
        .border_style(Style::default().fg(Color::Magenta));

    let rows = &data.comparison_categories.rows;
    let mut cat_lines = Vec::new();

    cat_lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            "Categoria       ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "      Mês Ant. ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "     Mês Atual ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "      Variação R$ ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "    Var. %",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    cat_lines.push(Line::from(
        "────────────────────────────────────────────────────────────────────────────",
    ));

    if rows.is_empty() {
        cat_lines.push(Line::from(Span::styled(
            "  Nenhuma despesa para comparar entre os períodos selecionados.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        let visible_rows = (chunks[1].height.saturating_sub(4)).max(1) as usize;
        let cursor = state.comparison_cursor.min(rows.len().saturating_sub(1));
        let start_idx = if cursor >= visible_rows {
            cursor - visible_rows + 1
        } else {
            0
        };

        for (i, row) in rows.iter().enumerate().skip(start_idx).take(visible_rows) {
            let is_cursor = i == cursor;
            let prefix = if is_cursor { "▶ " } else { "  " };

            let name = if row.category_name.chars().count() > 15 {
                let truncated: String = row.category_name.chars().take(14).collect();
                format!("{truncated}…")
            } else {
                format!("{:<15}", row.category_name)
            };

            let val_ant = row
                .monthly_amounts
                .first()
                .map(|(_, m)| format_decimal_pt_br(m.as_decimal()))
                .unwrap_or_else(|| "0,00".to_string());
            let val_curr = row
                .monthly_amounts
                .get(1)
                .map(|(_, m)| format_decimal_pt_br(m.as_decimal()))
                .unwrap_or_else(|| "0,00".to_string());

            let diff_sign = if row.absolute_diff >= Decimal::ZERO {
                "+"
            } else {
                ""
            };
            let diff_str = format!(
                "{:>16} ",
                format!("{diff_sign}{}", format_decimal_pt_br(row.absolute_diff))
            );
            let diff_color = if row.absolute_diff <= Decimal::ZERO {
                Color::Green // Despesa diminuiu: economia!
            } else {
                Color::Red // Despesa aumentou: alerta!
            };

            let pct_str = if let Some(pct) = row.percent_diff {
                let sign = if pct >= Decimal::ZERO { "+" } else { "" };
                let s = format!("{sign}{:.1}%", pct).replace('.', ",");
                format!("{:>9}", s)
            } else {
                format!("{:>9}", "N/A")
            };

            let row_style = if is_cursor {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            cat_lines.push(
                Line::from(vec![
                    Span::styled(prefix, Style::default().fg(Color::Cyan)),
                    Span::raw(format!("{name} ")),
                    Span::styled(
                        format!("{:>14} ", val_ant),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        format!("{:>14} ", val_curr),
                        Style::default().fg(Color::White),
                    ),
                    Span::styled(
                        diff_str,
                        Style::default().fg(diff_color).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(pct_str, Style::default().fg(diff_color)),
                ])
                .style(row_style),
            );
        }
    }

    let p_cat = Paragraph::new(cat_lines).block(cat_block);
    frame.render_widget(p_cat, chunks[1]);
}

fn render_reports_period_modal(modal: &PeriodModalState, frame: &mut Frame, area: Rect) {
    let popup_width = 46.min(area.width.saturating_sub(4));
    let popup_height = 8.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Selecionar Período ")
        .border_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                " Mês de Referência (AAAA-MM): ",
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                format!("{}_", modal.input_month),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
    ];

    if let Some(ref err) = modal.validation_error {
        lines.push(Line::from(Span::styled(
            format!(" {err}"),
            Style::default().fg(Color::Red),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            " [Enter] Confirmar  [Esc] Cancelar",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let p = Paragraph::new(lines).block(block);
    frame.render_widget(p, popup_area);
}

fn render_budgets_tab(_model: &Model, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" 4: Orçamentos ")
        .border_style(Style::default().fg(Color::Yellow));

    let text = vec![
        Line::from(Span::styled(
            "Gestão de Orçamentos e Tetos de Gastos",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("O resumo em tempo real já está visível no painel do Dashboard (Aba 1)."),
        Line::from("Para definir novos tetos, utilize o comando CLI 'finctl budget set'."),
    ];

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn render_goals_tab(model: &Model, frame: &mut Frame, area: Rect) {
    let state = &model.goals_state;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Seletor de subvisões (Metas / Projeção)
            Constraint::Min(6),    // Conteúdo principal
        ])
        .split(area);

    render_goals_subview_tabs(model, frame, chunks[0]);

    match state.active_subview {
        GoalsSubView::Goals => render_goals_subview(model, frame, chunks[1]),
        GoalsSubView::Forecast => render_forecast_view(model, frame, chunks[1]),
    }

    if let Some(ref modal) = state.contribution_modal {
        render_contribution_modal(modal, frame, area);
    }
}

fn render_goals_subview_tabs(model: &Model, frame: &mut Frame, area: Rect) {
    let state = &model.goals_state;
    let theme = model.theme;

    let subviews = [
        ("1: Metas de Economia", GoalsSubView::Goals),
        ("2: Projeção de Fluxo de Caixa", GoalsSubView::Forecast),
    ];

    let titles: Vec<Line> = subviews
        .iter()
        .map(|(label, sv)| {
            if *sv == state.active_subview {
                Line::from(vec![
                    Span::styled(" [", Style::default().fg(theme.border_focus)),
                    Span::styled(
                        *label,
                        Style::default()
                            .fg(theme.tab_active_fg)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("] ", Style::default().fg(theme.border_focus)),
                ])
            } else {
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(*label, Style::default().fg(theme.tab_inactive_fg)),
                    Span::raw("  "),
                ])
            }
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Aba 5: Metas e Projeção [Alterne com 1 / 2 ou 'v'] ")
        .border_style(Style::default().fg(theme.border));

    let tabs = Tabs::new(titles)
        .block(block)
        .select(match state.active_subview {
            GoalsSubView::Goals => 0,
            GoalsSubView::Forecast => 1,
        })
        .style(Style::default().fg(theme.fg_muted))
        .highlight_style(
            Style::default()
                .fg(theme.tab_active_fg)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_widget(tabs, area);
}

fn render_goals_subview(model: &Model, frame: &mut Frame, area: Rect) {
    let state = &model.goals_state;
    let theme = model.theme;

    if state.goals.is_empty() {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Metas de Economia ")
            .border_style(Style::default().fg(theme.border));

        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "Nenhuma meta de economia encontrada.",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("• Para sincronizar com o banco de dados, pressione 'r'."),
            Line::from("• Para criar uma nova meta, utilize o CLI: 'finctl goal add --name \"Reserva\" --target 5000'."),
        ];

        let p = Paragraph::new(text).block(block);
        frame.render_widget(p, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),    // Lista/Tabela de Metas
            Constraint::Length(8), // Painel de Detalhes da Meta Selecionada
        ])
        .split(area);

    // Tabela de Metas
    let table_block = Block::default()
        .borders(Borders::ALL)
        .title(format!(
            " Metas Cadastradas ({}) [Use j/k para selecionar, c/a para aporte] ",
            state.goals.len()
        ))
        .border_style(Style::default().fg(Color::Cyan));

    let mut table_lines = Vec::new();
    table_lines.push(Line::from(vec![
        Span::styled(
            "   ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Nome               ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Tipo     ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "        Alvo ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "        Atual ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "     Restante ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Progresso       ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "     % ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " Data Alvo  ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "   Aporte Nec. ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Status",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    let visible_rows = chunks[0].height.saturating_sub(3) as usize;
    let selected_idx = state
        .selected_goal_index
        .min(state.goals.len().saturating_sub(1));
    let start_idx = if visible_rows > 0 && selected_idx >= visible_rows {
        selected_idx.saturating_sub(visible_rows.saturating_sub(1))
    } else {
        0
    };

    for (i, p) in state
        .goals
        .iter()
        .enumerate()
        .skip(start_idx)
        .take(visible_rows.max(1))
    {
        let is_selected = i == selected_idx;
        let prefix = if is_selected { "► " } else { "  " };

        let name = if p.goal.name.chars().count() > 18 {
            let truncated: String = p.goal.name.chars().take(17).collect();
            format!("{truncated}…")
        } else {
            format!("{:<18}", p.goal.name)
        };

        let kind = if p.goal.is_account_linked() {
            "Conta   "
        } else {
            "Manual  "
        };

        let target_str = format!(
            "{:>12} ",
            format_decimal_pt_br(p.goal.target_amount.as_decimal())
        );
        let current_str = format!(
            "{:>13} ",
            format_decimal_pt_br(p.current_amount.as_decimal())
        );
        let remaining_str = format!(
            "{:>13} ",
            format_decimal_pt_br(p.remaining_amount.as_decimal())
        );

        let bar = format_progress_bar(p.percentage, 10);
        let pct_str = format!("{:>6.1}% ", p.percentage);

        let target_date_str = p
            .goal
            .target_date
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "    —     ".to_string());
        let date_col = format!("{:^12}", target_date_str);

        let needed_str = p
            .monthly_needed
            .map(|m| format!("{:>12}/mês ", format_decimal_pt_br(m.as_decimal())))
            .unwrap_or_else(|| "          —     ".to_string());

        let status_color = if p.is_completed {
            Color::Green
        } else {
            Color::Cyan
        };
        let status_text = if p.is_completed {
            "Concluída"
        } else {
            "Ativa"
        };

        let row_style = if is_selected {
            Style::default()
                .bg(theme.table_selected_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        table_lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(Color::Yellow)),
            Span::styled(format!("{name} "), row_style.fg(theme.fg)),
            Span::styled(kind, row_style.fg(theme.fg_muted)),
            Span::styled(target_str, row_style.fg(theme.fg)),
            Span::styled(current_str, row_style.fg(Color::Green)),
            Span::styled(
                remaining_str,
                row_style.fg(if p.remaining_amount.as_decimal() > Decimal::ZERO {
                    Color::Yellow
                } else {
                    Color::Green
                }),
            ),
            Span::styled(format!("{bar} "), row_style.fg(Color::Cyan)),
            Span::styled(pct_str, row_style.fg(Color::Cyan)),
            Span::styled(date_col, row_style.fg(theme.fg_muted)),
            Span::styled(needed_str, row_style.fg(theme.fg)),
            Span::styled(status_text, row_style.fg(status_color)),
        ]));
    }

    let table_p = Paragraph::new(table_lines).block(table_block);
    frame.render_widget(table_p, chunks[0]);

    // Painel de Detalhes da Meta Selecionada
    if let Some(selected_goal) = state.goals.get(selected_idx) {
        render_goal_details_panel(selected_goal, frame, chunks[1], theme);
    }
}

fn render_goal_details_panel(
    goal_prog: &GoalProgress,
    frame: &mut Frame,
    area: Rect,
    theme: crate::theme::Theme,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Detalhes da Meta Selecionada ")
        .border_style(Style::default().fg(Color::Green));

    let mut lines = Vec::new();

    let kind_desc = if goal_prog.goal.is_account_linked() {
        "Conta bancária vinculada (saldo real)"
    } else {
        "Aportes manuais (registre com tecla [c] ou [a])"
    };

    let status_str = if goal_prog.is_completed {
        "Concluída"
    } else {
        "Em andamento (Ativa)"
    };

    let goal_id_str = goal_prog.goal.id.to_string();
    let goal_id_short = if goal_id_str.len() >= 8 {
        &goal_id_str[..8]
    } else {
        &goal_id_str
    };

    lines.push(Line::from(vec![
        Span::styled(
            "Meta: ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            &goal_prog.goal.name,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  |  ID: ", Style::default().fg(theme.fg_muted)),
        Span::styled(goal_id_short, Style::default().fg(theme.fg_muted)),
        Span::styled("  |  Controle: ", Style::default().fg(theme.fg_muted)),
        Span::styled(kind_desc, Style::default().fg(Color::Yellow)),
        Span::styled("  |  Status: ", Style::default().fg(theme.fg_muted)),
        Span::styled(
            status_str,
            Style::default().fg(if goal_prog.is_completed {
                Color::Green
            } else {
                Color::Cyan
            }),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::styled("Alvo: ", Style::default().fg(theme.fg_muted)),
        Span::styled(
            format!(
                "{}   ",
                format_decimal_pt_br(goal_prog.goal.target_amount.as_decimal())
            ),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("Atual: ", Style::default().fg(theme.fg_muted)),
        Span::styled(
            format!(
                "{}   ",
                format_decimal_pt_br(goal_prog.current_amount.as_decimal())
            ),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("Restante: ", Style::default().fg(theme.fg_muted)),
        Span::styled(
            format!(
                "{}   ",
                format_decimal_pt_br(goal_prog.remaining_amount.as_decimal())
            ),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("Concluído: ", Style::default().fg(theme.fg_muted)),
        Span::styled(
            format!("{:.1}%", goal_prog.percentage),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    let target_date_desc = goal_prog
        .goal
        .target_date
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "Não definida".to_string());

    let needed_desc = goal_prog
        .monthly_needed
        .map(|m| format!("{}/mês", format_decimal_pt_br(m.as_decimal())))
        .unwrap_or_else(|| "n/d".to_string());

    let rate_desc = goal_prog
        .recent_monthly_rate
        .map(|m| format!("{}/mês", format_decimal_pt_br(m.as_decimal())))
        .unwrap_or_else(|| "Histórico insuficiente (< 30 dias)".to_string());

    let est_desc = goal_prog
        .estimated_completion_date
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "n/d".to_string());

    lines.push(Line::from(vec![
        Span::styled("Data Alvo: ", Style::default().fg(theme.fg_muted)),
        Span::styled(
            format!("{target_date_desc}   "),
            Style::default().fg(Color::White),
        ),
        Span::styled(
            "Aporte Mensal Necessário: ",
            Style::default().fg(theme.fg_muted),
        ),
        Span::styled(
            format!("{needed_desc}   "),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("Média Recente (90d): ", Style::default().fg(theme.fg_muted)),
        Span::styled(format!("{rate_desc}   "), Style::default().fg(Color::White)),
        Span::styled("Estimativa: ", Style::default().fg(theme.fg_muted)),
        Span::styled(est_desc, Style::default().fg(Color::Cyan)),
    ]));

    let action_hint = if !goal_prog.goal.is_account_linked() && !goal_prog.is_completed {
        "Pressione [c] ou [a] para registrar aporte manual  |  [2] Projeção de Fluxo de Caixa"
    } else if goal_prog.is_completed {
        "Meta concluída! Parabéns!  |  [2] Projeção de Fluxo de Caixa"
    } else {
        "Meta vinculada: atualizada automaticamente pelo saldo da conta  |  [2] Projeção de Fluxo de Caixa"
    };

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        action_hint,
        Style::default().fg(Color::DarkGray),
    )));

    let p = Paragraph::new(lines).block(block);
    frame.render_widget(p, area);
}

fn render_forecast_view(model: &Model, frame: &mut Frame, area: Rect) {
    let state = &model.goals_state;
    let theme = model.theme;

    let forecast = match state.forecast.as_ref() {
        Some(f) => f,
        None => {
            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Projeção de Fluxo de Caixa ")
                .border_style(Style::default().fg(theme.border));

            let text = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Projeção ainda não carregada.",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from("Pressione 'r' para sincronizar e calcular a projeção de saldo."),
            ];

            let p = Paragraph::new(text).block(block);
            frame.render_widget(p, area);
            return;
        }
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // KPI & Sparkline
            Constraint::Min(8),    // Tabela detalhada de períodos
        ])
        .split(area);

    // Painel superior: KPI & Gráfico de Saldo Futuro
    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(62), // Resumo e Indicadores Críticos
            Constraint::Percentage(38), // Sparkline de Trajetória
        ])
        .split(chunks[0]);

    let gran_label = match forecast.granularity {
        ForecastGranularity::Week => "Semanal",
        ForecastGranularity::Month => "Mensal",
    };

    let kpi_block = Block::default()
        .borders(Borders::ALL)
        .title(format!(
            " Indicadores da Projeção ({} Meses — {}) ",
            forecast.months, gran_label
        ))
        .border_style(Style::default().fg(Color::Cyan));

    let first_neg = forecast.periods.iter().find(|p| p.is_negative);

    let mut kpi_lines = Vec::new();
    kpi_lines.push(Line::from(vec![
        Span::styled("Saldo Inicial Atual: ", Style::default().fg(theme.fg_muted)),
        Span::styled(
            format!("{}   ", format_decimal_pt_br(forecast.initial_balance)),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("|  Metas Planejadas: ", Style::default().fg(theme.fg_muted)),
        if state.include_goals {
            Span::styled(
                "[g] ATIVADO (Aportes Deduzidos)",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                "[g] DESATIVADO (Ignorar Metas)",
                Style::default().fg(Color::Yellow),
            )
        },
    ]));

    if let Some(neg) = first_neg {
        kpi_lines.push(Line::from(vec![Span::styled(
            format!(
                "⚠️  PRIMEIRO PERÍODO NEGATIVO: {} (Saldo Projetado: {})",
                neg.period_label,
                format_decimal_pt_br(neg.closing_balance)
            ),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )]));
    } else {
        kpi_lines.push(Line::from(vec![Span::styled(
            "✅ Saldo projetado permanece POSITIVO em todo o horizonte",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )]));
    }

    kpi_lines.push(Line::from(""));
    kpi_lines.push(Line::from(Span::styled(
        "Atalhos: [g] Alternar Metas  |  [w/m] Granularidade  |  [+/-] Horizonte  |  [j/k] Rolar  |  [r] Recarregar",
        Style::default().fg(theme.fg_muted),
    )));

    let kpi_p = Paragraph::new(kpi_lines).block(kpi_block);
    frame.render_widget(kpi_p, top_chunks[0]);

    // Sparkline de Trajetória do Saldo Projetado
    let spark_block = Block::default()
        .borders(Borders::ALL)
        .title(" Trajetória do Saldo Futuro ")
        .border_style(Style::default().fg(if first_neg.is_some() {
            Color::Red
        } else {
            Color::Green
        }));

    let min_closing = forecast
        .periods
        .iter()
        .map(|p| p.closing_balance)
        .min()
        .unwrap_or(Decimal::ZERO);
    let shift_offset = if min_closing < Decimal::ZERO {
        min_closing.abs()
    } else {
        Decimal::ZERO
    };

    let spark_data: Vec<u64> = forecast
        .periods
        .iter()
        .map(|p| {
            ((p.closing_balance + shift_offset)
                .to_f64()
                .unwrap_or(0.0)
                .max(0.0)
                .round()) as u64
        })
        .collect();

    let sparkline = Sparkline::default()
        .block(spark_block)
        .data(&spark_data)
        .style(Style::default().fg(if first_neg.is_some() {
            Color::Yellow
        } else {
            Color::Green
        }));

    frame.render_widget(sparkline, top_chunks[1]);

    // Painel inferior: Tabela detalhada dos períodos
    let table_block = Block::default()
        .borders(Borders::ALL)
        .title(" Períodos Projetados (Entradas, Saídas e Saldos Futuros) ")
        .border_style(Style::default().fg(Color::Cyan));

    let mut table_lines = Vec::new();
    table_lines.push(Line::from(vec![
        Span::styled(
            "   ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Período           ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "   Saldo Inicial ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "    Entradas (+) ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "      Saídas (-) ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " Resultado Líquido ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "   Saldo Projetado ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    let visible_rows = chunks[1].height.saturating_sub(3) as usize;
    let selected_idx = state
        .forecast_cursor
        .min(forecast.periods.len().saturating_sub(1));
    let start_idx = if visible_rows > 0 && selected_idx >= visible_rows {
        selected_idx.saturating_sub(visible_rows.saturating_sub(1))
    } else {
        0
    };

    let mut first_neg_seen = false;

    for (i, p) in forecast
        .periods
        .iter()
        .enumerate()
        .skip(start_idx)
        .take(visible_rows.max(1))
    {
        let is_selected = i == selected_idx;
        let prefix = if is_selected { "► " } else { "  " };

        let period_label = format!("{:<18}", p.period_label);
        let open_str = format!("{:>16} ", format_decimal_pt_br(p.opening_balance));
        let inc_str = format!(
            "{:>16} ",
            format!("+ {}", format_decimal_pt_br(p.total_income))
        );
        let exp_str = format!(
            "{:>16} ",
            format!("- {}", format_decimal_pt_br(p.total_expense))
        );

        let net_prefix = if p.net_change >= Decimal::ZERO {
            "+"
        } else {
            "-"
        };
        let net_str = format!(
            "{:>18} ",
            format!("{net_prefix} {}", format_decimal_pt_br(p.net_change.abs()))
        );

        let is_first_neg = !first_neg_seen && p.is_negative;
        if is_first_neg {
            first_neg_seen = true;
        }

        let (closing_str, closing_style) = if is_first_neg {
            (
                format!("⚠️  {:>13} ", format_decimal_pt_br(p.closing_balance)),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )
        } else if p.is_negative {
            (
                format!("{:>16} ", format_decimal_pt_br(p.closing_balance)),
                Style::default().fg(Color::Red),
            )
        } else {
            (
                format!("{:>16} ", format_decimal_pt_br(p.closing_balance)),
                Style::default().fg(Color::Green),
            )
        };

        let row_style = if is_selected {
            Style::default()
                .bg(theme.table_selected_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let net_color = if p.net_change >= Decimal::ZERO {
            Color::Green
        } else {
            Color::Red
        };

        table_lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(Color::Yellow)),
            Span::styled(period_label, row_style.fg(theme.fg)),
            Span::styled(open_str, row_style.fg(theme.fg_muted)),
            Span::styled(inc_str, row_style.fg(Color::Green)),
            Span::styled(exp_str, row_style.fg(Color::Red)),
            Span::styled(net_str, row_style.fg(net_color)),
            Span::styled(closing_str, row_style.patch(closing_style)),
        ]));
    }

    let table_p = Paragraph::new(table_lines).block(table_block);
    frame.render_widget(table_p, chunks[1]);
}

fn render_contribution_modal(modal: &ContributionModalState, frame: &mut Frame, area: Rect) {
    let popup_area = centered_rect(60, 16, area);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Registrar Aporte Manual ")
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    let mut lines = Vec::new();
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(" Meta Alvo: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            &modal.goal_name,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(""));

    // Campo 1: Valor
    let is_amount = modal.focused_field == ContributionFormField::Amount;
    let amount_style = if is_amount {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    lines.push(Line::from(vec![
        Span::styled(
            if is_amount { " ► " } else { "   " },
            Style::default().fg(Color::Yellow),
        ),
        Span::styled("Valor (R$): ", Style::default().fg(Color::Cyan)),
        Span::styled(format!("[ {:<20} ]", modal.amount_input), amount_style),
    ]));

    // Campo 2: Data
    let is_date = modal.focused_field == ContributionFormField::Date;
    let date_style = if is_date {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    lines.push(Line::from(vec![
        Span::styled(
            if is_date { " ► " } else { "   " },
            Style::default().fg(Color::Yellow),
        ),
        Span::styled("Data (AAAA-MM-DD): ", Style::default().fg(Color::Cyan)),
        Span::styled(format!("[ {:<16} ]", modal.date_input), date_style),
    ]));

    // Campo 3: Observação
    let is_note = modal.focused_field == ContributionFormField::Note;
    let note_style = if is_note {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    lines.push(Line::from(vec![
        Span::styled(
            if is_note { " ► " } else { "   " },
            Style::default().fg(Color::Yellow),
        ),
        Span::styled("Observação: ", Style::default().fg(Color::Cyan)),
        Span::styled(format!("[ {:<26} ]", modal.note_input), note_style),
    ]));

    lines.push(Line::from(""));

    if let Some(ref err) = modal.validation_error {
        lines.push(Line::from(vec![
            Span::styled(
                " ✖ ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                err,
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        ]));
    } else {
        lines.push(Line::from(""));
    }

    lines.push(Line::from(Span::styled(
        " [Tab/↓/↑] Alternar Campo  |  [Enter] Confirmar  |  [Esc] Cancelar ",
        Style::default().fg(Color::DarkGray),
    )));

    let p = Paragraph::new(lines).block(block);
    frame.render_widget(p, popup_area);
}

fn render_footer(model: &Model, frame: &mut Frame, area: Rect) {
    let theme = model.theme;
    let contextual_shortcuts = build_contextual_shortcuts(model);

    let status = if let Some(ref err) = model.error_message {
        Line::from(vec![
            Span::styled(
                "ERRO: ",
                Style::default()
                    .fg(theme.danger)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(err, Style::default().fg(theme.danger)),
            Span::raw(" "),
        ])
    } else {
        Line::from(vec![
            Span::styled("Status: ", Style::default().fg(theme.fg_muted)),
            Span::styled(&model.status_message, Style::default().fg(theme.fg)),
            Span::raw(" "),
        ])
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.fg_muted));

    let right_width = 32.min(area.width.saturating_sub(45)).max(24);
    let footer_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(45), Constraint::Length(right_width)])
        .split(area);

    let shortcuts_p = Paragraph::new(Line::from(contextual_shortcuts)).block(block.clone());
    let status_p = Paragraph::new(status)
        .alignment(Alignment::Right)
        .block(block);

    frame.render_widget(shortcuts_p, footer_layout[0]);
    frame.render_widget(status_p, footer_layout[1]);
}

fn build_contextual_shortcuts(model: &Model) -> Vec<Span<'static>> {
    let theme = model.theme;
    let mut spans = Vec::new();

    let mut push_sc = |key: &'static str, desc: &'static str| {
        spans.push(Span::styled(
            format!(" [{key}] "),
            Style::default()
                .fg(theme.shortcut_key)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!("{desc} "),
            Style::default().fg(theme.shortcut_desc),
        ));
    };

    if model.is_help_open {
        push_sc("j/k/Setas", "Rolar");
        push_sc("?/Esc", "Fechar Ajuda");
    } else if model.active_tab == Tab::Transactions {
        if model.transactions_state.delete_confirm.is_some() {
            push_sc("Enter/s", "Confirmar");
            push_sc("Esc/n", "Cancelar");
        } else if model.transactions_state.form_modal.is_some()
            || model.transactions_state.filter_modal.is_some()
        {
            push_sc("Tab/Enter", "Próx");
            push_sc("Space", "Alternar");
            push_sc("Esc", "Cancelar");
        } else if model.transactions_state.is_searching {
            push_sc("Digitar", "Busca");
            push_sc("Enter/Esc", "Concluir");
        } else {
            push_sc("a", "Novo");
            push_sc("e", "Edit");
            push_sc("d", "Excl");
            push_sc("p", "Pagar");
            push_sc("Space", "Sel");
            push_sc("/", "Busca");
            push_sc("f", "Filtro");
            push_sc("Tab/1-5", "Mudar Aba");
        }
    } else if model.active_tab == Tab::Reports {
        if model.reports_state.period_modal.is_some() {
            push_sc("Digitar", "AAAA-MM");
            push_sc("Enter", "Aplicar");
            push_sc("Esc", "Cancelar");
        } else {
            push_sc("1-3", "Visão");
            push_sc("[ / ]", "Mês");
            push_sc("i", "Previstos");
            push_sc("p", "Período");
            push_sc("Tab/1-5", "Mudar Aba");
        }
    } else if model.active_tab == Tab::Goals {
        if model.goals_state.contribution_modal.is_some() {
            push_sc("Tab/↓/↑", "Campos");
            push_sc("Enter", "Confirmar");
            push_sc("Esc", "Cancelar");
        } else {
            push_sc("1/2", "Visão");
            match model.goals_state.active_subview {
                GoalsSubView::Goals => {
                    push_sc("c/a", "Aporte");
                    push_sc("j/k", "Navegar");
                }
                GoalsSubView::Forecast => {
                    push_sc("g", "Metas");
                    push_sc("w/m", "Sem/Mês");
                    push_sc("+/-", "Horizonte");
                    push_sc("j/k", "Navegar");
                }
            }
            push_sc("r", "Atualizar");
            push_sc("Tab/1-5", "Mudar Aba");
        }
    } else {
        // Dashboard ou abas gerais
        push_sc("q", "Sair");
        push_sc("Tab/1-5", "Abas");
        push_sc("r", "Atualizar");
        push_sc("t", "Tema");
        push_sc("?", "Ajuda");
    }

    spans
}

pub fn build_help_lines(model: &Model) -> Vec<Line<'static>> {
    let theme = model.theme;
    let mut lines = Vec::new();

    lines.push(Line::from(vec![Span::styled(
        "  MAPA COMPLETO DE ATALHOS DE TECLADO  ",
        Style::default()
            .fg(theme.header_fg)
            .bg(theme.header_bg)
            .add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(""));

    for category in ShortcutCategory::ALL {
        let shortcuts = ShortcutRegistry::by_category(category);
        if shortcuts.is_empty() {
            continue;
        }

        lines.push(Line::from(vec![Span::styled(
            format!("── {} ──", category.title()),
            Style::default()
                .fg(theme.border_focus)
                .add_modifier(Modifier::BOLD),
        )]));

        for sc in shortcuts {
            let key_str = format!("  {:18} ", sc.key);
            lines.push(Line::from(vec![
                Span::styled(
                    key_str,
                    Style::default()
                        .fg(theme.shortcut_key)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(sc.description, Style::default().fg(theme.shortcut_desc)),
            ]));
        }

        lines.push(Line::from(""));
    }

    lines.push(Line::from(vec![
        Span::styled(
            "Dica: ",
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Pressione 't' para alternar o tema (Claro/Escuro). Pressione '?' ou 'Esc' para fechar.",
            Style::default().fg(theme.fg_muted),
        ),
    ]));

    lines
}

pub fn generate_help_text(model: &Model) -> String {
    let lines = build_help_lines(model);
    let mut result = String::new();
    for line in lines {
        for span in line.spans {
            result.push_str(&span.content);
        }
        result.push('\n');
    }
    result
}

pub fn render_help_modal(model: &Model, frame: &mut Frame, area: Rect) {
    let theme = model.theme;

    // Dimensões do modal de ajuda: centralizado
    let modal_width = (area.width.saturating_sub(4)).clamp(20, 78);
    let modal_height = (area.height.saturating_sub(2)).clamp(10, 100);

    let popup_area = Rect {
        x: area.x + (area.width.saturating_sub(modal_width)) / 2,
        y: area.y + (area.height.saturating_sub(modal_height)) / 2,
        width: modal_width,
        height: modal_height,
    };

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Ajuda & Mapa de Atalhos [?] ")
        .border_style(Style::default().fg(theme.modal_border));

    let help_lines = build_help_lines(model);
    let inner_height = popup_area.height.saturating_sub(2) as usize;
    let max_scroll = help_lines.len().saturating_sub(inner_height);
    let scroll_y = model.help_scroll.min(max_scroll);

    let paragraph = Paragraph::new(help_lines)
        .block(block)
        .scroll((scroll_y as u16, 0));

    frame.render_widget(paragraph, popup_area);
}
