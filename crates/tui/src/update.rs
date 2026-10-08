use crate::command::Command;
use crate::message::Message;
use crate::model::{
    DeleteConfirmState, FilterField, FilterModalState, FormField, FormMode, Model, Tab,
    TransactionFilterState, TransactionFormState,
};
use app::ListTransactionsInput;
use chrono::{Local, NaiveDate};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use domain::{Money, TransactionKind, TransactionStatus, UserId};
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
            if model.active_tab == Tab::Transactions {
                Some(build_fetch_transactions_command(model))
            } else {
                None
            }
        }
        Message::PreviousTab => {
            model.active_tab = model.active_tab.previous();
            if model.active_tab == Tab::Transactions {
                Some(build_fetch_transactions_command(model))
            } else {
                None
            }
        }
        Message::SelectTab(idx) => {
            model.active_tab = Tab::from_index(idx);
            if model.active_tab == Tab::Transactions {
                Some(build_fetch_transactions_command(model))
            } else {
                None
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
        Message::ErrorOccurred(err) => {
            model.is_loading = false;
            model.error_message = Some(err.clone());
            model.status_message = format!("Erro: {err}");
            None
        }
    }
}

fn handle_key_event(model: &mut Model, key: KeyEvent) -> Option<Command> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return update(model, Message::Quit);
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
        KeyCode::Char('r') | KeyCode::Char('R') => {
            model.is_loading = true;
            model.error_message = None;
            model.status_message = "Atualizando dados...".to_string();
            if model.active_tab == Tab::Transactions {
                Some(build_fetch_transactions_command(model))
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
