use crate::format::OutputFormat;
use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use domain::TransactionKind;
use std::path::PathBuf;
use uuid::Uuid;

pub const FINCTL_VERSION: &str = match option_env!("FINCTL_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};

#[derive(Parser, Debug)]
#[command(
    name = "finctl",
    version = FINCTL_VERSION,
    about = "Sistema de controle financeiro pessoal"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Inicializa o banco de dados e cria as categorias padrão
    Init(InitArgs),

    /// Operações de banco de dados
    Db {
        #[command(subcommand)]
        subcommand: DbCommands,
    },

    /// Gerenciamento de contas bancárias e carteiras
    Account {
        #[command(subcommand)]
        subcommand: AccountCommands,
    },

    /// Gerenciamento de categorias de receitas e despesas
    Category {
        #[command(subcommand)]
        subcommand: CategoryCommands,
    },

    /// Registra uma receita
    Income {
        #[command(subcommand)]
        subcommand: IncomeCommands,
    },

    /// Registra uma despesa
    Expense {
        #[command(subcommand)]
        subcommand: ExpenseCommands,
    },

    /// Transferências entre contas
    Transfer {
        #[command(subcommand)]
        subcommand: TransferCommands,
    },

    /// Gerenciamento de cartão de crédito e faturas
    Card {
        #[command(subcommand)]
        subcommand: CardCommands,
    },

    /// Consulta e gerenciamento de transações/lançamentos
    Tx {
        #[command(subcommand)]
        subcommand: TxCommands,
    },

    /// Gerenciamento de orçamentos por categoria
    Budget {
        #[command(subcommand)]
        subcommand: BudgetCommands,
    },

    /// Gerenciamento de regras de lançamentos recorrentes
    Recurring {
        #[command(subcommand)]
        subcommand: RecurringCommands,
    },

    /// Consulta de saldos consolidados por conta e total geral
    Balance(BalanceArgs),

    /// Relatórios financeiros e comparativos
    Report {
        #[command(subcommand)]
        subcommand: ReportCommands,
    },

    /// Exportação de dados para arquivos (CSV, JSON)
    Export {
        #[command(subcommand)]
        subcommand: ExportCommands,
    },

    /// Importação de dados a partir de arquivos externos
    Import {
        #[command(subcommand)]
        subcommand: ImportCommands,
    },

    /// Conciliação bancária de lançamentos com extrato CSV
    Reconcile(ReconcileArgs),

    /// Expura definitivamente registros que foram excluídos há mais tempo que o especificado
    Purge(PurgeArgs),

    /// Gerenciamento de tags para categorização transversal
    Tag {
        #[command(subcommand)]
        subcommand: TagCommands,
    },

    /// Consulta registros de auditoria do sistema
    Audit {
        #[command(subcommand)]
        subcommand: AuditCommands,
    },

    /// Executa o backup lógico do banco de dados em formato custom (-Fc)
    Backup(BackupArgs),

    /// Restaura um backup de banco de dados via pg_restore
    Restore(RestoreArgs),

    /// Gera scripts de autocompletar para shells (bash, zsh, fish, powershell, elvish)
    Completions(CompletionsArgs),

    /// Gera páginas de manual (man pages) da CLI
    Man(ManArgs),

    /// Inicia a interface interativa no terminal (TUI)
    Tui(TuiArgs),
}

#[derive(Subcommand, Debug)]
pub enum DbCommands {
    /// Testa a conexão com o banco de dados
    Ping,
    /// Aplica as migrações pendentes no banco de dados
    Migrate,
    /// Executa o backup do banco de dados (alias para `finctl backup`)
    Backup(BackupArgs),
    /// Restaura um backup do banco de dados (alias para `finctl restore`)
    Restore(RestoreArgs),
}

