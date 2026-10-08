use crate::command::Command;
use crate::message::Message;
use crate::model::{Model, Tab};
use chrono::Local;
use crossterm::event::{KeyCode, KeyModifiers};

pub fn update(model: &mut Model, msg: Message) -> Option<Command> {
    match msg {
        Message::Quit => {
            model.running = false;
            None
        }
        Message::Key(key) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                return update(model, Message::Quit);
            }

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
                    Some(Command::RefreshData)
                }
                _ => None,
            }
        }
        Message::NextTab => {
            model.active_tab = model.active_tab.next();
            None
        }
        Message::PreviousTab => {
            model.active_tab = model.active_tab.previous();
            None
        }
        Message::SelectTab(idx) => {
            model.active_tab = Tab::from_index(idx);
            None
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
        Message::ErrorOccurred(err) => {
            model.is_loading = false;
            model.error_message = Some(err.clone());
            model.status_message = format!("Erro: {err}");
            None
        }
    }
}
