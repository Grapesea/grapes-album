use serde::Deserialize;

use crate::error::{GrapesError, Result};
use crate::types::{DimensionView, SearchPlan, SearchPlanFilter};

use super::{RerankCandidate, SemanticProviderClient};

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
struct RawSearchPlan {
    #[serde(alias = "hard_filters")]
    hard_filters: Vec<RawSearchPlanFilter>,
    #[serde(alias = "soft_filters")]
    soft_filters: Vec<RawSearchPlanFilter>,
    #[serde(alias = "semantic_queries")]
    semantic_queries: Vec<String>,
    exclusions: Vec<String>,
    #[serde(alias = "rerank_instruction")]
    rerank_instruction: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
struct RawSearchPlanFilter {
    #[serde(alias = "dimension_id", alias = "dimension")]
    dimension_id: String,
    #[serde(alias = "dimension_name")]
    dimension_name: String,
    #[serde(alias = "value_ids", alias = "values")]
    value_ids: Vec<String>,
    #[serde(alias = "value_names")]
    value_names: Vec<String>,
}

pub fn fallback_search_plan(query: &str) -> SearchPlan {
    SearchPlan {
        semantic_queries: vec![query.trim().to_owned()],
        ..SearchPlan::default()
    }
}

pub async fn create_search_plan(
    client: &SemanticProviderClient,
    config: &crate::types::SemanticProviderConfig,
    api_key: &str,
    query: &str,
    dimensions: &[DimensionView],
) -> Result<SearchPlan> {
    let system = plan_system_prompt(dimensions);
    let mut correction = String::new();
    let mut last_error = None;
    for _ in 0..2 {
        let user = if correction.is_empty() {
            format!("用户查询：{query}")
        } else {
            format!(
                "用户查询：{query}\n上一次输出校验失败：{correction}\n请重新输出严格合法的 JSON。"
            )
        };
        let content = client.chat(config, api_key, &system, &user, true).await?;
        match parse_search_plan(&content, dimensions, query) {
            Ok(plan) => return Ok(plan),
            Err(error) => {
                correction = error.to_string();
                last_error = Some(error);
            }
        }
    }
    Err(last_error.unwrap_or_else(|| GrapesError::Provider("Search Plan 校验失败".into())))
}

pub fn parse_search_plan(
    content: &str,
    dimensions: &[DimensionView],
    query: &str,
) -> Result<SearchPlan> {
    let json = strip_json_fence(content);
    let raw: RawSearchPlan = serde_json::from_str(json)
        .map_err(|error| GrapesError::Provider(format!("Search Plan 不是合法 JSON：{error}")))?;
    let hard_filters = normalize_filters(raw.hard_filters, dimensions)?;
    let soft_filters = normalize_filters(raw.soft_filters, dimensions)?;
    let mut semantic_queries = raw
        .semantic_queries
        .into_iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .take(4)
        .collect::<Vec<_>>();
    if semantic_queries.is_empty() {
        semantic_queries.push(query.trim().to_owned());
    }
    let exclusions = raw
        .exclusions
        .into_iter()
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .take(12)
        .collect();
    Ok(SearchPlan {
        hard_filters,
        soft_filters,
        semantic_queries,
        exclusions,
        rerank_instruction: raw
            .rerank_instruction
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty()),
    })
}

