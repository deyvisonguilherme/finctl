use app::{
    AccountService, CategoryService, CreateAccountInput, CreateInstallmentsInput,
    CreateRecurringInput, CreateTransactionInput, CreateTransferInput, ForecastInput,
    ForecastService, RecurringService, TransactionService, TransferService,
};
use chrono::{Duration, Local, NaiveDate};
use domain::{AccountKind, ForecastGranularity, Money, RecurringFrequency, TransactionKind};
use rust_decimal_macros::dec;
use storage::TestDb;

#[tokio::test]
async fn test_forecast_no_double_counting_recurring_rule() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let tx_service = TransactionService::new(pool);
    let rec_service = RecurringService::new(pool);
    let forecast_service = ForecastService::new(pool);

    let today = Local::now().date_naive();

    // 1. Criar conta corrente com saldo 1000.00
    let _account = acc_service
        .create_account(
            user_id,
            "Conta Salário".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(1000.00)).unwrap(),
        )
        .await
        .unwrap();

    let _cat = cat_service
        .create_category(
            user_id,
            "Assinaturas".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    // 2. Criar regra recorrente mensal de 50.00
    let rule = rec_service
        .create_rule(CreateRecurringInput {
            user_id,
            account_query: "Conta Salário".to_string(),
            category_query: "Assinaturas".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::from_decimal_non_negative(dec!(50.00)).unwrap(),
            description: "Streaming Mensal".to_string(),
            frequency: RecurringFrequency::Monthly,
            day_of_month: Some(15),
            day_of_week: None,
            start_date: Some(today - Duration::days(40)),
            end_date: None,
        })
        .await
        .unwrap();

    // 3. Simular que uma transação já foi gerada e materializada no banco no mês passado
    let last_gen_date = today - Duration::days(10);
    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Conta Salário".to_string(),
            category_query: "Assinaturas".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::from_decimal_non_negative(dec!(50.00)).unwrap(),
            date: last_gen_date,
            description: "Streaming Mensal (gerado)".to_string(),
            status: Some(domain::TransactionStatus::Paid),
        })
        .await
        .unwrap();

    // Atualizar last_generated_date da regra para refletir a materialização
    let mut updated_rule = rule.clone();
    updated_rule.last_generated_date = Some(last_gen_date);
    storage::RecurringRepository::update(pool, &updated_rule)
        .await
        .unwrap();

    // 4. Rodar o forecast para 3 meses
    let forecast = forecast_service
        .generate_forecast(ForecastInput {
            user_id,
            months: Some(3),
            account_query: Some("Conta Salário".to_string()),
            granularity: Some(ForecastGranularity::Month),
            as_of_date: Some(today),
            include_goals: false,
        })
        .await
        .unwrap();

    // A transação do mês passado (last_gen_date) já foi debitada no saldo realizado (1000 - 50 = 950).
    // O forecast parte de 950.00 e apenas projeta as ocorrências futuras (curr > last_gen_date).
    assert_eq!(forecast.initial_balance, dec!(950.00));

    // A regra não deve contar a ocorrência já gerada duas vezes
    let total_projected_expense: rust_decimal::Decimal =
        forecast.periods.iter().map(|p| p.total_expense).sum();
    // Em 3 meses futuros, teremos no máximo 3 ou 4 ocorrências de 50.00
    assert!(total_projected_expense >= dec!(100.00) && total_projected_expense <= dec!(200.00));
}

