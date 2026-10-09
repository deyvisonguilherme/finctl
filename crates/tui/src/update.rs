use crate::command::Command;
use crate::message::Message;
use crate::model::{
    ContributionFormField, ContributionModalState, DeleteConfirmState, FilterField,
    FilterModalState, FormField, FormMode, GoalsSubView, Model, PeriodModalState, ReportSubView,
    Tab, TransactionFilterState, TransactionFormState,
};
use crate::theme::{Theme, ThemeMode};
use app::{AddContributionInput, ListTransactionsInput};
use chrono::{Local, NaiveDate};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use domain::{ForecastGranularity, Money, TransactionKind, TransactionStatus, UserId};
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;

pub fn update(model: &mut Model, msg: Message) -> Option<Command> {
    match msg {
        Message::Quit => {
            model.running = false;
            None
        }
        Message::Key(key) => handle_key_event(model, key),
        Message::NextTab => {
            model.active_tab = model.active_tab.next();
            match model.active_tab {
                Tab::Transactions => Some(build_fetch_transactions_command(model)),
                Tab::Reports => Some(build_fetch_reports_command(model)),
                Tab::Goals => Some(build_fetch_goals_command(model)),
                _ => None,
            }
        }
        Message::PreviousTab => {
            model.active_tab = model.active_tab.previous();
            match model.active_tab {
                Tab::Transactions => Some(build_fetch_transactions_command(model)),
                Tab::Reports => Some(build_fetch_reports_command(model)),
                Tab::Goals => Some(build_fetch_goals_command(model)),
                _ => None,
            }
        }
        Message::SelectTab(idx) => {
            model.active_tab = Tab::from_index(idx);
            match model.active_tab {
                Tab::Transactions => Some(build_fetch_transactions_command(model)),
                Tab::Reports => Some(build_fetch_reports_command(model)),
                Tab::Goals => Some(build_fetch_goals_command(model)),
                _ => None,
            }
        }
        Message::Tick => {
            model.tick_count = model.tick_count.wrapping_add(1);
            model.last_tick = Local::now();
            None
        }
        Message::Resize(_, _) => None,
        Message::StatusMessage(text) => {
            model.status_message = text;
            None
        }
        Message::SetLoading(loading) => {
            model.is_loading = loading;
            if loading {
                model.error_message = None;
            }
            None
        }
        Message::DataLoaded(summary) => {
            model.is_loading = false;
            model.data_summary = Some(summary);
            model.status_message = "Dados sincronizados com sucesso".to_string();
            None
        }
        Message::DashboardLoaded(data) => {
            model.is_loading = false;
            model.error_message = None;
            model.data_summary = Some(format!(
                "Contas: {} | Saldo: R$ {:.2}",
                data.balance_report.accounts.len(),
                data.balance_report.total_balance
            ));
            model.dashboard_data = Some(*data);
            model.status_message = "Dados sincronizados com sucesso".to_string();
            None
        }
        Message::TransactionsLoaded(paginated) => {
            model.is_loading = false;
            model.error_message = None;
            let count = paginated.items.len();
            model.transactions_state.items = paginated.items;
            model.transactions_state.total_count = paginated.total_count;
            model.transactions_state.page = paginated.page;
            model.transactions_state.page_size = paginated.page_size;
            model.transactions_state.total_pages = paginated.total_pages;

            if model.transactions_state.cursor_index >= count {
                model.transactions_state.cursor_index = count.saturating_sub(1);
            }

            model.status_message = format!(
                "Lançamentos carregados (pág. {}/{}, total: {})",
                paginated.page, paginated.total_pages, paginated.total_count
            );
            None
        }
        Message::TransactionActionSuccess(msg) => {
            model.is_loading = false;
            model.error_message = None;
            model.status_message = msg;
            model.transactions_state.selected_ids.clear();
            model.transactions_state.delete_confirm = None;
            model.transactions_state.form_modal = None;
            model.transactions_state.filter_modal = None;
            // Recarregar os dados da página atual
            Some(build_fetch_transactions_command(model))
        }
        Message::ReportDataLoaded(data) => {
            model.is_loading = false;
            model.error_message = None;
            model.status_message = format!(
                "Relatórios de {} atualizados com sucesso",
                data.reference_month
            );
            model.reports_state.data = Some(*data);
            None
        }
        Message::GoalsDataLoaded(goals) => {
            model.is_loading = false;
            model.error_message = None;
            let count = goals.len();
            model.goals_state.goals = goals;
            if model.goals_state.selected_goal_index >= count {
                model.goals_state.selected_goal_index = count.saturating_sub(1);
            }
            model.status_message = format!("Metas carregadas ({} cadastradas)", count);
            Some(build_fetch_forecast_command(model))
        }
        Message::ForecastDataLoaded(forecast) => {
            model.is_loading = false;
            model.error_message = None;
            let count = forecast.periods.len();
            model.goals_state.forecast = Some(*forecast);
            if model.goals_state.forecast_cursor >= count {
                model.goals_state.forecast_cursor = count.saturating_sub(1);
            }
            model.status_message = format!("Projeção carregada ({} períodos)", count);
            None
        }
        Message::GoalContributionSuccess(msg) => {
            model.is_loading = false;
            model.error_message = None;
            model.status_message = msg;
            model.goals_state.contribution_modal = None;
            Some(build_fetch_goals_command(model))
        }
        Message::ErrorOccurred(err) => {
            model.is_loading = false;
            model.error_message = Some(err.clone());
            model.status_message = format!("Erro: {err}");
            None
        }
        Message::ToggleHelp => {
            model.is_help_open = !model.is_help_open;
            if model.is_help_open {
                model.help_scroll = 0;
            }
            None
        }
        Message::CloseHelp => {
            model.is_help_open = false;
            None
        }
        Message::ToggleTheme => {
            model.theme = model.theme.toggle();
            let name = match model.theme.mode {
                ThemeMode::Dark => "Escuro",
                ThemeMode::Light => "Claro",
            };
            model.status_message = format!("Tema visual alterado para: {name}");
            None
        }
        Message::SetTheme(mode) => {
            model.theme = Theme::from_mode(mode);
            None
        }
    }
}

