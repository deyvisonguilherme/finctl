use crate::errors::AppError;
use domain::{Category, CategoryId, TransactionKind, UserId};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use storage::CategoryRepository;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryItem {
    pub id: CategoryId,
    pub name: String,
    pub kind: TransactionKind,
    pub parent_id: Option<CategoryId>,
    pub parent_name: Option<String>,
}

pub struct CategoryService<'a> {
    pool: &'a PgPool,
}

impl<'a> CategoryService<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_category(
        &self,
        user_id: UserId,
        name: String,
        kind: TransactionKind,
        parent_query: Option<&str>,
    ) -> Result<Category, AppError> {
        let parent_id = if let Some(query) = parent_query {
            let parent = CategoryRepository::find_by_id_or_name(self.pool, user_id, query)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("Categoria pai '{query}' não encontrada."))
                })?;

            if parent.kind != kind {
                return Err(AppError::Validation(format!(
                    "A categoria pai '{}' é do tipo '{}', mas a categoria '{}' é do tipo '{}'. O tipo deve ser igual.",
                    parent.name,
                    parent.kind.display_pt_br(),
                    name,
                    kind.display_pt_br()
                )));
            }

            if parent.parent_id.is_some() {
                return Err(AppError::Validation(
                    "Não é permitido aninhar categorias em mais de 2 níveis (subcategoria de subcategoria).".to_string(),
                ));
            }

            Some(parent.id)
        } else {
            None
        };

        let category = Category::new(user_id, name, kind, parent_id)?;
        CategoryRepository::create(self.pool, &category).await?;
        Ok(category)
    }

    pub async fn list_categories(&self, user_id: UserId) -> Result<Vec<CategoryItem>, AppError> {
        let all = CategoryRepository::list_by_user(self.pool, user_id).await?;

        let mut items = Vec::with_capacity(all.len());
        for cat in &all {
            let parent_name = cat
                .parent_id
                .and_then(|p_id| all.iter().find(|c| c.id == p_id).map(|c| c.name.clone()));

            items.push(CategoryItem {
                id: cat.id,
                name: cat.name.clone(),
                kind: cat.kind,
                parent_id: cat.parent_id,
                parent_name,
            });
        }

        // Sort by kind, then root categories first, then subcategories under their parents
        items.sort_by(|a, b| {
            let kind_cmp = a.kind.as_str().cmp(b.kind.as_str());
            if kind_cmp != std::cmp::Ordering::Equal {
                return kind_cmp;
            }
            let a_root = a.parent_name.as_ref().unwrap_or(&a.name);
            let b_root = b.parent_name.as_ref().unwrap_or(&b.name);
            let root_cmp = a_root.cmp(b_root);
            if root_cmp != std::cmp::Ordering::Equal {
                root_cmp
            } else {
                a.parent_id
                    .is_some()
                    .cmp(&b.parent_id.is_some())
                    .then(a.name.cmp(&b.name))
            }
        });

        Ok(items)
    }

    /// Cria categorias e subcategorias padrão para um usuário de forma idempotente.
    pub async fn seed_default_categories(&self, user_id: UserId) -> Result<usize, AppError> {
        let default_seed = vec![
            // Despesas
            (
                "Alimentação",
                TransactionKind::Expense,
                vec!["Supermercado", "Restaurante", "Lanches"],
            ),
            (
                "Moradia",
                TransactionKind::Expense,
                vec!["Aluguel", "Contas residenciais", "Manutenção"],
            ),
            (
                "Transporte",
                TransactionKind::Expense,
                vec![
                    "Combustível",
                    "Transporte público",
                    "Manutenção veículo",
                    "Estacionamento",
                ],
            ),
            (
                "Saúde",
                TransactionKind::Expense,
                vec!["Farmácia", "Consultas e exames", "Plano de saúde"],
            ),
            (
                "Lazer",
                TransactionKind::Expense,
                vec!["Viagens", "Cinema e streaming", "Bares e festas"],
            ),
            (
                "Educação",
                TransactionKind::Expense,
                vec!["Cursos", "Livros e material"],
            ),
            ("Outras Despesas", TransactionKind::Expense, vec![]),
            // Receitas
            (
                "Salário",
                TransactionKind::Income,
                vec!["Salário mensal", "13º Salário", "Férias", "Bônus"],
            ),
            (
                "Rendimentos",
                TransactionKind::Income,
                vec!["Investimentos", "Dividendos", "Juros"],
            ),
            ("Outras Receitas", TransactionKind::Income, vec![]),
        ];

        let mut created_count = 0;
        let existing = CategoryRepository::list_by_user(self.pool, user_id).await?;

        for (root_name, kind, subcategories) in default_seed {
            let root_cat = if let Some(found) = existing
                .iter()
                .find(|c| c.parent_id.is_none() && c.name.eq_ignore_ascii_case(root_name))
            {
                found.clone()
            } else {
                let new_root = Category::new(user_id, root_name.to_string(), kind, None)?;
                CategoryRepository::create(self.pool, &new_root).await?;
                created_count += 1;
                new_root
            };

            for sub_name in subcategories {
                let exists = existing.iter().any(|c| {
                    c.parent_id == Some(root_cat.id) && c.name.eq_ignore_ascii_case(sub_name)
                });

                if !exists {
                    let new_sub =
                        Category::new(user_id, sub_name.to_string(), kind, Some(root_cat.id))?;
                    if CategoryRepository::create(self.pool, &new_sub)
                        .await
                        .is_ok()
                    {
                        created_count += 1;
                    }
                }
            }
        }

        Ok(created_count)
    }
}
