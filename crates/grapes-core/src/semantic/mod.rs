mod provider;

use std::collections::{BTreeMap, HashMap, HashSet};

use rusqlite::{params, Connection, OptionalExtension};

use crate::database::now;
use crate::error::{GrapesError, Result};
use crate::types::{
    SemanticAssetRecord, SemanticIndexStatus, SemanticMatchReason, SemanticProviderConfig,
    SemanticSearchResult,
};

pub use provider::{cosine_similarity, RerankCandidate, RerankOutcome, SemanticProviderClient};

pub fn load_provider_configs(conn: &Connection) -> Result<Vec<SemanticProviderConfig>> {
    let mut stmt = conn.prepare(
        "SELECT kind, provider_type, base_url, model, enabled, api_key
         FROM semantic_provider_configs
         ORDER BY CASE kind WHEN 'embedding' THEN 0 ELSE 1 END",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(SemanticProviderConfig {
            kind: row.get(0)?,
            provider_type: row.get(1)?,
            base_url: row.get(2)?,
            model: row.get(3)?,
            enabled: row.get::<_, i64>(4)? != 0,
            api_key: row.get(5)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn save_provider_config(conn: &Connection, config: &SemanticProviderConfig) -> Result<()> {
    if !matches!(config.kind.as_str(), "embedding" | "rerank") {
        return Err(GrapesError::InvalidPath(
            "Provider 类型必须是 embedding 或 rerank".into(),
        ));
    }
    if !matches!(config.provider_type.as_str(), "siliconflow" | "custom") {
        return Err(GrapesError::InvalidPath(
            "Provider 类型必须是 siliconflow 或 custom".into(),
        ));
    }
    let base_url = config.base_url.trim();
    let model = config.model.trim();
    if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
        return Err(GrapesError::InvalidPath(
            "Provider Base URL 必须使用 http:// 或 https://".into(),
        ));
    }
    if model.is_empty() {
        return Err(GrapesError::InvalidPath("Provider 模型不能为空".into()));
    }
    conn.execute(
        "INSERT INTO semantic_provider_configs(
             kind, provider_type, base_url, model, enabled, api_key, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(kind) DO UPDATE SET
             provider_type = excluded.provider_type,
             base_url = excluded.base_url,
             model = excluded.model,
             enabled = excluded.enabled,
             api_key = excluded.api_key,
             updated_at = excluded.updated_at",
        params![
            config.kind,
            config.provider_type,
            base_url,
            model,
            i64::from(config.enabled),
            config
                .api_key
                .as_deref()
                .map(str::trim)
                .filter(|key| !key.is_empty()),
            now()
        ],
    )?;
    Ok(())
}

pub fn load_ask_grapes_provider(conn: &Connection) -> Result<SemanticProviderConfig> {
    conn.query_row(
        "SELECT provider_type, base_url, model, enabled, api_key
         FROM ask_grapes_provider_configs WHERE id = 1",
        [],
        |row| {
            Ok(SemanticProviderConfig {
                kind: "qa".into(),
                provider_type: row.get(0)?,
                base_url: row.get(1)?,
                model: row.get(2)?,
                enabled: row.get::<_, i64>(3)? != 0,
                api_key: row.get(4)?,
            })
        },
    )
    .map_err(Into::into)
}

pub fn save_ask_grapes_provider(conn: &Connection, config: &SemanticProviderConfig) -> Result<()> {
    if config.kind != "qa" {
        return Err(GrapesError::InvalidPath(
            "Ask Grapes Provider 的错误类型".into(),
        ));
    }
    if !matches!(config.provider_type.as_str(), "siliconflow" | "custom") {
        return Err(GrapesError::InvalidPath(
            "Provider 类型必须是 siliconflow 或 custom".into(),
        ));
    }
    let base_url = config.base_url.trim();
    let model = config.model.trim();
    if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
        return Err(GrapesError::InvalidPath(
            "Provider Base URL 必须使用 http:// 或 https://".into(),
        ));
    }
    if model.is_empty() {
        return Err(GrapesError::InvalidPath("Provider 模型不能为空".into()));
    }
    conn.execute(
        "UPDATE ask_grapes_provider_configs
         SET provider_type = ?1,
             base_url = ?2,
             model = ?3,
             enabled = ?4,
             api_key = ?5,
             updated_at = ?6
         WHERE id = 1",
        params![
            config.provider_type,
            base_url,
            model,
            i64::from(config.enabled),
            config
                .api_key
                .as_deref()
                .map(str::trim)
                .filter(|key| !key.is_empty()),
            now()
        ],
    )?;
    Ok(())
}