fn handle_key_event(model: &mut Model, key: KeyEvent) -> Option<Command> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return update(model, Message::Quit);
    }

    // Se o painel de ajuda estiver aberto, capturar navegação ou fechamento
    if model.is_help_open {
        match key.code {
            KeyCode::Esc
            | KeyCode::Char('?')
            | KeyCode::Char('q')
            | KeyCode::Char('Q')
            | KeyCode::Enter => {
                model.is_help_open = false;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                model.help_scroll = model.help_scroll.saturating_add(1);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                model.help_scroll = model.help_scroll.saturating_sub(1);
            }
            KeyCode::PageDown => {
                model.help_scroll = model.help_scroll.saturating_add(5);
            }
            KeyCode::PageUp => {
                model.help_scroll = model.help_scroll.saturating_sub(5);
            }
            KeyCode::Home => {
                model.help_scroll = 0;
            }
            _ => {}
        }
        return None;
    }

    // Se estivermos na aba de Lançamentos e algum modal ou busca estiver ativa
    if model.active_tab == Tab::Transactions {
        if model.transactions_state.delete_confirm.is_some() {
            return handle_delete_modal_key(model, key);
        }
        if model.transactions_state.form_modal.is_some() {
            return handle_form_modal_key(model, key);
        }
        if model.transactions_state.filter_modal.is_some() {
            return handle_filter_modal_key(model, key);
        }
        if model.transactions_state.is_searching {
            return handle_search_key(model, key);
        }

        // Atalhos específicos da tabela de lançamentos
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if model.transactions_state.cursor_index > 0 {
                    model.transactions_state.cursor_index -= 1;
                }
                return None;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !model.transactions_state.items.is_empty()
                    && model.transactions_state.cursor_index
                        < model.transactions_state.items.len() - 1
                {
                    model.transactions_state.cursor_index += 1;
                }
                return None;
            }
            KeyCode::Char(' ') => {
                // Alternar seleção múltipla do item sob o cursor
                if let Some(item) = model
                    .transactions_state
                    .items
                    .get(model.transactions_state.cursor_index)
                {
                    if model.transactions_state.selected_ids.contains(&item.id) {
                        model.transactions_state.selected_ids.remove(&item.id);
                    } else {
                        model.transactions_state.selected_ids.insert(item.id);
                    }
                }
                return None;
            }
            KeyCode::Char('[') | KeyCode::PageUp => {
                // Página anterior
                if model.transactions_state.page > 1 {
                    model.transactions_state.page -= 1;
                    model.transactions_state.cursor_index = 0;
                    return Some(build_fetch_transactions_command(model));
                }
                return None;
            }
            KeyCode::Char(']') | KeyCode::PageDown => {
                // Próxima página
                if model.transactions_state.page < model.transactions_state.total_pages {
                    model.transactions_state.page += 1;
                    model.transactions_state.cursor_index = 0;
                    return Some(build_fetch_transactions_command(model));
                }
                return None;
            }
            KeyCode::Char('p') | KeyCode::Char('P') => {
                // Marcar como pago
                let ids: Vec<_> = if !model.transactions_state.selected_ids.is_empty() {
                    model
                        .transactions_state
                        .selected_ids
                        .iter()
                        .copied()
                        .collect()
                } else if let Some(item) = model
                    .transactions_state
                    .items
                    .get(model.transactions_state.cursor_index)
                {
                    vec![item.id]
                } else {
                    vec![]
                };

                if !ids.is_empty() {
                    model.is_loading = true;
                    model.status_message =
                        format!("Marcando {} lançamento(s) como pago...", ids.len());
                    return Some(Command::PayTransactions(ids, None));
                }
                return None;
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                // Abrir modal de criação
                let today = Local::now().date_naive();
                model.transactions_state.form_modal = Some(TransactionFormState {
                    mode: FormMode::Add,
                    editing_id: None,
                    focused_field: FormField::Kind,
                    kind: TransactionKind::Expense,
                    account_input: String::new(),
                    category_input: String::new(),
                    amount_input: String::new(),
                    date_input: today.format("%Y-%m-%d").to_string(),
                    description_input: String::new(),
                    status: TransactionStatus::Paid,
                    validation_error: None,
                });
                return None;
            }
            KeyCode::Enter | KeyCode::Char('e') | KeyCode::Char('E') => {
                // Abrir modal de edição para o item sob o cursor
                if let Some(item) = model
                    .transactions_state
                    .items
                    .get(model.transactions_state.cursor_index)
                {
                    model.transactions_state.form_modal = Some(TransactionFormState {
                        mode: FormMode::Edit,
                        editing_id: Some(item.id),
                        focused_field: FormField::Amount,
                        kind: item.kind,
                        account_input: item.account_name.clone(),
                        category_input: item.category_name.clone(),
                        amount_input: format!("{:.2}", item.amount.as_decimal()),
                        date_input: item.date.format("%Y-%m-%d").to_string(),
                        description_input: item.description.clone(),
                        status: item.status,
                        validation_error: None,
                    });
                }
                return None;
            }
            KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Delete => {
                // Abrir diálogo de confirmação de exclusão
                let (ids, desc) = if !model.transactions_state.selected_ids.is_empty() {
                    let ids: Vec<_> = model
                        .transactions_state
                        .selected_ids
                        .iter()
                        .copied()
                        .collect();
                    let prompt = format!(
                        "Deseja realmente remover os {} lançamentos selecionados?",
                        ids.len()
                    );
                    (ids, prompt)
                } else if let Some(item) = model
                    .transactions_state
                    .items
                    .get(model.transactions_state.cursor_index)
                {
                    let prompt = format!(
                        "Deseja remover '{}' de R$ {:.2}?",
                        item.description,
                        item.amount.as_decimal()
                    );
                    (vec![item.id], prompt)
                } else {
                    (vec![], String::new())
                };

                if !ids.is_empty() {
                    model.transactions_state.delete_confirm = Some(DeleteConfirmState {
                        target_ids: ids,
                        prompt_message: desc,
                    });
                }
                return None;
            }
            KeyCode::Char('/') => {
                // Ativar campo de busca por descrição
                model.transactions_state.is_searching = true;
                return None;
            }
            KeyCode::Char('f') | KeyCode::Char('F') => {
                // Abrir modal de filtros
                let active = &model.transactions_state.active_filters;
                model.transactions_state.filter_modal = Some(FilterModalState {
                    focused_field: FilterField::Month,
                    month_input: active.month.clone().unwrap_or_default(),
                    account_input: active.account.clone().unwrap_or_default(),
                    category_input: active.category.clone().unwrap_or_default(),
                    kind_selection: active.kind,
                    status_selection: active.status,
                    tag_input: active.tag.clone().unwrap_or_default(),
                });
                return None;
            }
            _ => {}
        }
    }

    // Se estivermos na aba de Relatórios
    if model.active_tab == Tab::Reports {
        if model.reports_state.period_modal.is_some() {
            return handle_period_modal_key(model, key);
        }

        match key.code {
            KeyCode::Char('1') => {
                model.reports_state.active_subview = ReportSubView::Categories;
                return None;
            }
            KeyCode::Char('2') => {
                model.reports_state.active_subview = ReportSubView::MonthlyEvolution;
                return None;
            }
            KeyCode::Char('3') => {
                model.reports_state.active_subview = ReportSubView::Comparison;
                return None;
            }
            KeyCode::Char('v') | KeyCode::Char('V') => {
                model.reports_state.active_subview = match model.reports_state.active_subview {
                    ReportSubView::Categories => ReportSubView::MonthlyEvolution,
                    ReportSubView::MonthlyEvolution => ReportSubView::Comparison,
                    ReportSubView::Comparison => ReportSubView::Categories,
                };
                return None;
            }
            KeyCode::Char('[') | KeyCode::PageUp => {
                if let Some(prev) = adjust_month_string(&model.reports_state.reference_month, -1) {
                    model.reports_state.reference_month = prev;
                    model.reports_state.category_cursor = 0;
                    model.reports_state.comparison_cursor = 0;
                    model.is_loading = true;
                    return Some(build_fetch_reports_command(model));
                }
                return None;
            }
            KeyCode::Char(']') | KeyCode::PageDown => {
                if let Some(next) = adjust_month_string(&model.reports_state.reference_month, 1) {
                    model.reports_state.reference_month = next;
                    model.reports_state.category_cursor = 0;
                    model.reports_state.comparison_cursor = 0;
                    model.is_loading = true;
                    return Some(build_fetch_reports_command(model));
                }
                return None;
            }
            KeyCode::Char('i') | KeyCode::Char('I') => {
                model.reports_state.include_pending = !model.reports_state.include_pending;
                model.is_loading = true;
                return Some(build_fetch_reports_command(model));
            }
            KeyCode::Char('p') | KeyCode::Char('P') => {
                model.reports_state.period_modal = Some(PeriodModalState {
                    input_month: model.reports_state.reference_month.clone(),
                    validation_error: None,
                });
                return None;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                match model.reports_state.active_subview {
                    ReportSubView::Categories if model.reports_state.category_cursor > 0 => {
                        model.reports_state.category_cursor -= 1;
                    }
                    ReportSubView::Comparison if model.reports_state.comparison_cursor > 0 => {
                        model.reports_state.comparison_cursor -= 1;
                    }
                    _ => {}
                }
                return None;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                match model.reports_state.active_subview {
                    ReportSubView::Categories => {
                        let total_items = model
                            .reports_state
                            .data
                            .as_ref()
                            .map(|d| d.category_report.items.len())
                            .unwrap_or(0);
                        if total_items > 0 && model.reports_state.category_cursor < total_items - 1
                        {
                            model.reports_state.category_cursor += 1;
                        }
                    }
                    ReportSubView::Comparison => {
                        let total_items = model
                            .reports_state
                            .data
                            .as_ref()
                            .map(|d| d.comparison_categories.rows.len())
                            .unwrap_or(0);
                        if total_items > 0
                            && model.reports_state.comparison_cursor < total_items - 1
                        {
                            model.reports_state.comparison_cursor += 1;
                        }
                    }
                    _ => {}
                }
                return None;
            }
            _ => {}
        }
    }

    // Se estivermos na aba de Metas e Projeção
    if model.active_tab == Tab::Goals {
        if model.goals_state.contribution_modal.is_some() {
            return handle_contribution_modal_key(model, key);
        }

        match key.code {
            KeyCode::Char('1') => {
                model.goals_state.active_subview = GoalsSubView::Goals;
                return None;
            }
            KeyCode::Char('2') => {
                model.goals_state.active_subview = GoalsSubView::Forecast;
                return None;
            }
            KeyCode::Char('v') | KeyCode::Char('V') => {
                model.goals_state.active_subview = match model.goals_state.active_subview {
                    GoalsSubView::Goals => GoalsSubView::Forecast,
                    GoalsSubView::Forecast => GoalsSubView::Goals,
                };
                return None;
            }
            _ => {}
        }

        match model.goals_state.active_subview {
            GoalsSubView::Goals => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if model.goals_state.selected_goal_index > 0 {
                        model.goals_state.selected_goal_index -= 1;
                    }
                    return None;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let total = model.goals_state.goals.len();
                    if total > 0 && model.goals_state.selected_goal_index < total - 1 {
                        model.goals_state.selected_goal_index += 1;
                    }
                    return None;
                }
                KeyCode::Char('c')
                | KeyCode::Char('C')
                | KeyCode::Char('a')
                | KeyCode::Char('A') => {
                    if let Some(progress) = model
                        .goals_state
                        .goals
                        .get(model.goals_state.selected_goal_index)
                    {
                        if progress.goal.is_account_linked() {
                            model.status_message = "Metas vinculadas a conta acompanham o saldo bancário. Aportes manuais não se aplicam.".to_string();
                        } else if progress.is_completed {
                            model.status_message =
                                "Meta já concluída. Reabra via CLI se desejar adicionar aportes."
                                    .to_string();
                        } else {
                            let today_str =
                                Local::now().date_naive().format("%Y-%m-%d").to_string();
                            model.goals_state.contribution_modal = Some(ContributionModalState {
                                goal_id: progress.goal.id,
                                goal_name: progress.goal.name.clone(),
                                amount_input: String::new(),
                                date_input: today_str,
                                note_input: String::new(),
                                focused_field: ContributionFormField::Amount,
                                validation_error: None,
                            });
                        }
                    } else {
                        model.status_message = "Nenhuma meta selecionada para aporte.".to_string();
                    }
                    return None;
                }
                _ => {}
            },
            GoalsSubView::Forecast => match key.code {
                KeyCode::Char('g') | KeyCode::Char('G') => {
                    model.goals_state.include_goals = !model.goals_state.include_goals;
                    model.is_loading = true;
                    model.status_message = if model.goals_state.include_goals {
                        "Simulando projeção COM saídas de metas ativas".to_string()
                    } else {
                        "Simulando projeção SEM saídas de metas".to_string()
                    };
                    return Some(build_fetch_forecast_command(model));
                }
                KeyCode::Char('w') | KeyCode::Char('W') => {
                    model.goals_state.forecast_granularity = ForecastGranularity::Week;
                    model.is_loading = true;
                    return Some(build_fetch_forecast_command(model));
                }
                KeyCode::Char('m') | KeyCode::Char('M') => {
                    model.goals_state.forecast_granularity = ForecastGranularity::Month;
                    model.is_loading = true;
                    return Some(build_fetch_forecast_command(model));
                }
                KeyCode::Char('+') | KeyCode::Char('=') => {
                    if model.goals_state.forecast_months < 24 {
                        model.goals_state.forecast_months =
                            (model.goals_state.forecast_months + 3).min(24);
                        model.is_loading = true;
                        return Some(build_fetch_forecast_command(model));
                    }
                    return None;
                }
                KeyCode::Char('-') | KeyCode::Char('_') => {
                    if model.goals_state.forecast_months > 3 {
                        model.goals_state.forecast_months =
                            (model.goals_state.forecast_months - 3).max(3);
                        model.is_loading = true;
                        return Some(build_fetch_forecast_command(model));
                    }
                    return None;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if model.goals_state.forecast_cursor > 0 {
                        model.goals_state.forecast_cursor -= 1;
                    }
                    return None;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let total = model
                        .goals_state
                        .forecast
                        .as_ref()
                        .map(|f| f.periods.len())
                        .unwrap_or(0);
                    if total > 0 && model.goals_state.forecast_cursor < total - 1 {
                        model.goals_state.forecast_cursor += 1;
                    }
                    return None;
                }
                _ => {}
            },
        }
    }

    // Teclas globais de navegação e atalhos
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => update(model, Message::Quit),
        KeyCode::Tab => update(model, Message::NextTab),
        KeyCode::BackTab => update(model, Message::PreviousTab),
        KeyCode::Char('1') => update(model, Message::SelectTab(0)),
        KeyCode::Char('2') => update(model, Message::SelectTab(1)),
        KeyCode::Char('3') => update(model, Message::SelectTab(2)),
        KeyCode::Char('4') => update(model, Message::SelectTab(3)),
        KeyCode::Char('5') => update(model, Message::SelectTab(4)),
        KeyCode::Char('?') => {
            model.is_help_open = true;
            model.help_scroll = 0;
            None
        }
        KeyCode::Char('t') | KeyCode::Char('T') => update(model, Message::ToggleTheme),
        KeyCode::Char('r') | KeyCode::Char('R') => {
            model.is_loading = true;
            model.error_message = None;
            model.status_message = "Atualizando dados...".to_string();
            if model.active_tab == Tab::Transactions {
                Some(build_fetch_transactions_command(model))
            } else if model.active_tab == Tab::Reports {
                Some(build_fetch_reports_command(model))
            } else if model.active_tab == Tab::Goals {
                Some(build_fetch_goals_command(model))
            } else {
                Some(Command::RefreshData)
            }
        }
        _ => None,
    }
}