#[tokio::test]
async fn test_forecast_installments_and_credit_card_due_date_rule_d08() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let tx_service = TransactionService::new(pool);
    let forecast_service = ForecastService::new(pool);

    let today = NaiveDate::from_ymd_opt(2026, 10, 5).unwrap();

    // 1. Criar conta corrente (saldo: 5000.00)
    let _checking = acc_service
        .create_account(
            user_id,
            "Nubank Conta".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(5000.00)).unwrap(),
        )
        .await
        .unwrap();

    // 2. Criar cartão de crédito com fechamento dia 20 e vencimento dia 28
    let _card = acc_service
        .create_account_with_input(CreateAccountInput {
            user_id,
            name: "Cartão Roxinho".to_string(),
            kind: AccountKind::CreditCard,
            initial_balance: Money::ZERO,
            closing_day: Some(20),
            due_day: Some(28),
            credit_limit: Some(Money::from_decimal_non_negative(dec!(10000.00)).unwrap()),
        })
        .await
        .unwrap();

    let _cat = cat_service
        .create_category(
            user_id,
            "Eletrônicos".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    // 3. Registrar compra parcelada no cartão em 2026-10-05 de 3x de 600.00 (total 1800.00)
    // Compra no dia 05/10 (antes do fechamento 20/10):
    // - Parcela 1: cai na fatura de 10/2026 -> vencimento 28/10/2026
    // - Parcela 2: cai na fatura de 11/2026 -> vencimento 28/11/2026
    // - Parcela 3: cai na fatura de 12/2026 -> vencimento 28/12/2026
    tx_service
        .create_installments(CreateInstallmentsInput {
            user_id,
            account_query: "Cartão Roxinho".to_string(),
            category_query: "Eletrônicos".to_string(),
            kind: TransactionKind::Expense,
            total_amount: Some(Money::from_decimal_non_negative(dec!(1800.00)).unwrap()),
            installment_amount: None,
            installments_count: 3,
            start_date: today,
            description: "Notebook Parcelado".to_string(),
        })
        .await
        .unwrap();

    // 4. Executar forecast global consolidado (regime de caixa: D-08)
    let forecast = forecast_service
        .generate_forecast(ForecastInput {
            user_id,
            months: Some(3),
            account_query: None,
            granularity: Some(ForecastGranularity::Month),
            as_of_date: Some(today),
            include_goals: false,
        })
        .await
        .unwrap();

    assert_eq!(forecast.initial_balance, dec!(5000.00));

    // A compra foi feita em 05/10. No regime de caixa (D-08), o valor NÃO sai em 05/10,
    // mas sim no vencimento das faturas correspondentes:
    // Outubro: -600.00 (vencimento 28/10)
    // Novembro: -600.00 (vencimento 28/11)
    // Dezembro: -600.00 (vencimento 28/12)
    let oct_period = forecast
        .periods
        .iter()
        .find(|p| p.period_label == "2026-10")
        .unwrap();
    assert_eq!(oct_period.total_expense, dec!(600.00));
    assert_eq!(oct_period.closing_balance, dec!(4400.00));

    let nov_period = forecast
        .periods
        .iter()
        .find(|p| p.period_label == "2026-11")
        .unwrap();
    assert_eq!(nov_period.total_expense, dec!(600.00));
    assert_eq!(nov_period.closing_balance, dec!(3800.00));

    let dec_period = forecast
        .periods
        .iter()
        .find(|p| p.period_label == "2026-12")
        .unwrap();
    assert_eq!(dec_period.total_expense, dec!(600.00));
    assert_eq!(dec_period.closing_balance, dec!(3200.00));

    // Nenhum período ficou negativo
    assert!(forecast.first_negative_period.is_none());
}

#[tokio::test]
async fn test_forecast_transfers_do_not_affect_consolidated_total() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let transfer_service = TransferService::new(pool);
    let forecast_service = ForecastService::new(pool);

    let today = Local::now().date_naive();

    // 1. Criar duas contas bancárias
    let _acc1 = acc_service
        .create_account(
            user_id,
            "Banco A".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(2000.00)).unwrap(),
        )
        .await
        .unwrap();

    let _acc2 = acc_service
        .create_account(
            user_id,
            "Banco B".to_string(),
            AccountKind::Savings,
            Money::from_decimal_non_negative(dec!(1000.00)).unwrap(),
        )
        .await
        .unwrap();

    // Saldo inicial consolidado = 3000.00
    // 2. Registrar transferência pendente de 500.00 de Banco A para Banco B
    let summary = transfer_service
        .create_transfer(CreateTransferInput {
            user_id,
            from_account_query: "Banco A".to_string(),
            to_account_query: "Banco B".to_string(),
            amount: Money::from_decimal_non_negative(dec!(500.00)).unwrap(),
            date: today + Duration::days(5),
            description: Some("Transferência Interna".to_string()),
        })
        .await
        .unwrap();

    let mut from_tx = summary.from_transaction;
    from_tx.status = domain::TransactionStatus::Pending;
    storage::TransactionRepository::update(pool, &from_tx)
        .await
        .unwrap();

    let mut to_tx = summary.to_transaction;
    to_tx.status = domain::TransactionStatus::Pending;
    storage::TransactionRepository::update(pool, &to_tx)
        .await
        .unwrap();

    // 3. No forecast consolidado global, o saldo NÃO deve ser alterado
    let forecast_global = forecast_service
        .generate_forecast(ForecastInput {
            user_id,
            months: Some(2),
            account_query: None,
            granularity: Some(ForecastGranularity::Month),
            as_of_date: Some(today),
            include_goals: false,
        })
        .await
        .unwrap();

    assert_eq!(forecast_global.initial_balance, dec!(3000.00));
    assert_eq!(
        forecast_global.periods.last().unwrap().closing_balance,
        dec!(3000.00)
    );

    // 4. Mas se filtrado por Banco A, reflete a saída de 500.00
    let forecast_acc1 = forecast_service
        .generate_forecast(ForecastInput {
            user_id,
            months: Some(2),
            account_query: Some("Banco A".to_string()),
            granularity: Some(ForecastGranularity::Month),
            as_of_date: Some(today),
            include_goals: false,
        })
        .await
        .unwrap();

    assert_eq!(forecast_acc1.initial_balance, dec!(2000.00));
    assert_eq!(
        forecast_acc1.periods.last().unwrap().closing_balance,
        dec!(1500.00)
    );
}