fn normalize_filters(
    filters: Vec<RawSearchPlanFilter>,
    dimensions: &[DimensionView],
) -> Result<Vec<SearchPlanFilter>> {
    let mut result = Vec::new();
    for filter in filters.into_iter().take(8) {
        let token = if filter.dimension_id.trim().is_empty() {
            filter.dimension_name.trim()
        } else {
            filter.dimension_id.trim()
        };
        let Some(dimension) = dimensions.iter().find(|dimension| {
            dimension.id.eq_ignore_ascii_case(token)
                || dimension.kind.eq_ignore_ascii_case(token)
                || dimension.name.eq_ignore_ascii_case(token)
        }) else {
            return Err(GrapesError::Provider(format!(
                "Search Plan 引用了不存在的分类维度 {token}"
            )));
        };
        let mut value_ids = Vec::new();
        let mut value_names = Vec::new();
        let requested = filter
            .value_ids
            .iter()
            .chain(filter.value_names.iter())
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .take(20);
        for token in requested {
            if let Some(value) = dimension.values.iter().find(|value| {
                value.id.eq_ignore_ascii_case(token) || value.name.eq_ignore_ascii_case(token)
            }) {
                if !value_ids.contains(&value.id) {
                    value_ids.push(value.id.clone());
                    value_names.push(value.name.clone());
                }
            }
        }
        if value_ids.is_empty() {
            return Err(GrapesError::Provider(format!(
                "Search Plan 的维度 {} 没有有效分类值",
                dimension.name
            )));
        }
        result.push(SearchPlanFilter {
            dimension_id: dimension.id.clone(),
            dimension_name: dimension.name.clone(),
            value_ids,
            value_names,
        });
    }
    Ok(result)
}

