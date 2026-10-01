use app::{
    AccountService, AttachmentService, BalanceService, BudgetService, CardService,
    CategoryReportInput, CategoryService, CreateAccountInput, CreateInstallmentsInput,
    CreateRecurringInput, CreateTransactionInput, CreateTransferInput, MonthlyReportInput,
    PayCardInvoiceInput, RecurringService, ReportService, RunRecurringInput, TagService,
    TransactionService, TransferService,
};
use chrono::NaiveDate;
use domain::{
    AccountKind, BudgetIndicator, Money, RecurringFrequency, TransactionKind, TransactionStatus,
};
use rust_decimal_macros::dec;
use std::fs;
use storage::TestDb;

#[tokio::test]
async fn test_full_finctl_lifecycle_e2e() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    // Services
    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let tx_service = TransactionService::new(pool);
    let recurring_service = RecurringService::new(pool);
    let card_service = CardService::new(pool);
    let transfer_service = TransferService::new(pool);
    let balance_service = BalanceService::new(pool);
    let report_service = ReportService::new(pool);
    let budget_service = BudgetService::new(pool);
    let tag_service = TagService::new(pool);
    let attachment_service = AttachmentService::new(pool);

    // ==========================================
    // 1. ACCOUNTS SETUP
    // ==========================================
    let acc_checking = acc_service
        .create_account_with_input(CreateAccountInput {
            user_id,
            name: "Conta Corrente".to_string(),
            kind: AccountKind::Checking,
            initial_balance: Money::from_decimal_non_negative(dec!(5000.00)).unwrap(),
            credit_limit: None,
            closing_day: None,
            due_day: None,
        })
        .await
        .unwrap();

    let acc_savings = acc_service
        .create_account_with_input(CreateAccountInput {
            user_id,
            name: "Poupança".to_string(),
            kind: AccountKind::Savings,
            initial_balance: Money::from_decimal_non_negative(dec!(1000.00)).unwrap(),
            credit_limit: None,
            closing_day: None,
            due_day: None,
        })
        .await
        .unwrap();

    let acc_card = acc_service
        .create_account_with_input(CreateAccountInput {
            user_id,
            name: "Cartão Nubank".to_string(),
            kind: AccountKind::CreditCard,
            initial_balance: Money::from_decimal_non_negative(dec!(0.00)).unwrap(),
            credit_limit: Some(Money::from_decimal_non_negative(dec!(3000.00)).unwrap()),
            closing_day: Some(25),
            due_day: Some(5),
        })
        .await
        .unwrap();

    // ==========================================
    // 2. CATEGORIES HIERARCHY
    // ==========================================
    let cat_salary = cat_service
        .create_category(
            user_id,
            "Salário".to_string(),
            TransactionKind::Income,
            None,
        )
        .await
        .unwrap();

    let cat_food_parent = cat_service
        .create_category(
            user_id,
            "Alimentação".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    let cat_market = cat_service
        .create_category(
            user_id,
            "Supermercado".to_string(),
            TransactionKind::Expense,
            Some("Alimentação"),
        )
        .await
        .unwrap();

    let cat_housing = cat_service
        .create_category(
            user_id,
            "Moradia".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    // ==========================================
    // 3. RECURRING SALARY & EXECUTION
    // ==========================================
    let _rec_salary = recurring_service
        .create_rule(CreateRecurringInput {
            user_id,
            description: "Salário Mensal Tech Corp".to_string(),
            amount: Money::new(dec!(6000.00)).unwrap(),
            kind: TransactionKind::Income,
            account_query: acc_checking.name.clone(),
            category_query: cat_salary.name.clone(),
            frequency: RecurringFrequency::Monthly,
            start_date: Some(NaiveDate::from_ymd_opt(2026, 10, 5).unwrap()),
            end_date: None,
            day_of_month: Some(5),
            day_of_week: None,
        })
        .await
        .unwrap();

    // Run recurring for October 2026
    let rec_run_1 = recurring_service
        .run_recurring(RunRecurringInput {
            user_id,
            until_date: Some(NaiveDate::from_ymd_opt(2026, 10, 31).unwrap()),
            dry_run: false,
        })
        .await
        .unwrap();
    assert_eq!(rec_run_1.transactions_generated.len(), 1);

    // Idempotency: second run generates 0 new transactions
    let rec_run_2 = recurring_service
        .run_recurring(RunRecurringInput {
            user_id,
            until_date: Some(NaiveDate::from_ymd_opt(2026, 10, 31).unwrap()),
            dry_run: false,
        })
        .await
        .unwrap();
    assert_eq!(rec_run_2.transactions_generated.len(), 0);

    // Pay the generated salary transaction so it becomes realized
    let pending_txs = tx_service
        .list_transactions(app::ListTransactionsInput {
            user_id,
            month: Some("2026-10".to_string()),
            status: Some(TransactionStatus::Pending),
            ..Default::default()
        })
        .await
        .unwrap();
    let salary_tx = pending_txs
        .iter()
        .find(|t| t.description.contains("Salário"))
        .unwrap();
    tx_service
        .pay_transaction(user_id, salary_tx.id, None)
        .await
        .unwrap();

    // ==========================================
    // 4. DIRECT TRANSACTIONS (EXPENSES)
    // ==========================================
    // Rent on Oct 6th
    let tx_rent = tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: acc_checking.name.clone(),
            category_query: cat_housing.name.clone(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(1200.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 6).unwrap(),
            description: "Aluguel Apartamento".to_string(),
            status: Some(TransactionStatus::Paid),
        })
        .await
        .unwrap();

    // Supermarket on Oct 10th
    let _tx_market = tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: acc_checking.name.clone(),
            category_query: cat_market.name.clone(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(300.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 10).unwrap(),
            description: "Compras Semanais Mercado".to_string(),
            status: Some(TransactionStatus::Paid),
        })
        .await
        .unwrap();

    // ==========================================
    // 5. CREDIT CARD PURCHASES & INSTALLMENTS
    // ==========================================
    // Purchase 1: Before closing date (Oct 15) -> Invoice 2026-10
    let _tx_card_1 = tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: acc_card.name.clone(),
            category_query: cat_food_parent.name.clone(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(150.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 15).unwrap(),
            description: "Jantar Restaurante".to_string(),
            status: Some(TransactionStatus::Paid),
        })
        .await
        .unwrap();

    // Purchase 2: Installment purchase on Oct 20 -> 3x of R$ 200,00
    let inst_summary = tx_service
        .create_installments(CreateInstallmentsInput {
            user_id,
            account_query: acc_card.name.clone(),
            category_query: cat_housing.name.clone(),
            kind: TransactionKind::Expense,
            total_amount: Some(Money::new(dec!(600.00)).unwrap()),
            installment_amount: None,
            installments_count: 3,
            start_date: NaiveDate::from_ymd_opt(2026, 10, 20).unwrap(),
            description: "Mesa de Trabalho Escritório".to_string(),
        })
        .await
        .unwrap();
    assert_eq!(inst_summary.count, 3);

    // Realize installment transactions
    for inst in &inst_summary.transactions {
        tx_service
            .pay_transaction(user_id, inst.id, None)
            .await
            .unwrap();
    }

    // Purchase 3: After closing date (Oct 26) -> Rollover to Invoice 2026-11
    let _tx_card_after_closing = tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: acc_card.name.clone(),
            category_query: cat_food_parent.name.clone(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(80.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 26).unwrap(),
            description: "Café da Manhã Pós-Fechamento".to_string(),
            status: Some(TransactionStatus::Paid),
        })
        .await
        .unwrap();

    // ==========================================
    // 6. CARD INVOICE VERIFICATION & PAYMENT
    // ==========================================
    // Invoice 2026-10: 150.00 (Purchase 1) + 200.00 (Installment 1) = 350.00
    let inv_oct = card_service
        .show_invoice(user_id, &acc_card.name, Some("2026-10".to_string()))
        .await
        .unwrap();
    assert_eq!(inv_oct.total_amount.as_decimal(), dec!(350.00));
    assert_eq!(inv_oct.available_limit.unwrap().as_decimal(), dec!(2170.00)); // 3000 - (150 + 600 + 80)

    // Invoice 2026-11: 200.00 (Installment 2) + 80.00 (Purchase 3) = 280.00
    let inv_nov = card_service
        .show_invoice(user_id, &acc_card.name, Some("2026-11".to_string()))
        .await
        .unwrap();
    assert_eq!(inv_nov.total_amount.as_decimal(), dec!(280.00));

    // Close and Pay Invoice 2026-10
    card_service
        .close_invoice(user_id, &acc_card.name, Some("2026-10".to_string()))
        .await
        .unwrap();

    let pay_summary = card_service
        .pay_invoice(PayCardInvoiceInput {
            user_id,
            card_query: acc_card.name.clone(),
            from_account_query: acc_checking.name.clone(),
            month: Some("2026-10".to_string()),
            amount: None, // Pay full invoice amount (350.00)
            date: Some(NaiveDate::from_ymd_opt(2026, 11, 5).unwrap()),
        })
        .await
        .unwrap();
    assert_eq!(pay_summary.amount_paid.as_decimal(), dec!(350.00));
    assert_eq!(pay_summary.invoice_status, domain::InvoiceStatus::Paid);

    // Limit restores by R$ 350,00 -> 2520.00
    let inv_after_pay = card_service
        .show_invoice(user_id, &acc_card.name, Some("2026-10".to_string()))
        .await
        .unwrap();
    assert_eq!(
        inv_after_pay.available_limit.unwrap().as_decimal(),
        dec!(2520.00)
    );

    // ==========================================
    // 7. ACCOUNT TRANSFERS
    // ==========================================
    // Transfer R$ 500,00 from Checking to Savings
    let transfer = transfer_service
        .create_transfer(CreateTransferInput {
            user_id,
            from_account_query: acc_checking.name.clone(),
            to_account_query: acc_savings.name.clone(),
            amount: Money::new(dec!(500.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 10, 20).unwrap(),
            description: Some("Aporte Reserva de Emergência".to_string()),
        })
        .await
        .unwrap();
    assert_eq!(transfer.amount.as_decimal(), dec!(500.00));

    // ==========================================
    // 8. TAGS & ATTACHMENTS
    // ==========================================
    tag_service
        .tag_transaction(
            user_id,
            &tx_rent.id.to_string(),
            vec!["moradia-fixo".to_string(), "essencial".to_string()],
        )
        .await
        .unwrap();

    let tmp_recibo = format!("/tmp/recibo_aluguel_{}.pdf", uuid::Uuid::new_v4());
    fs::write(&tmp_recibo, b"PDF_RECIBO_ALUGUEL_OUTUBRO").unwrap();

    let attachment = attachment_service
        .attach(
            user_id,
            &tx_rent.id.to_string(),
            &tmp_recibo,
            Some("Recibo assinado pelo proprietário".to_string()),
        )
        .await
        .unwrap();
    assert!(attachment.sha256.is_some());

    // ==========================================
    // 9. BUDGET MONITORING
    // ==========================================
    // Budget for Alimentação: R$ 500,00
    budget_service
        .set_budget(
            user_id,
            "Alimentação".to_string(),
            Money::new(dec!(500.00)).unwrap(),
            Some("2026-10".to_string()),
        )
        .await
        .unwrap();

    let budget_statuses = budget_service
        .get_budget_status(user_id, "2026-10".to_string())
        .await
        .unwrap();
    let food_budget = budget_statuses
        .iter()
        .find(|b| b.category_name == "Alimentação")
        .unwrap();
    // Supermercado (300) + Restaurante (150) + Café (80) = 530.00 -> Exceeded (Alert Exceeded)
    assert_eq!(food_budget.consumed_amount.as_decimal(), dec!(530.00));
    assert_eq!(food_budget.indicator, BudgetIndicator::Exceeded);

    // ==========================================
    // 10. BALANCES AND REPORTS ACCURACY
    // ==========================================
    // Checking: 5000 (init) + 6000 (salario) - 1200 (aluguel) - 300 (mercado) - 350 (card pay) - 500 (transf) = 8650.00
    let balance_report = balance_service
        .get_balance(user_id, None, false)
        .await
        .unwrap();
    let acc_chk_bal = balance_report
        .accounts
        .iter()
        .find(|a| a.account_name == "Conta Corrente")
        .unwrap();
    assert_eq!(acc_chk_bal.current_balance, dec!(8650.00));

    // Savings: 1000 (init) + 500 (transf) = 1500.00
    let acc_sav_bal = balance_report
        .accounts
        .iter()
        .find(|a| a.account_name == "Poupança")
        .unwrap();
    assert_eq!(acc_sav_bal.current_balance, dec!(1500.00));

    // Credit card: -830.00 (purchases) + 350.00 (payment) = -480.00
    let acc_crd_bal = balance_report
        .accounts
        .iter()
        .find(|a| a.account_name == "Cartão Nubank")
        .unwrap();
    assert_eq!(acc_crd_bal.current_balance, dec!(-480.00));

    // Consolidated Total Balance: 8650 + 1500 - 480 = 9670.00
    assert_eq!(balance_report.total_balance, dec!(9670.00));

    // Monthly Report for 2026-10:
    // Transfers (500) and card payments (350) are excluded from report totals!
    // Income: 6000.00 (Salario)
    // Expense: Aluguel (1200) + Mercado (300) + Jantar (150) + Mesa 1/3 (200) + Café (80) = 1930.00
    // Net: 6000 - 1930 = 4070.00
    let monthly_rep = report_service
        .monthly_report(MonthlyReportInput {
            user_id,
            month: Some("2026-10".to_string()),
            year: None,
            account_query: None,
            include_pending: false,
        })
        .await
        .unwrap();
    assert_eq!(monthly_rep.len(), 1);
    assert_eq!(monthly_rep[0].total_income.as_decimal(), dec!(6000.00));
    assert_eq!(monthly_rep[0].total_expense.as_decimal(), dec!(1930.00));
    assert_eq!(monthly_rep[0].net_balance, dec!(4070.00));

    // Full Year Report for 2026 (includes Mesa installments in Oct, Nov, Dec = 600.00):
    // Expense: Aluguel (1200) + Mercado (300) + Jantar (150) + Mesa (600) + Café (80) = 2330.00
    // Net: 6000 - 2330 = 3670.00
    let yearly_rep = report_service
        .monthly_report(MonthlyReportInput {
            user_id,
            month: None,
            year: Some(2026),
            account_query: None,
            include_pending: false,
        })
        .await
        .unwrap();
    let year_total_income: rust_decimal::Decimal =
        yearly_rep.iter().map(|m| m.total_income.as_decimal()).sum();
    let year_total_expense: rust_decimal::Decimal = yearly_rep
        .iter()
        .map(|m| m.total_expense.as_decimal())
        .sum();
    let year_net: rust_decimal::Decimal = yearly_rep.iter().map(|m| m.net_balance).sum();
    assert_eq!(year_total_income, dec!(6000.00));
    assert_eq!(year_total_expense, dec!(2330.00));
    assert_eq!(year_net, dec!(3670.00));

    // Category Report with Tag Filter:
    let tag_report = report_service
        .category_report(CategoryReportInput {
            user_id,
            month: Some("2026-10".to_string()),
            tag: Some("moradia-fixo".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(tag_report.items.len(), 1);
    assert_eq!(tag_report.items[0].category_name, "Moradia");
    assert_eq!(tag_report.total_amount.as_decimal(), dec!(1200.00));

    let _ = fs::remove_file(tmp_recibo);
}