fn handle_delete_modal_key(model: &mut Model, key: KeyEvent) -> Option<Command> {
    match key.code {
        KeyCode::Char('s')
        | KeyCode::Char('S')
        | KeyCode::Char('y')
        | KeyCode::Char('Y')
        | KeyCode::Enter => {
            if let Some(confirm) = model.transactions_state.delete_confirm.take() {
                model.is_loading = true;
                model.status_message =
                    format!("Removendo {} lançamento(s)...", confirm.target_ids.len());
                return Some(Command::DeleteTransactions(confirm.target_ids));
            }
            None
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            model.transactions_state.delete_confirm = None;
            None
        }
        _ => None,
    }
}

fn handle_search_key(model: &mut Model, key: KeyEvent) -> Option<Command> {
    match key.code {
        KeyCode::Esc => {
            model.transactions_state.is_searching = false;
            None
        }
        KeyCode::Enter => {
            model.transactions_state.is_searching = false;
            model.transactions_state.page = 1;
            model.transactions_state.cursor_index = 0;
            Some(build_fetch_transactions_command(model))
        }
        KeyCode::Backspace => {
            model.transactions_state.search_query.pop();
            None
        }
        KeyCode::Char(c) => {
            model.transactions_state.search_query.push(c);
            None
        }
        _ => None,
    }
}

