use crate::model::{
    DeleteConfirmState, FilterField, FilterModalState, FormField, FormMode, Model, Tab,
    TransactionFormState,
};
use app::{DashboardData, UpcomingKind};
use domain::{format_decimal_pt_br, BudgetIndicator, TransactionKind, TransactionStatus};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Tabs},
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
}

fn render_header(model: &Model, frame: &mut Frame, area: Rect) {
    let clock_str = model.last_tick.format("%d/%m/%Y %H:%M:%S").to_string();
    let status_indicator = if model.error_message.is_some() {
        Span::styled(
            " [Erro de Conexão]",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )
    } else if model.is_loading {
        Span::styled(" [Sincronizando...]", Style::default().fg(Color::Yellow))
    } else {
        Span::styled(" [Online]", Style::default().fg(Color::Green))
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
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(title_desc),
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
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Saldos por Conta ")
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

fn render_reports_tab(_model: &Model, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" 3: Relatórios e Gráficos ")
        .border_style(Style::default().fg(Color::Green));

    let text = vec![
        Line::from(Span::styled(
            "Gráficos e Indicadores Financeiros (F6-04)",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Visualização de gastos por categoria e evolução mensal na etapa F6-04."),
        Line::from("Navegue de volta para o Dashboard usando a tecla '1' ou Tab."),
    ];

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
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

fn render_goals_tab(_model: &Model, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" 5: Metas de Economia ")
        .border_style(Style::default().fg(Color::Blue));

    let text = vec![
        Line::from(Span::styled(
            "Metas de Economia e Projeção de Fluxo de Caixa (F6-06 a F6-08)",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(
            "O acompanhamento de metas com aportes e projeções futuras será implementado em F6-08.",
        ),
        Line::from("Navegue de volta para o Dashboard usando a tecla '1' ou Tab."),
    ];

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn render_footer(model: &Model, frame: &mut Frame, area: Rect) {
    let shortcuts = Line::from(vec![
        Span::styled(" [q/Ctrl+C] ", Style::default().fg(Color::Yellow)),
        Span::raw("Sair  "),
        Span::styled(" [Tab/1-5] ", Style::default().fg(Color::Yellow)),
        Span::raw("Mudar Aba  "),
        Span::styled(" [r] ", Style::default().fg(Color::Yellow)),
        Span::raw("Atualizar  "),
    ]);

    let status = if let Some(ref err) = model.error_message {
        Line::from(vec![
            Span::styled(
                "ERRO: ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(err, Style::default().fg(Color::Red)),
            Span::raw(" "),
        ])
    } else {
        Line::from(vec![
            Span::raw("Status: "),
            Span::styled(&model.status_message, Style::default().fg(Color::White)),
            Span::raw(" "),
        ])
    };

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
