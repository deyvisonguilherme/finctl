use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum AuditAction {
    Insert,
    Update,
    Delete,
}

impl AuditAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Insert => "INSERT",
            Self::Update => "UPDATE",
            Self::Delete => "DELETE",
        }
    }
}

impl std::fmt::Display for AuditAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for AuditAction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "INSERT" => Ok(Self::Insert),
            "UPDATE" => Ok(Self::Update),
            "DELETE" => Ok(Self::Delete),
            other => Err(format!("Ação de auditoria inválida: {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: i64,
    pub table_name: String,
    pub row_id: Uuid,
    pub action: AuditAction,
    pub old: Option<serde_json::Value>,
    pub new: Option<serde_json::Value>,
    pub changed_at: DateTime<Utc>,
    pub actor: String,
}

impl AuditEntry {
    /// Produz um resumo legível das alterações entre old e new
    pub fn diff_summary(&self) -> String {
        match self.action {
            AuditAction::Insert => {
                if let Some(new_val) = &self.new {
                    if let Some(obj) = new_val.as_object() {
                        if let Some(desc) = obj.get("description").and_then(|v| v.as_str()) {
                            let amt = obj.get("amount").and_then(|v| v.as_str()).unwrap_or("");
                            return format!("+ {desc} ({amt})");
                        }
                        if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
                            return format!("+ {name}");
                        }
                    }
                }
                "+ novo registro".to_string()
            }
            AuditAction::Delete => "- registro excluído".to_string(),
            AuditAction::Update => {
                match (&self.old, &self.new) {
                    (
                        Some(serde_json::Value::Object(old_map)),
                        Some(serde_json::Value::Object(new_map)),
                    ) => {
                        // Detectar soft delete ou restore
                        let old_del = old_map
                            .get("deleted_at")
                            .unwrap_or(&serde_json::Value::Null);
                        let new_del = new_map
                            .get("deleted_at")
                            .unwrap_or(&serde_json::Value::Null);

                        if old_del.is_null() && !new_del.is_null() {
                            return "soft delete (removido)".to_string();
                        }
                        if !old_del.is_null() && new_del.is_null() {
                            return "restaurado".to_string();
                        }

                        let mut diffs = Vec::new();
                        for (k, v_new) in new_map {
                            if k == "updated_at" {
                                continue;
                            }
                            let v_old = old_map.get(k).unwrap_or(&serde_json::Value::Null);
                            if v_old != v_new {
                                diffs.push(format!("{k}: {v_old} -> {v_new}"));
                            }
                        }

                        if diffs.is_empty() {
                            "updated_at atualizado".to_string()
                        } else {
                            diffs.join(", ")
                        }
                    }
                    _ => "atualizado".to_string(),
                }
            }
        }
    }
}
