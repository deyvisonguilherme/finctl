use crate::cli::ForecastArgs;
use crate::format::OutputFormat;
use app::{ForecastInput, ForecastService};
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, CellAlignment, Color, Table};
use domain::{format_decimal_pt_br, ForecastGranularity, UserId};
use rust_decimal::Decimal;
use sqlx::PgPool;

pub async fn handle_forecast_command(
    pool: &PgPool,
    user_id: UserId,
    args: ForecastArgs,
) -> Result<(), (String, u8)> {
    let granularity: ForecastGranularity = args
        .granularity
        .parse()
        .map_err(|e: domain::DomainError| (format!("{e}"), 1))?;

    let service = ForecastService::new(pool);
    let forecast = service
        .generate_forecast(ForecastInput {
            user_id,
            months: Some(args.months),
            account_query: args.account,
            granularity: Some(granularity),
            as_of_date: None,
        })
        .await
        .map_err(|e| match e {
            app::AppError::NotFound(n) => (n, 1),
            app::AppError::Validation(v) => (v, 1),
            app::AppError::Domain(d) => (format!("{d}"), 1),
            other => (format!("{other}"), 2),
        })?;

    match args.format {
        OutputFormat::Table => {
            let scope_desc = match &forecast.account_name {
                Some(name) => format!("Conta: {name}"),
                None => "Consolidado Geral (Regime de Caixa)".to_string(),
            };

            let gran_desc = match forecast.granularity {
                ForecastGranularity::Month => "Mensal",
                ForecastGranularity::Week => "Semanal",
            };

            println!("PROJEÇÃO DE FLUXO DE CAIXA");
            println!("Escopo: {}", scope_desc);
            println!(
                "Saldo Inicial Atual: R$ {}",
                format_decimal_pt_br(forecast.initial_balance)
            );
            println!(
                "Horizonte: {} meses (Granularidade: {})",
                forecast.months, gran_desc
            );
            println!();

            let mut table = Table::new();
            table
                .load_preset(UTF8_FULL)
                .apply_modifier(UTF8_ROUND_CORNERS)
                .set_header(vec![
                    "Período",
                    "Saldo Inicial",
                    "Entradas (+)",
                    "Saídas (-)",
                    "Resultado Líquido",
                    "Saldo Projetado",
                ]);

            let mut first_neg_found = false;

            for p in &forecast.periods {
                let is_first_neg = !first_neg_found && p.is_negative;
                if is_first_neg {
                    first_neg_found = true;
                }

                let closing_cell = if is_first_neg {
                    Cell::new(format!(
                        "⚠️  R$ {}",
                        format_decimal_pt_br(p.closing_balance)
                    ))
                    .set_alignment(CellAlignment::Right)
                    .fg(Color::Red)
                } else if p.is_negative {
                    Cell::new(format!("R$ {}", format_decimal_pt_br(p.closing_balance)))
                        .set_alignment(CellAlignment::Right)
                        .fg(Color::Red)
                } else {
                    Cell::new(format!("R$ {}", format_decimal_pt_br(p.closing_balance)))
                        .set_alignment(CellAlignment::Right)
                        .fg(Color::Green)
                };

                let net_cell = if p.net_change < Decimal::ZERO {
                    Cell::new(format!("R$ {}", format_decimal_pt_br(p.net_change)))
                        .set_alignment(CellAlignment::Right)
                        .fg(Color::Red)
                } else {
                    Cell::new(format!("+R$ {}", format_decimal_pt_br(p.net_change)))
                        .set_alignment(CellAlignment::Right)
                };

                table.add_row(vec![
                    Cell::new(&p.period_label).set_alignment(CellAlignment::Center),
                    Cell::new(format!("R$ {}", format_decimal_pt_br(p.opening_balance)))
                        .set_alignment(CellAlignment::Right),
                    Cell::new(format!("R$ {}", format_decimal_pt_br(p.total_income)))
                        .set_alignment(CellAlignment::Right),
                    Cell::new(format!("R$ {}", format_decimal_pt_br(p.total_expense)))
                        .set_alignment(CellAlignment::Right),
                    net_cell,
                    closing_cell,
                ]);
            }

            println!("{table}");

            if let Some(ref neg_period) = forecast.first_negative_period {
                println!(
                    "\n⚠️  Alerta: O saldo projetado fica negativo pela primeira vez no período {} (Saldo mínimo: R$ {}).",
                    neg_period,
                    format_decimal_pt_br(forecast.lowest_projected_balance)
                );
            } else {
                println!(
                    "\n✓ Saldo projetado permanece positivo durante todo o horizonte analisado."
                );
            }
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&forecast)
                .map_err(|e| (format!("Erro ao gerar JSON: {e}"), 1))?;
            println!("{json}");
        }
        OutputFormat::Csv => {
            let mut wtr = csv::Writer::from_writer(std::io::stdout());
            wtr.write_record([
                "period",
                "start_date",
                "end_date",
                "opening_balance",
                "income",
                "expense",
                "net_change",
                "closing_balance",
                "is_negative",
            ])
            .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;

            for p in &forecast.periods {
                wtr.write_record([
                    p.period_label.clone(),
                    p.start_date.format("%Y-%m-%d").to_string(),
                    p.end_date.format("%Y-%m-%d").to_string(),
                    format!("{:.2}", p.opening_balance),
                    format!("{:.2}", p.total_income),
                    format!("{:.2}", p.total_expense),
                    format!("{:.2}", p.net_change),
                    format!("{:.2}", p.closing_balance),
                    p.is_negative.to_string(),
                ])
                .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
            }

            wtr.flush()
                .map_err(|e| (format!("Erro ao gerar CSV: {e}"), 1))?;
        }
    }

    Ok(())
}