pub fn config_hash(config: &SemanticProviderConfig) -> String {
    let provider_id = format!(
        "{}|{}|{}",
        config.provider_type.trim(),
        config.base_url.trim().trim_end_matches('/'),
        config.model.trim()
    );
    blake3::hash(provider_id.as_bytes()).to_hex().to_string()
}

pub fn model_id(config: &SemanticProviderConfig, dimension: usize) -> String {
    format!("embedding:{}:{dimension}", config_hash(config))
}

pub fn ensure_embedding_model(
    conn: &Connection,
    config: &SemanticProviderConfig,
    dimension: usize,
) -> Result<String> {
    let id = model_id(config, dimension);
    conn.execute(
        "INSERT OR IGNORE INTO embedding_models(
             id, provider_kind, provider_id, model, dimension, config_hash, created_at
         ) VALUES (?1, 'embedding', ?2, ?3, ?4, ?5, ?6)",
        params![
            id,
            config.provider_type,
            config.model,
            dimension as i64,
            config_hash(config),
            now()
        ],
    )?;
    Ok(id)
}

pub fn current_embedding_model(
    conn: &Connection,
    config: &SemanticProviderConfig,
) -> Result<Option<(String, usize)>> {
    let hash = config_hash(config);
    conn.query_row(
        "SELECT id, dimension
         FROM embedding_models
         WHERE provider_kind = 'embedding' AND provider_id = ?1 AND config_hash = ?2
         ORDER BY created_at DESC LIMIT 1",
        params![config.provider_type, hash],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize)),
    )
    .optional()
    .map_err(Into::into)
}

pub fn store_embedding(
    conn: &Connection,
    asset_id: &str,
    model_id: &str,
    vector: &[f32],
) -> Result<()> {
    if vector.is_empty() {
        return Err(GrapesError::Provider("embedding 不能为空".into()));
    }
    let bytes = vector
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    conn.execute(
        "INSERT INTO asset_embeddings(asset_id, model_id, vector, created_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(asset_id, model_id) DO UPDATE SET
             vector = excluded.vector,
             created_at = excluded.created_at",
        params![asset_id, model_id, bytes, now()],
    )?;
    Ok(())
}

pub fn load_embeddings(conn: &Connection, model_id: &str) -> Result<HashMap<String, Vec<f32>>> {
    let mut stmt =
        conn.prepare("SELECT asset_id, vector FROM asset_embeddings WHERE model_id = ?1")?;
    let rows = stmt.query_map([model_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
    })?;
    let mut embeddings = HashMap::new();
    for row in rows {
        let (asset_id, bytes) = row?;
        if bytes.len() % 4 != 0 {
            continue;
        }
        let vector = bytes
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect::<Vec<_>>();
        embeddings.insert(asset_id, vector);
    }
    Ok(embeddings)
}

pub fn latest_embeddings(conn: &Connection) -> Result<HashMap<String, Vec<f32>>> {
    let model_id = conn
        .query_row(
            "SELECT id FROM embedding_models
             WHERE provider_kind = 'embedding'
             ORDER BY rowid DESC LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    match model_id {
        Some(model_id) => load_embeddings(conn, &model_id),
        None => Ok(HashMap::new()),
    }
}

pub fn indexed_asset_ids(conn: &Connection, model_id: &str) -> Result<HashSet<String>> {
    let mut stmt = conn.prepare("SELECT asset_id FROM asset_embeddings WHERE model_id = ?1")?;
    let rows = stmt.query_map([model_id], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<std::result::Result<HashSet<_>, _>>()?)
}

pub fn index_status(
    conn: &Connection,
    total_assets: usize,
    config: &SemanticProviderConfig,
) -> Result<SemanticIndexStatus> {
    let current = current_embedding_model(conn, config)?;
    let (model_id, dimension, indexed_assets) = if let Some((id, dimension)) = current {
        let count = conn.query_row(
            "SELECT COUNT(*) FROM asset_embeddings WHERE model_id = ?1",
            [&id],
            |row| row.get::<_, i64>(0),
        )? as usize;
        (Some(id), Some(dimension), count)
    } else {
        (None, None, 0)
    };
    Ok(SemanticIndexStatus {
        total_assets,
        indexed_assets,
        pending_assets: total_assets.saturating_sub(indexed_assets),
        model_id,
        dimension,
    })
}

pub fn fuse_search_results(
    query: &str,
    assets: &[SemanticAssetRecord],
    embeddings: &HashMap<String, Vec<f32>>,
    query_embedding: &[f32],
    candidate_limit: usize,
) -> Vec<SemanticSearchResult> {
    let query_lower = query.trim().to_lowercase();
    let tokens = query_lower
        .split(|character: char| {
            character.is_whitespace()
                || matches!(character, ',' | '，' | '.' | '。' | ';' | '；' | ':' | '：')
        })
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let mut candidates = Vec::new();
    for asset in assets {
        let embedding_score = embeddings
            .get(&asset.asset_id)
            .and_then(|vector| cosine_similarity(query_embedding, vector))
            .map(|score| ((score + 1.0) / 2.0) as f64);
        let classification_matches = asset
            .classification_search_terms
            .iter()
            .filter(|label| text_matches(&query_lower, &tokens, label))
            .cloned()
            .collect::<Vec<_>>();
        let classification_score = if asset.classification_search_terms.is_empty() {
            0.0
        } else {
            classification_matches.len() as f64 / asset.classification_search_terms.len() as f64
        };
        let metadata_score = text_match_score(&query_lower, &tokens, &asset.metadata_text);
        let Some(embedding_score) = embedding_score else {
            if classification_score <= 0.0 && metadata_score <= 0.0 {
                continue;
            }
            let score = 0.55 * classification_score + 0.45 * metadata_score;
            candidates.push(result(
                asset,
                score,
                None,
                classification_score,
                metadata_score,
                classification_matches,
            ));
            continue;
        };
        let score = 0.72 * embedding_score + 0.2 * classification_score + 0.08 * metadata_score;
        candidates.push(result(
            asset,
            score,
            Some(embedding_score),
            classification_score,
            metadata_score,
            classification_matches,
        ));
    }
    candidates.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.filename.cmp(&right.filename))
    });
    candidates.truncate(candidate_limit);
    candidates
}