#[derive(Args, Debug, Clone)]
pub struct InitArgs {
    /// Inicializa o banco sem criar as categorias padrão
    #[arg(long = "no-seed")]
    pub no_seed: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum AccountCommands {
    /// Adiciona uma nova conta
    Add {
        /// Nome da conta (ex: "Nubank", "Carteira")
        name: String,

        /// Tipo da conta: checking (corrente), savings (poupança), wallet (carteira), investment (investimento), credit_card (cartão de crédito)
        #[arg(short, long)]
        kind: String,

        /// Saldo inicial da conta (ex: 1500,00 ou 1500.00)
        #[arg(short, long, default_value = "0,00")]
        initial_balance: String,

        /// Dia de fechamento da fatura (1-31, para cartões de crédito)
        #[arg(long = "closing-day")]
        closing_day: Option<u8>,

        /// Dia de vencimento da fatura (1-31, para cartões de crédito)
        #[arg(long = "due-day")]
        due_day: Option<u8>,

        /// Limite de crédito (para cartões de crédito)
        #[arg(long = "credit-limit", alias = "limit")]
        credit_limit: Option<String>,
    },

    /// Lista as contas cadastradas
    List {
        /// Formato de saída (table, json, csv)
        #[arg(short, long, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },

    /// Remove uma conta (apenas se não houver lançamentos ativos vinculados)
    Rm {
        /// Nome ou ID da conta a ser removida
        account: String,

        /// Pular a confirmação interativa
        #[arg(short = 'y', long = "yes")]
        yes: bool,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum CategoryCommands {
    /// Adiciona uma nova categoria
    Add {
        /// Nome da categoria (ex: "Alimentação", "Mercado")
        name: String,

        /// Tipo da categoria: income (receita) ou expense (despesa)
        #[arg(short, long)]
        kind: String,

        /// Nome ou ID da categoria pai (opcional)
        #[arg(short, long)]
        parent: Option<String>,
    },

    /// Lista as categorias cadastradas
    List {
        /// Formato de saída (table, json, csv)
        #[arg(short, long, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },

    /// Remove uma categoria (apenas se não houver lançamentos ativos vinculados)
    Rm {
        /// Nome ou ID da categoria a ser removida
        category: String,

        /// Pular a confirmação interativa
        #[arg(short = 'y', long = "yes")]
        yes: bool,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum IncomeCommands {
    /// Registra uma nova receita
    Add {
        /// Nome ou ID da conta
        #[arg(short, long)]
        account: String,

        /// Nome ou ID da categoria de receita
        #[arg(short, long)]
        category: String,

        /// Valor da receita (ex: 3500,00 ou 3500.00)
        #[arg(short = 'm', long = "amount")]
        amount: String,

        /// Data da receita no formato AAAA-MM-DD (padrão: hoje)
        #[arg(short, long)]
        date: Option<String>,

        /// Descrição do lançamento
        #[arg(long = "desc", default_value = "")]
        description: String,

        /// Registra a receita como prevista (pendente de realização)
        #[arg(long)]
        pending: bool,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum ExpenseCommands {
    /// Registra uma nova despesa ou compra parcelada
    Add {
        /// Nome ou ID da conta
        #[arg(short, long)]
        account: String,

        /// Nome ou ID da categoria de despesa
        #[arg(short, long)]
        category: String,

        /// Valor total da despesa (ex: 89,90 ou 89.90)
        #[arg(short = 'm', long = "amount")]
        amount: Option<String>,

        /// Valor de cada parcela (alternativa ao valor total ao usar --installments)
        #[arg(long = "installment-amount")]
        installment_amount: Option<String>,

        /// Número de parcelas (para compras parceladas, mínimo: 2)
        #[arg(short = 'i', long = "installments")]
        installments: Option<u32>,

        /// Data da despesa / primeira parcela no formato AAAA-MM-DD (padrão: hoje)
        #[arg(short, long)]
        date: Option<String>,

        /// Descrição do lançamento
        #[arg(long = "desc", default_value = "")]
        description: String,

        /// Registra a despesa avulsa como prevista (pendente de realização)
        #[arg(long)]
        pending: bool,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum TransferCommands {
    /// Registra uma transferência entre contas
    Add {
        /// Conta de origem (nome ou ID)
        #[arg(long = "from")]
        from: String,

        /// Conta de destino (nome ou ID)
        #[arg(long = "to")]
        to: String,

        /// Valor da transferência (ex: 150.00 ou 150,00)
        #[arg(short = 'm', long = "amount")]
        amount: String,

        /// Data da transferência no formato AAAA-MM-DD (padrão: hoje)
        #[arg(short, long)]
        date: Option<String>,

        /// Descrição personalizada opcional
        #[arg(long = "desc")]
        description: Option<String>,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum CardCommands {
    /// Gerenciamento de faturas de cartão de crédito
    Invoice {
        #[command(subcommand)]
        subcommand: InvoiceCommands,
    },

    /// Realiza o pagamento de uma fatura de cartão de crédito
    Pay {
        /// Nome ou ID do cartão de crédito
        card: String,

        /// Conta bancária pagadora de onde sairá o dinheiro
        #[arg(long = "from")]
        from: String,

        /// Mês da fatura a pagar (AAAA-MM). Se omitido, paga a fatura fechada mais antiga
        #[arg(short, long)]
        month: Option<String>,

        /// Valor do pagamento (se omitido, paga o saldo total restante da fatura)
        #[arg(short = 'a', long = "amount")]
        amount: Option<String>,

        /// Data do pagamento no formato AAAA-MM-DD (padrão: hoje)
        #[arg(short, long)]
        date: Option<String>,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum InvoiceCommands {
    /// Lista as faturas de um cartão de crédito
    List {
        /// Nome ou ID do cartão de crédito
        card: String,

        /// Formato de saída (table, json, csv)
        #[arg(short, long, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },

    /// Exibe os detalhes e lançamentos de uma fatura
    Show {
        /// Nome ou ID do cartão de crédito
        card: String,

        /// Mês da fatura no formato AAAA-MM (padrão: fatura aberta atual)
        #[arg(short, long)]
        month: Option<String>,

        /// Formato de saída (table, json, csv)
        #[arg(short, long, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },

    /// Fecha uma fatura de cartão de crédito
    Close {
        /// Nome ou ID do cartão de crédito
        card: String,

        /// Mês da fatura no formato AAAA-MM (padrão: fatura aberta mais antiga)
        #[arg(short, long)]
        month: Option<String>,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum TxCommands {
    /// Lista lançamentos com filtros
    List {
        /// Data inicial no formato AAAA-MM-DD
        #[arg(long = "from")]
        from_date: Option<String>,

        /// Data final no formato AAAA-MM-DD
        #[arg(long = "to")]
        to_date: Option<String>,

        /// Mês de referência no formato AAAA-MM (ex: 2026-10)
        #[arg(short, long)]
        month: Option<String>,

        /// Filtrar por nome ou ID da conta
        #[arg(short, long)]
        account: Option<String>,

        /// Filtrar por nome ou ID da categoria
        #[arg(short, long)]
        category: Option<String>,

        /// Filtrar por tipo: income (receita) ou expense (despesa)
        #[arg(short, long)]
        kind: Option<String>,

        /// Filtrar por status: paid (realizado) ou pending (previsto)
        #[arg(long)]
        status: Option<String>,

        /// Filtrar por ID do grupo de parcelamento
        #[arg(long = "group")]
        group: Option<String>,

        /// Filtrar por tag
        #[arg(long)]
        tag: Option<String>,

        /// Limitar o número de registros exibidos
        #[arg(short, long)]
        limit: Option<i64>,

        /// Exibir apenas lançamentos excluídos (soft delete)
        #[arg(long = "deleted")]
        deleted: bool,

        /// Formato de saída (table, json, csv)
        #[arg(short, long, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },

    /// Restaura um lançamento ou parcelamento excluído
    Restore {
        /// ID do lançamento
        id: String,

        /// Restaurar todo o grupo de parcelamento ao qual o lançamento pertence
        #[arg(long = "group")]
        group: bool,
    },

    /// Associa tags a um lançamento
    Tag {
        /// ID do lançamento
        id: String,

        /// Tags a serem associadas (ex: viagem trabalho)
        #[arg(required = true, num_args = 1..)]
        tags: Vec<String>,
    },

    /// Vincula um arquivo local ou URL externa como anexo ao lançamento
    Attach {
        /// ID do lançamento
        id: String,

        /// Caminho do arquivo local ou URL externa
        path_or_uri: String,

        /// Nota descritiva do anexo (opcional)
        #[arg(short, long)]
        note: Option<String>,
    },

    /// Marca um lançamento previsto (pendente) como realizado (pago)
    Pay {
        /// ID do lançamento
        id: String,

        /// Nova data de realização no formato AAAA-MM-DD (opcional; padrão: mantém a data original)
        #[arg(short, long)]
        date: Option<String>,
    },

    /// Edita os campos de um lançamento existente ou de um grupo de parcelamento
    Edit {
        /// ID do lançamento (opcional se --group for fornecido)
        id: Option<String>,

        /// ID do grupo de parcelamento (edita apenas as parcelas ainda pendentes)
        #[arg(long = "group")]
        group: Option<String>,

        /// Novo nome ou ID da conta
        #[arg(short, long)]
        account: Option<String>,

        /// Novo nome ou ID da categoria
        #[arg(short, long)]
        category: Option<String>,

        /// Novo valor monetário (apenas para edição individual de lançamento)
        #[arg(short = 'm', long = "amount")]
        amount: Option<String>,

        /// Nova data no formato AAAA-MM-DD (apenas para edição individual de lançamento)
        #[arg(short, long)]
        date: Option<String>,

        /// Nova descrição do lançamento
        #[arg(long = "desc")]
        description: Option<String>,
    },

    /// Remove um lançamento ou parcelas pendentes de um grupo de parcelamento
    Rm {
        /// ID do lançamento (opcional se --group for fornecido)
        id: Option<String>,

        /// ID do grupo de parcelamento (remove apenas as parcelas ainda pendentes)
        #[arg(long = "group")]
        group: Option<String>,

        /// Pular a confirmação interativa
        #[arg(short = 'y', long = "yes")]
        yes: bool,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum BudgetCommands {
    /// Define ou atualiza o orçamento de uma categoria
    Set(BudgetSetArgs),

    /// Lista os orçamentos cadastrados
    List(BudgetListArgs),

    /// Exibe o status de consumo dos orçamentos para um determinado mês
    Status(BudgetStatusArgs),
}

#[derive(Args, Debug, Clone)]
pub struct BudgetSetArgs {
    /// Nome ou UUID da categoria de despesa
    pub category: String,

    /// Limite de gastos da categoria (ex: 1500,00 ou 1500.00)
    pub amount: String,

    /// Mês de referência no formato AAAA-MM (opcional; sem este parâmetro, o orçamento é padrão recorrente)
    #[arg(short, long)]
    pub month: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct BudgetListArgs {
    /// Filtrar por mês de referência no formato AAAA-MM
    #[arg(short, long)]
    pub month: Option<String>,

    /// Formato de saída (table, json, csv)
    #[arg(short, long, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

#[derive(Args, Debug, Clone)]
pub struct BudgetStatusArgs {
    /// Mês de referência no formato AAAA-MM (padrão: mês atual)
    #[arg(short, long)]
    pub month: Option<String>,

    /// Formato de saída (table, json, csv)
    #[arg(short, long, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

#[derive(Subcommand, Debug, Clone)]
pub enum RecurringCommands {
    /// Cadastra uma nova regra de recorrência
    Add(RecurringAddArgs),

    /// Lista as regras de recorrência cadastradas
    List(RecurringListArgs),

    /// Edita uma regra de recorrência existente
    Edit(RecurringEditArgs),

    /// Pausa uma regra de recorrência
    Pause(RecurringIdArg),

    /// Retoma uma regra de recorrência pausada
    Resume(RecurringIdArg),

    /// Remove uma regra de recorrência (não apaga lançamentos já gerados)
    Rm(RecurringRmArgs),

    /// Processa as regras de recorrência e gera os lançamentos pendentes
    Run(RecurringRunArgs),
}

#[derive(Args, Debug, Clone)]
pub struct RecurringAddArgs {
    /// Nome ou ID da conta
    #[arg(short, long)]
    pub account: String,

    /// Nome ou ID da categoria
    #[arg(short, long)]
    pub category: String,

    /// Valor monetário da recorrência (ex: 2500,00 ou 2500.00)
    #[arg(short = 'm', long = "amount")]
    pub amount: String,

    /// Tipo do lançamento: expense (despesa) ou income (receita) [padrão: expense]
    #[arg(short = 'k', long, default_value = "expense")]
    pub kind: String,

    /// Frequência da recorrência: weekly (semanal), monthly (mensal), yearly (anual) [padrão: monthly]
    #[arg(short, long, default_value = "monthly")]
    pub frequency: String,

    /// Dia do mês (1-31) ou dia da semana (1-7 para semanal)
    #[arg(short, long)]
    pub day: Option<u32>,

    /// Descrição da recorrência
    #[arg(long = "desc", default_value = "")]
    pub description: String,

    /// Data de início no formato AAAA-MM-DD (padrão: hoje)
    #[arg(long = "start")]
    pub start_date: Option<String>,

    /// Data de término no formato AAAA-MM-DD (opcional)
    #[arg(long = "end")]
    pub end_date: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct RecurringListArgs {
    /// Exibe também as regras pausadas/inativas
    #[arg(long)]
    pub all: bool,

    /// Formato de saída (table, json, csv)
    #[arg(short, long, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

#[derive(Args, Debug, Clone)]
pub struct RecurringEditArgs {
    /// ID da regra de recorrência
    pub id: String,

    /// Novo nome ou ID da conta
    #[arg(short, long)]
    pub account: Option<String>,

    /// Novo nome ou ID da categoria
    #[arg(short, long)]
    pub category: Option<String>,

    /// Novo valor monetário
    #[arg(short = 'm', long = "amount")]
    pub amount: Option<String>,

    /// Nova descrição
    #[arg(long = "desc")]
    pub description: Option<String>,

    /// Nova data de término no formato AAAA-MM-DD
    #[arg(long = "end")]
    pub end_date: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct RecurringIdArg {
    /// ID da regra de recorrência
    pub id: String,
}

#[derive(Args, Debug, Clone)]
pub struct RecurringRmArgs {
    /// ID da regra de recorrência
    pub id: String,

    /// Pular confirmação interativa
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,
}

#[derive(Args, Debug, Clone)]
pub struct RecurringRunArgs {
    /// Data limite para geração dos lançamentos no formato AAAA-MM-DD (padrão: hoje)
    #[arg(long)]
    pub until: Option<String>,

    /// Executa em modo de simulação sem salvar alterações no banco
    #[arg(long)]
    pub dry_run: bool,

    /// Formato de saída (table, json, csv)
    #[arg(short, long, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

#[derive(Args, Debug, Clone)]
pub struct BalanceArgs {
    /// Data limite para cálculo do saldo no formato AAAA-MM-DD (ex: 2026-10-01)
    #[arg(short, long)]
    pub at: Option<String>,

    /// Inclui lançamentos previstos (pendentes) no cálculo do saldo projetado
    #[arg(long)]
    pub projected: bool,

    /// Formato de saída (table, json, csv)
    #[arg(short, long, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ReportCommands {
    /// Relatório mensal consolidado de receitas, despesas e economia
    Monthly(MonthlyArgs),

    /// Relatório de gastos ou receitas agrupados por categoria
    Categories(CategoriesArgs),

    /// Comparativo de categorias entre meses com variação absoluta e percentual
    Compare(CompareArgs),
}

#[derive(Args, Debug, Clone)]
pub struct MonthlyArgs {
    /// Mês de referência no formato AAAA-MM (ex: 2026-10) [padrão: mês atual]
    #[arg(short, long)]
    pub month: Option<String>,

    /// Ano completo para visualização mês a mês (ex: 2026)
    #[arg(short, long)]
    pub year: Option<i32>,

    /// Filtrar por nome ou UUID da conta
    #[arg(short, long)]
    pub account: Option<String>,

    /// Incluir lançamentos previstos/pendentes no relatório
    #[arg(long)]
    pub include_pending: bool,

    /// Formato de saída dos dados
    #[arg(short, long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

#[derive(Args, Debug, Clone)]
pub struct CategoriesArgs {
    /// Mês de referência no formato AAAA-MM (ex: 2026-10) [padrão: mês atual]
    #[arg(short, long)]
    pub month: Option<String>,

    /// Filtrar por tipo de transação (income/receita ou expense/despesa)
    #[arg(short, long)]
    pub kind: Option<TransactionKind>,

    /// Profundidade da agregação: 1 (agrupa subcategorias na pai) ou 2 (detalha por subcategoria) [padrão: 2]
    #[arg(short, long, default_value_t = 2)]
    pub depth: u32,

    /// Filtrar por nome ou UUID da conta
    #[arg(short, long)]
    pub account: Option<String>,

    /// Incluir lançamentos previstos/pendentes no relatório
    #[arg(long)]
    pub include_pending: bool,

    /// Filtrar lançamentos associados a uma tag específica
    #[arg(long)]
    pub tag: Option<String>,

    /// Formato de saída dos dados
    #[arg(short, long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

#[derive(Args, Debug, Clone)]
pub struct CompareArgs {
    /// Meses para comparar separados por vírgula (ex: 2026-08,2026-09)
    #[arg(short, long, value_delimiter = ',')]
    pub months: Option<Vec<String>>,

    /// Quantidade dos últimos N meses para comparar (ex: 2, 3, 6)
    #[arg(short, long)]
    pub last: Option<u32>,

    /// Filtrar por tipo de transação (income/receita ou expense/despesa)
    #[arg(short, long)]
    pub kind: Option<TransactionKind>,

    /// Filtrar por nome ou UUID da conta
    #[arg(short, long)]
    pub account: Option<String>,

    /// Incluir lançamentos previstos/pendentes no relatório
    #[arg(long)]
    pub include_pending: bool,

    /// Formato de saída dos dados
    #[arg(short, long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ExportCommands {
    /// Exporta lançamentos/transações para arquivo
    Tx(ExportTxArgs),
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ExportFormat {
    #[default]
    Csv,
    Json,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ExportLocale {
    #[default]
    #[value(name = "pt-BR", alias = "pt_BR", alias = "pt")]
    PtBr,
    #[value(name = "en-US", alias = "en_US", alias = "en")]
    EnUs,
}

#[derive(Args, Debug, Clone)]
pub struct ExportTxArgs {
    /// Caminho do arquivo de saída
    #[arg(short, long)]
    pub output: String,

    /// Formato do arquivo (csv ou json) [padrão: csv]
    #[arg(short, long, value_enum, default_value_t = ExportFormat::Csv)]
    pub format: ExportFormat,

    /// Padrão de formatação regional (pt-BR com ';' e vírgula decimal, en-US com ',' e ponto) [padrão: pt-BR]
    #[arg(short, long, value_enum, default_value_t = ExportLocale::PtBr)]
    pub locale: ExportLocale,

    /// Sobrescrever arquivo caso já exista
    #[arg(long)]
    pub force: bool,

    /// Data inicial no formato AAAA-MM-DD
    #[arg(long)]
    pub from: Option<chrono::NaiveDate>,

    /// Data final no formato AAAA-MM-DD
    #[arg(long)]
    pub to: Option<chrono::NaiveDate>,

    /// Mês específico no formato AAAA-MM (ex: 2026-10)
    #[arg(short, long)]
    pub month: Option<String>,

    /// Filtrar por nome ou UUID da conta
    #[arg(short, long)]
    pub account: Option<String>,

    /// Filtrar por nome ou UUID da categoria
    #[arg(short, long)]
    pub category: Option<String>,

    /// Filtrar por tipo (income/receita ou expense/despesa)
    #[arg(short, long)]
    pub kind: Option<TransactionKind>,

    /// Limite máximo de registros para exportar
    #[arg(long)]
    pub limit: Option<i64>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ImportCommands {
    /// Importa lançamentos a partir de um arquivo CSV
    Csv(ImportCsvArgs),
}

#[derive(Args, Debug, Clone)]
pub struct ImportCsvArgs {
    /// Caminho do arquivo CSV para importação
    pub file: String,

    /// Nome ou UUID da conta bancária de destino
    #[arg(short, long)]
    pub account: String,

    /// Nome do perfil de importação (ex: 'generic', 'nubank') [padrão: generic]
    #[arg(short, long)]
    pub profile: Option<String>,

    /// Simula a importação sem gravar alterações no banco de dados
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug, Clone)]
pub struct ReconcileArgs {
    #[command(subcommand)]
    pub command: Option<ReconcileSubcommands>,

    /// Nome ou UUID da conta bancária para conciliação
    #[arg(short, long)]
    pub account: Option<String>,

    /// Caminho do arquivo CSV de extrato bancário
    #[arg(short, long)]
    pub file: Option<String>,

    /// Perfil de parsing do CSV (ex: 'generic', 'nubank')
    #[arg(short, long)]
    pub profile: Option<String>,

    /// Janela de tolerância em dias para a data (padrão: 3)
    #[arg(short, long, default_value = "3")]
    pub days: i64,

    /// Aplica as conciliações diretamente sem pedir confirmação interativa
    #[arg(short, long)]
    pub yes: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ReconcileSubcommands {
    /// Exibe os lançamentos pendentes de conciliação no sistema
    Status(ReconcileStatusArgs),
}

#[derive(Args, Debug, Clone)]
pub struct ReconcileStatusArgs {
    /// Filtrar por nome ou UUID da conta
    #[arg(short, long)]
    pub account: Option<String>,

    /// Formato de saída (table, json, csv)
    #[arg(long, default_value = "table")]
    pub format: OutputFormat,
}

#[derive(Args, Debug, Clone)]
pub struct PurgeArgs {
    /// Tempo limite para expurgo definitivo de registros excluídos (ex: 30d, 60d, 90d, 6m, 1y)
    #[arg(long = "older-than")]
    pub older_than: String,

    /// Pular confirmação interativa
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum TagCommands {
    /// Cria uma nova tag
    Add(AddTagArgs),

    /// Lista todas as tags cadastradas e a quantidade de lançamentos vinculados
    List(ListTagArgs),

    /// Remove uma tag existente
    Rm(RmTagArgs),
}

#[derive(Args, Debug, Clone)]
pub struct AddTagArgs {
    /// Nome da tag (ex: 'viagem', 'reforma')
    pub name: String,
}

#[derive(Args, Debug, Clone)]
pub struct ListTagArgs {
    /// Formato de saída (table, json, csv)
    #[arg(short, long, default_value = "table")]
    pub format: OutputFormat,
}

#[derive(Args, Debug, Clone)]
pub struct RmTagArgs {
    /// Nome ou UUID da tag a ser removida
    pub name: String,

    /// Força a exclusão sem solicitar confirmação, mesmo que esteja em uso
    #[arg(short = 'y', long = "yes", alias = "force")]
    pub yes: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum AuditCommands {
    /// Lista os registros de auditoria capturados pelo sistema
    List(AuditListArgs),
}

#[derive(Args, Debug, Clone)]
pub struct AuditListArgs {
    /// Filtrar por nome da tabela (transactions, accounts, categories, budgets)
    #[arg(short, long)]
    pub table: Option<String>,

    /// Filtrar por ID do registro auditado (UUID)
    #[arg(long)]
    pub id: Option<Uuid>,

    /// Filtrar registros a partir de uma data (YYYY-MM-DD ou RFC3339)
    #[arg(short, long)]
    pub since: Option<String>,

    /// Limite de registros retornados
    #[arg(short, long, default_value = "50")]
    pub limit: Option<i64>,

    /// Formato de saída (table, json, csv)
    #[arg(short, long, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

#[derive(Args, Debug, Clone)]
pub struct BackupArgs {
    /// Diretório onde o arquivo de backup (.dump) será salvo
    #[arg(short, long)]
    pub output: PathBuf,

    /// Quantidade de backups mais recentes a manter (remove os excedentes mais antigos)
    #[arg(short, long)]
    pub keep: Option<usize>,
}

#[derive(Args, Debug, Clone)]
pub struct RestoreArgs {
    /// Caminho do arquivo de backup (.dump) a ser restaurado
    pub file: PathBuf,

    /// Nome do banco de dados de destino (se omitido, restaura em um novo banco gerado automaticamente)
    #[arg(short, long)]
    pub into: Option<String>,

    /// Confirmação expressa para restaurar sobre o banco de dados atual
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,
}

#[derive(Args, Debug, Clone)]
pub struct CompletionsArgs {
    /// Shell para o qual gerar o script de autocompletar (bash, zsh, fish, powershell, elvish)
    #[arg(value_enum)]
    pub shell: Shell,
}

#[derive(Args, Debug, Clone)]
pub struct ManArgs {
    /// Diretório onde as páginas de manual serão gravadas. Se omitido, imprime a página principal na saída padrão (stdout)
    #[arg(short, long)]
    pub dir: Option<PathBuf>,
}

#[derive(Args, Debug, Clone, Default)]
pub struct TuiArgs {
    /// Intervalo de atualização periódica em milissegundos
    #[arg(long, default_value = "250")]
    pub tick_rate: u64,

    /// Tema visual da interface interativa (dark | light)
    #[arg(long, default_value = "dark", value_parser = ["dark", "light"])]
    pub theme: String,
}
