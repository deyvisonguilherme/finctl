use app::{
    AccountService, CategoryReportInput, CategoryService, MonthlyReportInput, ReportService,
};
use chrono::NaiveDate;
use domain::{AccountKind, Money, Transaction, TransactionKind, TransactionStatus};
use rust_decimal_macros::dec;
use sqlx::Row;
use storage::{TestDb, TransactionRepository};
use uuid::Uuid;

#[tokio::test]
async fn test_aggregation_layer_monthly_and_categories() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let report_service = ReportService::new(pool);

    // 1. Create account
    let acc = acc_service
        .create_account(
            user_id,
            "Conta Corrente".to_string(),
            AccountKind::Checking,
            Money::from_decimal_non_negative(dec!(1000.00)).unwrap(),
        )
        .await
        .unwrap();

    // 2. Create category hierarchy:
    // Alimentação (Parent)
    //   -> Supermercado (Child)
    //   -> Restaurante (Child)
    // Salário (Parent Income)
    let _cat_alim = cat_service
        .create_category(
            user_id,
            "Alimentação".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    let cat_mercado = cat_service
        .create_category(
            user_id,
            "Supermercado".to_string(),
            TransactionKind::Expense,
            Some("Alimentação"),
        )
        .await
        .unwrap();

    let cat_restaurante = cat_service
        .create_category(
            user_id,
            "Restaurante".to_string(),
            TransactionKind::Expense,
            Some("Alimentação"),
        )
        .await
        .unwrap();

    let cat_salario = cat_service
        .create_category(
            user_id,
            "Salário".to_string(),
            TransactionKind::Income,
            None,
        )
        .await
        .unwrap();

    // 3. Create transactions directly to test various statuses & transfers:
    // Tx 1: Salário (Paid Income) R$ 5.000,00 on 2026-10-05
    let tx1 = Transaction::new_full(
        user_id,
        acc.id,
        cat_salario.id,
        TransactionKind::Income,
        Money::new(dec!(5000.00)).unwrap(),
        NaiveDate::from_ymd_opt(2026, 10, 5).unwrap(),
        "Salário Mensal".to_string(),
        TransactionStatus::Paid,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();
    TransactionRepository::create(pool, &tx1).await.unwrap();

    // Tx 2: Supermercado (Paid Expense) R$ 800,00 on 2026-10-10
    let tx2 = Transaction::new_full(
        user_id,
        acc.id,
        cat_mercado.id,
        TransactionKind::Expense,
        Money::new(dec!(800.00)).unwrap(),
        NaiveDate::from_ymd_opt(2026, 10, 10).unwrap(),
        "Compras da semana".to_string(),
        TransactionStatus::Paid,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();
    TransactionRepository::create(pool, &tx2).await.unwrap();

    // Tx 3: Restaurante (Paid Expense) R$ 200,00 on 2026-10-12
    let tx3 = Transaction::new_full(
        user_id,
        acc.id,
        cat_restaurante.id,
        TransactionKind::Expense,
        Money::new(dec!(200.00)).unwrap(),
        NaiveDate::from_ymd_opt(2026, 10, 12).unwrap(),
        "Jantar fora".to_string(),
        TransactionStatus::Paid,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();
    TransactionRepository::create(pool, &tx3).await.unwrap();

    // Tx 4: Supermercado (PENDING Expense) R$ 400,00 on 2026-10-25
    let tx4 = Transaction::new_full(
        user_id,
        acc.id,
        cat_mercado.id,
        TransactionKind::Expense,
        Money::new(dec!(400.00)).unwrap(),
        NaiveDate::from_ymd_opt(2026, 10, 25).unwrap(),
        "Feira pendente".to_string(),
        TransactionStatus::Pending,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();
    TransactionRepository::create(pool, &tx4).await.unwrap();

    // Tx 5: Transfer transaction (TRANSFER Expense) R$ 300,00 on 2026-10-15
    let tx5 = Transaction::new_full(
        user_id,
        acc.id,
        cat_mercado.id,
        TransactionKind::Expense,
        Money::new(dec!(300.00)).unwrap(),
        NaiveDate::from_ymd_opt(2026, 10, 15).unwrap(),
        "Transferência".to_string(),
        TransactionStatus::Paid,
        Some(Uuid::new_v4()),
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();
    TransactionRepository::create(pool, &tx5).await.unwrap();

    // --- TEST MONTHLY REPORT ---
    // Test A: monthly report without pending (default)
    let rep_oct = report_service
        .monthly_report(MonthlyReportInput {
            user_id,
            month: Some("2026-10".to_string()),
            year: None,
            account_query: None,
            include_pending: false,
        })
        .await
        .unwrap();

    assert_eq!(rep_oct.len(), 1);
    assert_eq!(rep_oct[0].month, "2026-10");
    assert_eq!(rep_oct[0].total_income.as_decimal(), dec!(5000.00));
    // tx2 (800) + tx3 (200) = 1000 (tx4 is pending, tx5 is transfer -> both excluded)
    assert_eq!(rep_oct[0].total_expense.as_decimal(), dec!(1000.00));
    assert_eq!(rep_oct[0].net_balance, dec!(4000.00));
    // savings rate: 4000 / 5000 = 80.00%
    assert_eq!(rep_oct[0].savings_rate, dec!(80.00));

    // Test B: monthly report WITH pending
    let rep_oct_pending = report_service
        .monthly_report(MonthlyReportInput {
            user_id,
            month: Some("2026-10".to_string()),
            year: None,
            account_query: None,
            include_pending: true,
        })
        .await
        .unwrap();

    assert_eq!(rep_oct_pending.len(), 1);
    // tx2 (800) + tx3 (200) + tx4 (400 pending) = 1400.00 (tx5 transfer still excluded)
    assert_eq!(rep_oct_pending[0].total_expense.as_decimal(), dec!(1400.00));
    assert_eq!(rep_oct_pending[0].net_balance, dec!(3600.00));

    // --- TEST CATEGORY REPORT ---
    // Test C: Depth = 1 (Rollup subcategories into parent)
    let cat_rep_d1 = report_service
        .category_report(CategoryReportInput {
            user_id,
            month: Some("2026-10".to_string()),
            from_date: None,
            to_date: None,
            account_query: None,
            kind: Some(TransactionKind::Expense),
            depth: 1,
            include_pending: false,
            tag: None,
        })
        .await
        .unwrap();

    assert_eq!(cat_rep_d1.total_amount.as_decimal(), dec!(1000.00));
    assert_eq!(cat_rep_d1.items.len(), 1);
    assert_eq!(cat_rep_d1.items[0].category_name, "Alimentação");
    assert_eq!(cat_rep_d1.items[0].total_amount.as_decimal(), dec!(1000.00));
    assert_eq!(cat_rep_d1.items[0].transaction_count, 2);
    assert_eq!(cat_rep_d1.items[0].percentage, dec!(100.00));

    // Test D: Depth = 2 (Keep subcategories separate)
    let cat_rep_d2 = report_service
        .category_report(CategoryReportInput {
            user_id,
            month: Some("2026-10".to_string()),
            from_date: None,
            to_date: None,
            account_query: None,
            kind: Some(TransactionKind::Expense),
            depth: 2,
            include_pending: false,
            tag: None,
        })
        .await
        .unwrap();

    assert_eq!(cat_rep_d2.total_amount.as_decimal(), dec!(1000.00));
    assert_eq!(cat_rep_d2.items.len(), 2);
    // Ordered by amount DESC
    assert_eq!(cat_rep_d2.items[0].category_name, "Supermercado");
    assert_eq!(cat_rep_d2.items[0].total_amount.as_decimal(), dec!(800.00));
    assert_eq!(cat_rep_d2.items[0].percentage, dec!(80.00));
    assert_eq!(
        cat_rep_d2.items[0].parent_name.as_deref(),
        Some("Alimentação")
    );

    assert_eq!(cat_rep_d2.items[1].category_name, "Restaurante");
    assert_eq!(cat_rep_d2.items[1].total_amount.as_decimal(), dec!(200.00));
    assert_eq!(cat_rep_d2.items[1].percentage, dec!(20.00));
    assert_eq!(
        cat_rep_d2.items[1].parent_name.as_deref(),
        Some("Alimentação")
    );

    // --- TEST EXPLAIN QUERY EXECUTION ---
    let explain_rows = sqlx::query(
        r#"
        EXPLAIN
        SELECT 
            TO_CHAR(date, 'YYYY-MM') AS month_str,
            COALESCE(SUM(CASE WHEN kind = 'income' THEN amount ELSE 0 END), 0) AS total_income,
            COALESCE(SUM(CASE WHEN kind = 'expense' THEN amount ELSE 0 END), 0) AS total_expense
        FROM transactions
        WHERE user_id = $1
          AND transfer_id IS NULL
          AND date >= $2
          AND date <= $3
          AND account_id = $4
          AND status = 'paid'
        GROUP BY month_str
        "#,
    )
    .bind(user_id.as_uuid())
    .bind(NaiveDate::from_ymd_opt(2026, 10, 1).unwrap())
    .bind(NaiveDate::from_ymd_opt(2026, 10, 31).unwrap())
    .bind(acc.id.as_uuid())
    .fetch_all(pool)
    .await
    .unwrap();

    assert!(!explain_rows.is_empty());
    let mut explain_text = String::new();
    for r in explain_rows {
        let line: String = r.get(0);
        explain_text.push_str(&line);
        explain_text.push('\n');
    }
    println!("EXPLAIN query output:\n{explain_text}");
}