fn result(
    asset: &SemanticAssetRecord,
    score: f64,
    embedding_score: Option<f64>,
    classification_score: f64,
    metadata_score: f64,
    classification_matches: Vec<String>,
) -> SemanticSearchResult {
    let mut reasons = Vec::new();
    if let Some(embedding_score) = embedding_score {
        reasons.push(SemanticMatchReason {
            kind: "embedding".into(),
            label: String::new(),
            score: embedding_score,
        });
    }
    for label in classification_matches {
        reasons.push(SemanticMatchReason {
            kind: "classification".into(),
            label,
            score: classification_score,
        });
    }
    if metadata_score > 0.0 {
        reasons.push(SemanticMatchReason {
            kind: "metadata".into(),
            label: asset.relative_path.clone(),
            score: metadata_score,
        });
    }
    SemanticSearchResult {
        asset_id: asset.asset_id.clone(),
        filename: asset.filename.clone(),
        relative_path: asset.relative_path.clone(),
        score: score.clamp(0.0, 1.0),
        match_reasons: reasons,
    }
}

fn text_match_score(query: &str, tokens: &[&str], text: &str) -> f64 {
    let normalized = text.to_lowercase();
    if query.is_empty() {
        return 0.0;
    }
    if normalized.contains(query) {
        return 1.0;
    }
    if query.contains(&normalized) {
        return 1.0;
    }
    let matches = tokens
        .iter()
        .filter(|token| normalized.contains(**token))
        .count();
    matches as f64 / tokens.len().max(1) as f64
}

fn text_matches(query: &str, tokens: &[&str], text: &str) -> bool {
    let normalized = text.to_lowercase();
    normalized.contains(query)
        || query.contains(&normalized)
        || tokens.iter().any(|token| normalized.contains(*token))
}

pub fn classification_alias(dimension_kind: &str, value_id: &str) -> Option<&'static str> {
    let aliases: BTreeMap<(&str, &str), &str> = BTreeMap::from([
        (("time", "morning"), "晨曦"),
        (("time", "day"), "白天"),
        (("time", "golden-hour"), "黄金时刻"),
        (("time", "blue-hour"), "蓝调时刻"),
        (("time", "night"), "夜晚"),
        (("weather", "sunny"), "晴"),
        (("weather", "cloudy"), "多云"),
        (("weather", "overcast"), "阴"),
        (("weather", "rain"), "雨"),
        (("weather", "snow"), "雪"),
        (("weather", "fog"), "雾"),
        (("type", "landscape"), "风光"),
        (("type", "portrait"), "人像"),
        (("type", "documentary"), "纪实"),
        (("type", "street"), "街头"),
        (("type", "architecture"), "建筑"),
        (("type", "still-life"), "静物"),
        (("type", "wildlife"), "生态"),
        (("tone", "warm"), "暖色"),
        (("tone", "cool"), "冷色"),
        (("tone", "neutral"), "中性"),
        (("tone", "high-saturation"), "高饱和"),
        (("tone", "low-saturation"), "低饱和"),
        (("tone", "cinematic"), "电影感"),
    ]);
    let key = value_id.split(':').next_back().unwrap_or(value_id);
    aliases.get(&(dimension_kind, key)).copied()
}