fn plan_system_prompt(dimensions: &[DimensionView]) -> String {
    let dimension_json = serde_json::to_string(
        &dimensions
            .iter()
            .map(|dimension| {
                serde_json::json!({
                    "dimensionId": dimension.id,
                    "dimensionName": dimension.name,
                    "kind": dimension.kind,
                    "values": dimension.values.iter().map(|value| {
                        serde_json::json!({
                            "valueId": value.id,
                            "valueName": value.name,
                        })
                    }).collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".into());
    format!(
        r#"你是 Grapes 相片检索查询规划器。只输出一个 JSON 对象，不要 Markdown。
JSON 必须符合以下结构：
{{
  "hardFilters": [{{"dimensionId": "必须来自可用维度", "valueIds": ["必须来自该维度"]}}],
  "softFilters": [{{"dimensionId": "...", "valueIds": ["..."]}}],
  "semanticQueries": ["用于多模态向量检索的简短视觉描述"],
  "exclusions": ["需要排除的视觉或分类词语"],
  "rerankInstruction": "对候选相片重新排序的简短说明"
}}
hardFilters 表示必须同时满足的分类条件；softFilters 表示加分条件。
不要把不确定条件放入 hardFilters。semanticQueries 最多 4 条。
可用分类维度和值：
{dimension_json}"#
    )
}

fn strip_json_fence(content: &str) -> &str {
    let trimmed = content.trim();
    let without_open = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed)
        .trim_start();
    without_open
        .strip_suffix("```")
        .unwrap_or(without_open)
        .trim()
}

pub fn plan_allows_asset(plan: &SearchPlan, asset: &crate::types::SemanticAssetRecord) -> bool {
    let excluded = plan
        .exclusions
        .iter()
        .any(|term| asset.combined_text.to_lowercase().contains(term));
    if excluded {
        return false;
    }
    plan.hard_filters.iter().all(|filter| {
        asset.classification_values.iter().any(|value| {
            value.dimension_id == filter.dimension_id && filter.value_ids.contains(&value.value_id)
        })
    })
}

pub fn soft_filter_score(plan: &SearchPlan, asset: &crate::types::SemanticAssetRecord) -> f64 {
    if plan.soft_filters.is_empty() {
        return 0.0;
    }
    let matched = plan
        .soft_filters
        .iter()
        .filter(|filter| {
            asset.classification_values.iter().any(|value| {
                value.dimension_id == filter.dimension_id
                    && filter.value_ids.contains(&value.value_id)
            })
        })
        .count();
    matched as f64 / plan.soft_filters.len() as f64
}

pub fn rerank_candidate_text(plan: &SearchPlan, candidate: &RerankCandidate) -> String {
    match plan.rerank_instruction.as_deref() {
        Some(instruction) if !instruction.is_empty() => {
            format!("{}。排序要求：{instruction}", candidate.text)
        }
        _ => candidate.text.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DimensionValueView, SemanticClassificationValue};

    fn dimensions() -> Vec<DimensionView> {
        vec![
            DimensionView {
                id: "dimension:weather".into(),
                name: "Weather".into(),
                kind: "weather".into(),
                sort_order: 10,
                system_defined: true,
                values: vec![
                    DimensionValueView {
                        id: "dimension:weather:rain".into(),
                        dimension_id: "dimension:weather".into(),
                        parent_id: None,
                        name: "Rain".into(),
                        description: None,
                        color: None,
                        sort_order: 10,
                    },
                    DimensionValueView {
                        id: "dimension:weather:sunny".into(),
                        dimension_id: "dimension:weather".into(),
                        parent_id: None,
                        name: "Sunny".into(),
                        description: None,
                        color: None,
                        sort_order: 20,
                    },
                ],
            },
            DimensionView {
                id: "dimension:tone".into(),
                name: "Tone".into(),
                kind: "tone".into(),
                sort_order: 20,
                system_defined: true,
                values: vec![DimensionValueView {
                    id: "dimension:tone:cool".into(),
                    dimension_id: "dimension:tone".into(),
                    parent_id: None,
                    name: "Cool".into(),
                    description: None,
                    color: None,
                    sort_order: 10,
                }],
            },
        ]
    }

    #[test]
    fn validates_and_resolves_search_plan_filters() {
        let plan = parse_search_plan(
            r#"```json
            {
              "hardFilters": [{"dimensionId": "weather", "valueIds": ["Rain"]}],
              "softFilters": [{"dimensionId": "Tone", "valueIds": ["dimension:tone:cool"]}],
              "semanticQueries": ["雨后街道"],
              "exclusions": ["sunny"],
              "rerankInstruction": "优先阴雨氛围"
            }
            ```"#,
            &dimensions(),
            "雨天冷色调",
        )
        .expect("valid plan");
        assert_eq!(plan.hard_filters[0].dimension_id, "dimension:weather");
        assert_eq!(plan.hard_filters[0].value_ids, ["dimension:weather:rain"]);
        assert_eq!(plan.soft_filters[0].value_ids, ["dimension:tone:cool"]);
        assert_eq!(plan.semantic_queries, ["雨后街道"]);
    }

    #[test]
    fn rejects_unknown_dimensions_and_values() {
        let error = parse_search_plan(
            r#"{"hardFilters":[{"dimensionId":"unknown","valueIds":["rain"]}]}"#,
            &dimensions(),
            "rain",
        )
        .expect_err("unknown dimension");
        assert!(error.to_string().contains("不存在的分类维度"));
    }

    #[test]
    fn hard_filters_and_exclusions_limit_assets() {
        let plan = parse_search_plan(
            r#"{
              "hardFilters": [{"dimensionId":"dimension:weather","valueIds":["dimension:weather:rain"]}],
              "exclusions": ["sunny"]
            }"#,
            &dimensions(),
            "rain",
        )
        .expect("plan");
        let matching = crate::types::SemanticAssetRecord {
            asset_id: "a".into(),
            filename: "rain.jpg".into(),
            relative_path: "rain.jpg".into(),
            combined_text: "rain photo".into(),
            classification_search_terms: vec!["Weather: Rain".into()],
            classification_values: vec![SemanticClassificationValue {
                dimension_id: "dimension:weather".into(),
                value_id: "dimension:weather:rain".into(),
            }],
            metadata_text: "rain.jpg".into(),
            thumbnail_path: None,
        };
        assert!(plan_allows_asset(&plan, &matching));
        let excluded = crate::types::SemanticAssetRecord {
            combined_text: "rain photo sunny".into(),
            ..matching
        };
        assert!(!plan_allows_asset(&plan, &excluded));
    }
}
