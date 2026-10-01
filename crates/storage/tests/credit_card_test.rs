use app::{
    AccountService, CategoryService, CreateAccountInput, CreateInstallmentsInput,
    CreateTransactionInput, TransactionService,
};
use chrono::NaiveDate;
use domain::{AccountKind, InvoiceStatus, Money, TransactionKind};
use rust_decimal_macros::dec;
use storage::{CardInvoiceRepository, TestDb};

#[tokio::test]
async fn test_credit_card_model_and_invoice_attribution() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let cat_service = CategoryService::new(pool);
    let tx_service = TransactionService::new(pool);

    // 1. Create credit card account: closing_day = 20, due_day = 27, limit = R$ 5.000,00
    let card = acc_service
        .create_account_with_input(CreateAccountInput {
            user_id,
            name: "Cartão Nubank".to_string(),
            kind: AccountKind::CreditCard,
            initial_balance: Money::from_decimal_non_negative(dec!(0.00)).unwrap(),
            closing_day: Some(20),
            due_day: Some(27),
            credit_limit: Some(Money::new(dec!(5000.00)).unwrap()),
        })
        .await
        .unwrap();

    assert_eq!(card.kind, AccountKind::CreditCard);
    assert_eq!(card.closing_day, Some(20));
    assert_eq!(card.due_day, Some(27));
    assert_eq!(card.credit_limit, Some(Money::new(dec!(5000.00)).unwrap()));

    let cat_mercado = cat_service
        .create_category(
            user_id,
            "Supermercado".to_string(),
            TransactionKind::Expense,
            None,
        )
        .await
        .unwrap();

    // 2. Explicit test: Purchase on closing day (2026-05-20) vs day after closing (2026-05-21)
    // Purchase on closing day -> Invoice 2026-05
    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: card.name.clone(),
            category_query: cat_mercado.name.clone(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(100.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
            description: "Compra no dia do fechamento".to_string(),
            status: None,
        })
        .await
        .unwrap();

    let inv_may = CardInvoiceRepository::find_by_account_and_month(pool, card.id, "2026-05")
        .await
        .unwrap()
        .expect("Fatura 2026-05 deve ter sido criada");

    assert_eq!(inv_may.month, "2026-05");
    assert_eq!(
        inv_may.closing_date,
        NaiveDate::from_ymd_opt(2026, 5, 20).unwrap()
    );
    assert_eq!(
        inv_may.due_date,
        NaiveDate::from_ymd_opt(2026, 5, 27).unwrap()
    );
    assert_eq!(inv_may.status, InvoiceStatus::Open);

    // Purchase on day after closing -> Invoice 2026-06
    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: card.name.clone(),
            category_query: cat_mercado.name.clone(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(150.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 5, 21).unwrap(),
            description: "Compra após o fechamento".to_string(),
            status: None,
        })
        .await
        .unwrap();

    let inv_june = CardInvoiceRepository::find_by_account_and_month(pool, card.id, "2026-06")
        .await
        .unwrap()
        .expect("Fatura 2026-06 deve ter sido criada");

    assert_eq!(inv_june.month, "2026-06");
    assert_eq!(
        inv_june.closing_date,
        NaiveDate::from_ymd_opt(2026, 6, 20).unwrap()
    );
    assert_eq!(
        inv_june.due_date,
        NaiveDate::from_ymd_opt(2026, 6, 27).unwrap()
    );
    assert_eq!(inv_june.status, InvoiceStatus::Open);

    // Another purchase in May on 2026-05-15 uses the SAME 2026-05 invoice without duplicating
    tx_service
        .create_transaction(CreateTransactionInput {
            user_id,
            account_query: card.name.clone(),
            category_query: cat_mercado.name.clone(),
            kind: TransactionKind::Expense,
            amount: Money::new(dec!(50.00)).unwrap(),
            date: NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
            description: "Outra compra em maio".to_string(),
            status: None,
        })
        .await
        .unwrap();

    let all_invoices = CardInvoiceRepository::list_by_account(pool, user_id, card.id)
        .await
        .unwrap();
    assert_eq!(all_invoices.len(), 2); // only 2026-05 and 2026-06

    // 3. Installments across N consecutive invoices
    // Installment purchase of 3 installments starting on 2026-07-10 (before closing 20)
    // Installments:
    // 1/3: 2026-07-10 -> Invoice 2026-07
    // 2/3: 2026-08-10 -> Invoice 2026-08
    // 3/3: 2026-09-10 -> Invoice 2026-09
    tx_service
        .create_installments(CreateInstallmentsInput {
            user_id,
            account_query: card.name.clone(),
            category_query: cat_mercado.name.clone(),
            kind: TransactionKind::Expense,
            total_amount: Some(Money::new(dec!(300.00)).unwrap()),
            installment_amount: None,
            installments_count: 3,
            start_date: NaiveDate::from_ymd_opt(2026, 7, 10).unwrap(),
            description: "Eletrodoméstico".to_string(),
        })
        .await
        .unwrap();

    let inv_jul = CardInvoiceRepository::find_by_account_and_month(pool, card.id, "2026-07")
        .await
        .unwrap();
    assert!(inv_jul.is_some());

    let inv_aug = CardInvoiceRepository::find_by_account_and_month(pool, card.id, "2026-08")
        .await
        .unwrap();
    assert!(inv_aug.is_some());

    let inv_sep = CardInvoiceRepository::find_by_account_and_month(pool, card.id, "2026-09")
        .await
        .unwrap();
    assert!(inv_sep.is_some());

    let updated_invoices = CardInvoiceRepository::list_by_account(pool, user_id, card.id)
        .await
        .unwrap();
    // 2026-05, 2026-06, 2026-07, 2026-08, 2026-09 -> 5 invoices total
    assert_eq!(updated_invoices.len(), 5);
}
