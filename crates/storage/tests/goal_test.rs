use app::{AccountService, AddContributionInput, CreateGoalInput, EditGoalInput, GoalService};
use chrono::{Duration, Local};
use domain::{AccountKind, Money};
use rust_decimal_macros::dec;
use storage::TestDb;

#[tokio::test]
async fn test_manual_goal_workflow_and_completion() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let goal_service = GoalService::new(pool);
    let today = Local::now().date_naive();
    let target_date = today + Duration::days(180);

    // 1. Criar meta manual
    let target_amount = Money::from_decimal_non_negative(dec!(1000.00)).unwrap();
    let goal = goal_service
        .create_goal(CreateGoalInput {
            user_id,
            name: "Viagem de Férias".to_string(),
            target_amount,
            target_date: Some(target_date),
            account_query: None,
        })
        .await
        .unwrap();

    assert_eq!(goal.name, "Viagem de Férias");
    assert!(!goal.is_account_linked());
    assert!(goal.completed_at.is_none());

    // 2. Verificar listagem ativa
    let active_goals = goal_service.list_goals(user_id, false).await.unwrap();
    assert_eq!(active_goals.len(), 1);
    assert_eq!(active_goals[0].current_amount.as_decimal(), dec!(0.00));
    assert_eq!(active_goals[0].remaining_amount.as_decimal(), dec!(1000.00));
    assert_eq!(active_goals[0].percentage, dec!(0.00));
    assert!(!active_goals[0].is_completed);
    assert!(active_goals[0].monthly_needed.is_some());

    // 3. Registrar primeiro aporte manual de 400.00
    let contrib1 = goal_service
        .add_contribution(AddContributionInput {
            user_id,
            goal_identifier: "Viagem de Férias".to_string(),
            amount: Money::from_decimal_non_negative(dec!(400.00)).unwrap(),
            date: today,
            note: Some("Primeiro aporte".to_string()),
        })
        .await
        .unwrap();

    assert_eq!(contrib1.amount.as_decimal(), dec!(400.00));

    let progress = goal_service
        .get_goal_progress(user_id, "Viagem de Férias")
        .await
        .unwrap();
    assert_eq!(progress.current_amount.as_decimal(), dec!(400.00));
    assert_eq!(progress.remaining_amount.as_decimal(), dec!(600.00));
    assert_eq!(progress.percentage, dec!(40.00));
    assert!(!progress.is_completed);
    assert_eq!(progress.contributions.len(), 1);

    // 4. Registrar segundo aporte de 600.00 (atingindo o alvo)
    goal_service
        .add_contribution(AddContributionInput {
            user_id,
            goal_identifier: "Viagem de Férias".to_string(),
            amount: Money::from_decimal_non_negative(dec!(600.00)).unwrap(),
            date: today,
            note: Some("Aporte final".to_string()),
        })
        .await
        .unwrap();

    // 5. Verificar que a meta foi concluída e sumiu da listagem padrão
    let active_goals_after = goal_service.list_goals(user_id, false).await.unwrap();
    assert_eq!(active_goals_after.len(), 0);

    let all_goals = goal_service.list_goals(user_id, true).await.unwrap();
    assert_eq!(all_goals.len(), 1);
    assert!(all_goals[0].is_completed);
    assert_eq!(all_goals[0].current_amount.as_decimal(), dec!(1000.00));
    assert_eq!(all_goals[0].remaining_amount.as_decimal(), dec!(0.00));
    assert_eq!(all_goals[0].percentage, dec!(100.00));

    // 6. Aporte em meta concluída deve retornar erro
    let err_contrib = goal_service
        .add_contribution(AddContributionInput {
            user_id,
            goal_identifier: "Viagem de Férias".to_string(),
            amount: Money::from_decimal_non_negative(dec!(100.00)).unwrap(),
            date: today,
            note: None,
        })
        .await;
    assert!(err_contrib.is_err());

    // 7. Reabrir meta aumentando o valor alvo
    let updated = goal_service
        .edit_goal(EditGoalInput {
            user_id,
            identifier: "Viagem de Férias".to_string(),
            name: None,
            target_amount: Some(Money::from_decimal_non_negative(dec!(1500.00)).unwrap()),
            target_date: None,
            reopen: true,
        })
        .await
        .unwrap();

    assert_eq!(updated.target_amount.as_decimal(), dec!(1500.00));
    assert!(updated.completed_at.is_none());

    let active_after_reopen = goal_service.list_goals(user_id, false).await.unwrap();
    assert_eq!(active_after_reopen.len(), 1);
    assert_eq!(
        active_after_reopen[0].remaining_amount.as_decimal(),
        dec!(500.00)
    );
}