fn handle_form_modal_key(model: &mut Model, key: KeyEvent) -> Option<Command> {
    let form = model.transactions_state.form_modal.as_mut()?;

    match key.code {
        KeyCode::Esc => {
            model.transactions_state.form_modal = None;
            None
        }
        KeyCode::Tab => {
            form.focused_field = form.focused_field.next();
            None
        }
        KeyCode::BackTab => {
            form.focused_field = form.focused_field.previous();
            None
        }
        KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
            if form.focused_field == FormField::Kind =>
        {
            form.kind = match form.kind {
                TransactionKind::Expense => TransactionKind::Income,
                TransactionKind::Income => TransactionKind::Expense,
            };
            None
        }
        KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
            if form.focused_field == FormField::Status =>
        {
            form.status = match form.status {
                TransactionStatus::Paid => TransactionStatus::Pending,
                TransactionStatus::Pending => TransactionStatus::Paid,
            };
            None
        }
        KeyCode::Enter => {
            // Validar e submeter
            form.validation_error = None;

            if form.account_input.trim().is_empty() {
                form.validation_error = Some("Conta não pode ser vazia.".to_string());
                return None;
            }
            if form.category_input.trim().is_empty() {
                form.validation_error = Some("Categoria não pode ser vazia.".to_string());
                return None;
            }

            let amount_str = form.amount_input.trim().replace(',', ".");
            let amount_dec = match Decimal::from_str(&amount_str) {
                Ok(d) if d > Decimal::ZERO => d,
                _ => {
                    form.validation_error =
                        Some("Valor deve ser um número positivo maior que zero.".to_string());
                    return None;
                }
            };
            let amount_money = match Money::from_decimal_non_negative(amount_dec) {
                Ok(m) => m,
                Err(e) => {
                    form.validation_error = Some(format!("Valor inválido: {e}"));
                    return None;
                }
            };

            let date_parsed = match NaiveDate::parse_from_str(form.date_input.trim(), "%Y-%m-%d") {
                Ok(d) => d,
                Err(_) => {
                    form.validation_error =
                        Some("Data deve estar no formato AAAA-MM-DD (ex: 2026-10-15).".to_string());
                    return None;
                }
            };

            if form.description_input.trim().is_empty() {
                form.validation_error = Some("Descrição não pode ser vazia.".to_string());
                return None;
            }

            model.is_loading = true;

            match form.mode {
                FormMode::Add => {
                    let cmd = Command::CreateTransaction(app::CreateTransactionInput {
                        user_id: UserId::new(Uuid::nil()),
                        account_query: form.account_input.trim().to_string(),
                        category_query: form.category_input.trim().to_string(),
                        kind: form.kind,
                        amount: amount_money,
                        date: date_parsed,
                        description: form.description_input.trim().to_string(),
                        status: Some(form.status),
                    });
                    model.status_message = "Salvando novo lançamento...".to_string();
                    Some(cmd)
                }
                FormMode::Edit => {
                    let id = form
                        .editing_id
                        .unwrap_or_else(domain::TransactionId::generate);
                    let cmd = Command::EditTransaction(app::EditTransactionInput {
                        user_id: UserId::new(Uuid::nil()),
                        id,
                        account_query: Some(form.account_input.trim().to_string()),
                        category_query: Some(form.category_input.trim().to_string()),
                        amount: Some(amount_money),
                        date: Some(date_parsed),
                        description: Some(form.description_input.trim().to_string()),
                    });
                    model.status_message = "Salvando alterações do lançamento...".to_string();
                    Some(cmd)
                }
            }
        }
        KeyCode::Backspace => {
            match form.focused_field {
                FormField::Account => {
                    form.account_input.pop();
                }
                FormField::Category => {
                    form.category_input.pop();
                }
                FormField::Amount => {
                    form.amount_input.pop();
                }
                FormField::Date => {
                    form.date_input.pop();
                }
                FormField::Description => {
                    form.description_input.pop();
                }
                _ => {}
            }
            None
        }
        KeyCode::Char(c) => {
            match form.focused_field {
                FormField::Account => form.account_input.push(c),
                FormField::Category => form.category_input.push(c),
                FormField::Amount => form.amount_input.push(c),
                FormField::Date => form.date_input.push(c),
                FormField::Description => form.description_input.push(c),
                _ => {}
            }
            None
        }
        _ => None,
    }
}

