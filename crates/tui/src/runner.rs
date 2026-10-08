use crate::command::Command;
use crate::message::Message;
use crate::model::Model;
use crate::update::update;
use crate::view::view;
use crossterm::event::{Event, EventStream};
use futures_util::StreamExt;
use ratatui::backend::Backend;
use ratatui::Terminal;
use std::io;
use std::time::Duration;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::time::interval;

pub async fn run_tui<B: Backend>(
    terminal: &mut Terminal<B>,
    mut model: Model,
    cmd_tx: Option<Sender<Command>>,
    mut msg_rx: Option<Receiver<Message>>,
) -> io::Result<Model> {
    let mut event_stream = EventStream::new();
    let mut ticker = interval(Duration::from_millis(250));

    // Desenhar estado inicial imediatamente
    terminal.draw(|f| view(&model, f))?;

    // Disparar comando inicial se houver canal configurado
    if let Some(ref tx) = cmd_tx {
        let _ = tx.send(Command::FetchInitialData).await;
    }

    while model.running {
        tokio::select! {
            // 1. Eventos do terminal (teclas, redimensionamento)
            maybe_event = event_stream.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) => {
                        let cmd = update(&mut model, Message::Key(key));
                        if let Some(c) = cmd {
                            if let Some(ref tx) = cmd_tx {
                                let _ = tx.send(c).await;
                            }
                        }
                    }
                    Some(Ok(Event::Resize(w, h))) => {
                        update(&mut model, Message::Resize(w, h));
                    }
                    Some(Err(err)) => {
                        tracing::error!("Erro no fluxo de eventos do terminal: {}", err);
                    }
                    _ => {}
                }
            }

            // 2. Mensagens do backend assíncrono
            Some(msg) = async {
                if let Some(ref mut rx) = msg_rx {
                    rx.recv().await
                } else {
                    futures_util::future::pending().await
                }
            } => {
                let cmd = update(&mut model, msg);
                if let Some(c) = cmd {
                    if let Some(ref tx) = cmd_tx {
                        let _ = tx.send(c).await;
                    }
                }
            }

            // 3. Ticker periódico (relógio / animação)
            _ = ticker.tick() => {
                update(&mut model, Message::Tick);
            }
        }

        // Redesenhar a interface após processamento
        terminal.draw(|f| view(&model, f))?;
    }

    Ok(model)
}