#[tokio::test]
async fn test_forecast_highlights_first_negative_period() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let tx_service = TransactionService::new(pool);
    let forecast_service = ForecastService::new(pool);

    let today = NaiveDate::from_ymd_opt(2026, 10, 10).unwrap();

    let _acc = acc_service
        .create_account(
            user_id,
            "Conta Única".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(1000.00)).unwrap(),
        )
        .await
        .unwrap();

    let _cat = cat_service
        .create_category(
            user_id,
            "Despesa Pesada".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    // Criar despesa pendente em novembro de 1500.00 (deixando saldo negativo em -500.00)
    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: "Conta Única".to_string(),
            category_query: "Despesa Pesada".to_string(),
            kind: TransactionKind::Expense,
            amount: Money::from_decimal_non_negative(dec!(1500.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 11, 15).unwrap(),
            description: "Aluguel Adiantado".to_string(),
            status: Some(domain::TransactionStatus::Pending),
        })
        .await
        .unwrap();

    let forecast = forecast_service
        .generate_forecast(ForecastInput {
            user_id,
            months: Some(3),
            account_query: None,
            granularity: Some(ForecastGranularity::Month),
            as_of_date: Some(today),
            include_goals: false,
        })
        .await
        .unwrap();

    assert_eq!(forecast.initial_balance, dec!(1000.00));
    assert_eq!(forecast.first_negative_period, Some("2026-11".to_string()));
    assert_eq!(forecast.lowest_projected_balance, dec!(-500.00));
}

#[tokio::test]
async fn test_forecast_with_include_goals_simulation() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let goal_service = app::GoalService::new(pool);
    let forecast_service = ForecastService::new(pool);

    let today = NaiveDate::from_ymd_opt(2026, 10, 8).unwrap();

    // Saldo inicial de R$ 5.000,00
    acc_service
        .create_account(
            user_id,
            "Conta Corrente".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(5000.00)).unwrap(),
        )
        .await
        .unwrap();

    // Criar meta manual de R$ 6.000,00 com data alvo para 3 meses à frente (janeiro/2027)
    // target = 6000, current = 0, remaining = 6000, months_left = 3 -> monthly_needed = 2000.00/mês
    let goal = goal_service
        .create_goal(app::CreateGoalInput {
            user_id,
            name: "Reserva Emergencial".to_string(),
            target_amount: Money::from_decimal_non_negative(dec!(6000.00)).unwrap(),
            target_date: Some(NaiveDate::from_ymd_opt(2027, 1, 15).unwrap()),
            account_query: None,
        })
        .await
        .unwrap();

    assert!(!goal.is_account_linked());

    // 1. Sem include_goals: saldo projetado permanece em 5.000,00
    let forecast_without_goals = forecast_service
        .generate_forecast(ForecastInput {
            user_id,
            months: Some(3),
            account_query: None,
            granularity: Some(ForecastGranularity::Month),
            as_of_date: Some(today),
            include_goals: false,
        })
        .await
        .unwrap();

    assert_eq!(forecast_without_goals.initial_balance, dec!(5000.00));
    assert_eq!(
        forecast_without_goals
            .periods
            .last()
            .unwrap()
            .closing_balance,
        dec!(5000.00)
    );

    // 2. Com include_goals: as metas saem como saída planejada de caixa
    let forecast_with_goals = forecast_service
        .generate_forecast(ForecastInput {
            user_id,
            months: Some(3),
            account_query: None,
            granularity: Some(ForecastGranularity::Month),
            as_of_date: Some(today),
            include_goals: true,
        })
        .await
        .unwrap();

    assert_eq!(forecast_with_goals.initial_balance, dec!(5000.00));
    // Cada mês deve ter despesa de 2.000,00 da meta
    let oct = forecast_with_goals
        .periods
        .iter()
        .find(|p| p.period_label == "2026-10")
        .unwrap();
    assert_eq!(oct.total_expense, dec!(2000.00));

    // Após 3 meses com -2000 cada: 5000 - 6000 = -1000.00 (primeiro período negativo detectado)
    assert!(forecast_with_goals.first_negative_period.is_some());
}