fn handle_filter_modal_key(model: &mut Model, key: KeyEvent) -> Option<Command> {
    let filter = model.transactions_state.filter_modal.as_mut()?;

    match key.code {
        KeyCode::Esc => {
            model.transactions_state.filter_modal = None;
            None
        }
        KeyCode::Tab => {
            filter.focused_field = filter.focused_field.next();
            None
        }
        KeyCode::BackTab => {
            filter.focused_field = filter.focused_field.previous();
            None
        }
        KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
            if filter.focused_field == FilterField::Kind =>
        {
            filter.kind_selection = match filter.kind_selection {
                None => Some(TransactionKind::Expense),
                Some(TransactionKind::Expense) => Some(TransactionKind::Income),
                Some(TransactionKind::Income) => None,
            };
            None
        }
        KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
            if filter.focused_field == FilterField::Status =>
        {
            filter.status_selection = match filter.status_selection {
                None => Some(TransactionStatus::Paid),
                Some(TransactionStatus::Paid) => Some(TransactionStatus::Pending),
                Some(TransactionStatus::Pending) => None,
            };
            None
        }
        KeyCode::Enter => {
            // Aplicar filtros
            model.transactions_state.active_filters = TransactionFilterState {
                month: if filter.month_input.trim().is_empty() {
                    None
                } else {
                    Some(filter.month_input.trim().to_string())
                },
                account: if filter.account_input.trim().is_empty() {
                    None
                } else {
                    Some(filter.account_input.trim().to_string())
                },
                category: if filter.category_input.trim().is_empty() {
                    None
                } else {
                    Some(filter.category_input.trim().to_string())
                },
                kind: filter.kind_selection,
                status: filter.status_selection,
                tag: if filter.tag_input.trim().is_empty() {
                    None
                } else {
                    Some(filter.tag_input.trim().to_string())
                },
            };
            model.transactions_state.filter_modal = None;
            model.transactions_state.page = 1;
            model.transactions_state.cursor_index = 0;
            Some(build_fetch_transactions_command(model))
        }
        KeyCode::Backspace => {
            match filter.focused_field {
                FilterField::Month => {
                    filter.month_input.pop();
                }
                FilterField::Account => {
                    filter.account_input.pop();
                }
                FilterField::Category => {
                    filter.category_input.pop();
                }
                FilterField::Tag => {
                    filter.tag_input.pop();
                }
                _ => {}
            }
            None
        }
        KeyCode::Char(c) => {
            match filter.focused_field {
                FilterField::Month => filter.month_input.push(c),
                FilterField::Account => filter.account_input.push(c),
                FilterField::Category => filter.category_input.push(c),
                FilterField::Tag => filter.tag_input.push(c),
                _ => {}
            }
            None
        }
        _ => None,
    }
}