#[tokio::test]
async fn test_account_linked_goal_workflow_and_restrictions() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let acc_service = AccountService::new(pool);
    let goal_service = GoalService::new(pool);

    // 1. Criar conta Poupança com saldo inicial de 3.000,00
    let account = acc_service
        .create_account(
            user_id,
            "Poupança Sonhos".to_string(),
            AccountKind::Savings,
            Money::from_decimal_non_negative(dec!(3000.00)).unwrap(),
        )
        .await
        .unwrap();

    // 2. Criar meta vinculada a essa conta com alvo de 10.000,00
    let goal = goal_service
        .create_goal(CreateGoalInput {
            user_id,
            name: "Carro Próprio".to_string(),
            target_amount: Money::from_decimal_non_negative(dec!(10000.00)).unwrap(),
            target_date: None,
            account_query: Some("Poupança Sonhos".to_string()),
        })
        .await
        .unwrap();

    assert_eq!(goal.account_id, Some(account.id));
    assert!(goal.is_account_linked());

    // 3. Progresso reflete o saldo da conta vinculada (3000 / 10000 = 30%)
    let progress = goal_service
        .get_goal_progress(user_id, "Carro Próprio")
        .await
        .unwrap();
    assert_eq!(progress.current_amount.as_decimal(), dec!(3000.00));
    assert_eq!(progress.remaining_amount.as_decimal(), dec!(7000.00));
    assert_eq!(progress.percentage, dec!(30.00));
    assert!(!progress.is_completed);

    // 4. Meta sem data alvo não possui aporte mensal necessário
    assert!(progress.monthly_needed.is_none());

    // 5. Tentativa de aporte manual em meta vinculada DEVE falhar (código de erro de validação)
    let today = Local::now().date_naive();
    let err_contrib = goal_service
        .add_contribution(AddContributionInput {
            user_id,
            goal_identifier: "Carro Próprio".to_string(),
            amount: Money::from_decimal_non_negative(dec!(500.00)).unwrap(),
            date: today,
            note: None,
        })
        .await;

    assert!(err_contrib.is_err());
    match err_contrib.unwrap_err() {
        app::AppError::Validation(msg) => {
            assert!(msg.contains("vinculada a uma conta"));
        }
        other => panic!("Esperado erro de validação, recebido: {:?}", other),
    }

    // 6. Não permite vincular meta a Cartão de Crédito
    let _cc = acc_service
        .create_account(
            user_id,
            "Cartão Black".to_string(),
            AccountKind::CreditCard,
            Money::ZERO,
        )
        .await
        .unwrap();

    let err_cc_link = goal_service
        .create_goal(CreateGoalInput {
            user_id,
            name: "Meta Inválida Cartão".to_string(),
            target_amount: Money::from_decimal_non_negative(dec!(1000.00)).unwrap(),
            target_date: None,
            account_query: Some("Cartão Black".to_string()),
        })
        .await;

    assert!(err_cc_link.is_err());
    match err_cc_link.unwrap_err() {
        app::AppError::Validation(msg) => {
            assert!(msg.contains("Cartão de Crédito"));
        }
        other => panic!("Esperado erro de validação, recebido: {:?}", other),
    }

    // 7. Soft delete da meta
    let deleted = goal_service
        .delete_goal(user_id, "Carro Próprio")
        .await
        .unwrap();
    assert_eq!(deleted.id, goal.id);

    let list_after_delete = goal_service.list_goals(user_id, true).await.unwrap();
    assert_eq!(list_after_delete.len(), 0);
}

#[tokio::test]
async fn test_unique_goal_name_per_user() {
    let test_db = TestDb::setup().await;
    let pool = &test_db.pool;
    let user_id = test_db.user_id;

    let goal_service = GoalService::new(pool);
    let target = Money::from_decimal_non_negative(dec!(500.00)).unwrap();

    goal_service
        .create_goal(CreateGoalInput {
            user_id,
            name: "Meta Única".to_string(),
            target_amount: target,
            target_date: None,
            account_query: None,
        })
        .await
        .unwrap();

    // Criar com mesmo nome deve falhar
    let err_dup = goal_service
        .create_goal(CreateGoalInput {
            user_id,
            name: "Meta Única".to_string(),
            target_amount: target,
            target_date: None,
            account_query: None,
        })
        .await;

    assert!(err_dup.is_err());
}
