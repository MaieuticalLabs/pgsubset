use serde::Deserialize;
use std::collections::HashMap;

pub type TargetedTransforms = HashMap<String, HashMap<String, TransformKind>>;

#[derive(Deserialize)]
pub struct Config {
    pub target_table: String,
    pub target_dir: String,
    pub database_url: String,
    pub m2m_tables: Option<Vec<M2MTable>>,
    pub transforms: Option<TargetedTransforms>,
}

#[derive(Deserialize)]
pub struct M2MTable {
    pub name: String,
    pub source: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransformKind {
    ClearField,
    FirstNameEn,
    LastNameEn,
    UsernameEn,
    EmailEn,
    DjangoGarbagePassword,
}