pub fn build_fetch_transactions_command(model: &Model) -> Command {
    let state = &model.transactions_state;
    let offset = (state.page - 1) * state.page_size;
    let input = ListTransactionsInput {
        user_id: UserId::new(Uuid::nil()),
        month: state.active_filters.month.clone(),
        account_query: state.active_filters.account.clone(),
        category_query: state.active_filters.category.clone(),
        kind: state.active_filters.kind,
        status: state.active_filters.status,
        tag: state.active_filters.tag.clone(),
        limit: Some(state.page_size),
        offset: Some(offset),
        search_description: if state.search_query.trim().is_empty() {
            None
        } else {
            Some(state.search_query.trim().to_string())
        },
        deleted: Some(false),
        ..Default::default()
    };
    Command::FetchTransactions(input)
}

fn handle_period_modal_key(model: &mut Model, key: KeyEvent) -> Option<Command> {
    let modal = model.reports_state.period_modal.as_mut()?;
    match key.code {
        KeyCode::Esc => {
            model.reports_state.period_modal = None;
            None
        }
        KeyCode::Enter => {
            let input = modal.input_month.trim().to_string();
            match app::parse_month_bounds(&input) {
                Ok(_) => {
                    model.reports_state.reference_month = input;
                    model.reports_state.period_modal = None;
                    model.reports_state.category_cursor = 0;
                    model.reports_state.comparison_cursor = 0;
                    model.is_loading = true;
                    Some(build_fetch_reports_command(model))
                }
                Err(_) => {
                    modal.validation_error =
                        Some("Formato inválido. Use AAAA-MM (ex: 2026-10)".to_string());
                    None
                }
            }
        }
        KeyCode::Backspace => {
            modal.input_month.pop();
            None
        }
        KeyCode::Char(c) => {
            if modal.input_month.len() < 7 {
                modal.input_month.push(c);
            }
            None
        }
        _ => None,
    }
}

pub fn build_fetch_reports_command(model: &Model) -> Command {
    Command::FetchReportData {
        month: model.reports_state.reference_month.clone(),
        include_pending: model.reports_state.include_pending,
    }
}

pub fn adjust_month_string(month_str: &str, delta: i32) -> Option<String> {
    let parts: Vec<&str> = month_str.split('-').collect();
    if parts.len() != 2 {
        return None;
    }
    let year: i32 = parts[0].parse().ok()?;
    let month: u32 = parts[1].parse().ok()?;
    let total_m = year * 12 + (month as i32 - 1) + delta;
    let new_year = total_m / 12;
    let new_month = (total_m % 12 + 1) as u32;
    Some(format!("{:04}-{:02}", new_year, new_month))
}

fn handle_contribution_modal_key(model: &mut Model, key: KeyEvent) -> Option<Command> {
    let modal = model.goals_state.contribution_modal.as_mut()?;
    match key.code {
        KeyCode::Esc => {
            model.goals_state.contribution_modal = None;
            None
        }
        KeyCode::Tab | KeyCode::Down => {
            modal.focused_field = modal.focused_field.next();
            None
        }
        KeyCode::BackTab | KeyCode::Up => {
            modal.focused_field = modal.focused_field.previous();
            None
        }
        KeyCode::Enter => {
            let amount_str = modal.amount_input.trim().replace(',', ".");
            let amount = match Money::parse(&amount_str) {
                Ok(a) if a.as_decimal() > Decimal::ZERO => a,
                _ => {
                    modal.validation_error = Some(
                        "Valor de aporte inválido ou menor/igual a zero (ex: 250.00)".to_string(),
                    );
                    return None;
                }
            };

            let date = match NaiveDate::parse_from_str(modal.date_input.trim(), "%Y-%m-%d") {
                Ok(d) => d,
                Err(_) => {
                    modal.validation_error = Some(
                        "Data inválida. Utilize o formato AAAA-MM-DD (ex: 2026-10-15)".to_string(),
                    );
                    return None;
                }
            };

            let note = if modal.note_input.trim().is_empty() {
                None
            } else {
                Some(modal.note_input.trim().to_string())
            };

            let input = AddContributionInput {
                user_id: UserId::new(Uuid::nil()),
                goal_identifier: modal.goal_id.to_string(),
                amount,
                date,
                note,
            };

            model.goals_state.contribution_modal = None;
            model.is_loading = true;
            model.status_message = "Registrando aporte na meta...".to_string();
            Some(Command::AddGoalContribution(input))
        }
        KeyCode::Backspace => {
            modal.validation_error = None;
            match modal.focused_field {
                ContributionFormField::Amount => {
                    modal.amount_input.pop();
                }
                ContributionFormField::Date => {
                    modal.date_input.pop();
                }
                ContributionFormField::Note => {
                    modal.note_input.pop();
                }
            }
            None
        }
        KeyCode::Char(c) => {
            modal.validation_error = None;
            match modal.focused_field {
                ContributionFormField::Amount => {
                    if c.is_ascii_digit() || c == '.' || c == ',' {
                        modal.amount_input.push(c);
                    }
                }
                ContributionFormField::Date => {
                    if c.is_ascii_digit() || c == '-' {
                        modal.date_input.push(c);
                    }
                }
                ContributionFormField::Note => {
                    modal.note_input.push(c);
                }
            }
            None
        }
        _ => None,
    }
}

pub fn build_fetch_goals_command(_model: &Model) -> Command {
    Command::FetchGoalsData
}

pub fn build_fetch_forecast_command(model: &Model) -> Command {
    Command::FetchForecastData {
        months: model.goals_state.forecast_months,
        granularity: model.goals_state.forecast_granularity,
        include_goals: model.goals_state.include_goals,
    }
}
