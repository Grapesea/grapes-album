use std::collections::HashMap;

use base64::Engine;
use serde_json::{json, Value};

use crate::error::{GrapesError, Result};
use crate::types::SemanticProviderConfig;

const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(90);

#[derive(Debug, Clone)]
pub struct RerankCandidate {
    pub asset_id: String,
    pub text: String,
    pub image_jpeg: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct RerankOutcome {
    pub scores: HashMap<String, f64>,
    pub mode: String,
}

pub struct SemanticProviderClient {
    http: reqwest::Client,
}

impl SemanticProviderClient {
    pub fn new() -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|error| GrapesError::Network(error.to_string()))?;
        Ok(Self { http })
    }

    pub async fn embed_text(
        &self,
        config: &SemanticProviderConfig,
        api_key: &str,
        text: &str,
    ) -> Result<Vec<f32>> {
        let response = self
            .http
            .post(endpoint(&config.base_url, "embeddings"))
            .bearer_auth(api_key)
            .json(&json!({
                "model": config.model,
                "input": text,
            }))
            .send()
            .await
            .map_err(network_error)?;
        parse_embedding_response(response).await
    }

    pub async fn embed_image(
        &self,
        config: &SemanticProviderConfig,
        api_key: &str,
        jpeg: &[u8],
    ) -> Result<Vec<f32>> {
        let response = self
            .http
            .post(endpoint(&config.base_url, "embeddings"))
            .bearer_auth(api_key)
            .json(&json!({
                "model": config.model,
                "input": { "image": data_url(jpeg) },
            }))
            .send()
            .await
            .map_err(network_error)?;
        parse_embedding_response(response).await
    }

    pub async fn rerank(
        &self,
        config: &SemanticProviderConfig,
        api_key: &str,
        query: &str,
        candidates: &[RerankCandidate],
    ) -> Result<RerankOutcome> {
        let multimodal_documents = candidates
            .iter()
            .map(|candidate| {
                json!({
                    "text": candidate.text,
                    "image_url": candidate.image_jpeg.as_deref().map(data_url),
                })
            })
            .collect::<Vec<_>>();
        let multimodal = self
            .http
            .post(endpoint(&config.base_url, "rerank"))
            .bearer_auth(api_key)
            .json(&json!({
                "model": config.model,
                "query": query,
                "documents": multimodal_documents,
                "top_n": candidates.len(),
                "return_documents": false,
            }))
            .send()
            .await
            .map_err(network_error)?;
        if multimodal.status().is_success() {
            return parse_rerank_response(multimodal, candidates, "multimodal").await;
        }

        let text_documents = candidates
            .iter()
            .map(|candidate| candidate.text.clone())
            .collect::<Vec<_>>();
        let text_only = self
            .http
            .post(endpoint(&config.base_url, "rerank"))
            .bearer_auth(api_key)
            .json(&json!({
                "model": config.model,
                "query": query,
                "documents": text_documents,
                "top_n": candidates.len(),
                "return_documents": false,
            }))
            .send()
            .await
            .map_err(network_error)?;
        parse_rerank_response(text_only, candidates, "text").await
    }
}

async fn parse_embedding_response(response: reqwest::Response) -> Result<Vec<f32>> {
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| GrapesError::Network(error.to_string()))?;
    if !status.is_success() {
        return Err(provider_error(status.as_u16(), &body));
    }
    let value: Value = serde_json::from_str(&body)
        .map_err(|error| GrapesError::Provider(format!("无法解析向量响应：{error}")))?;
    let embedding = value
        .get("data")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("embedding"))
        .or_else(|| {
            value
                .get("embeddings")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
        })
        .or_else(|| {
            value
                .get("output")
                .and_then(|output| output.get("embeddings"))
        })
        .and_then(Value::as_array)
        .ok_or_else(|| GrapesError::Provider("响应中缺少 embedding 数组".into()))?;
    let vector = embedding
        .iter()
        .map(|component| {
            component
                .as_f64()
                .filter(|value| value.is_finite())
                .map(|value| value as f32)
                .ok_or_else(|| GrapesError::Provider("embedding 包含无效数值".into()))
        })
        .collect::<Result<Vec<_>>>()?;
    normalize_vector(vector)
}

async fn parse_rerank_response(
    response: reqwest::Response,
    candidates: &[RerankCandidate],
    mode: &str,
) -> Result<RerankOutcome> {
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| GrapesError::Network(error.to_string()))?;
    if !status.is_success() {
        return Err(provider_error(status.as_u16(), &body));
    }
    let value: Value = serde_json::from_str(&body)
        .map_err(|error| GrapesError::Provider(format!("无法解析 Rerank 响应：{error}")))?;
    let results = value
        .get("results")
        .and_then(Value::as_array)
        .ok_or_else(|| GrapesError::Provider("响应中缺少 Rerank 结果".into()))?;
    let mut scores = HashMap::new();
    for result in results {
        let index = result
            .get("index")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .ok_or_else(|| GrapesError::Provider("Rerank 结果缺少 index".into()))?;
        let candidate = candidates
            .get(index)
            .ok_or_else(|| GrapesError::Provider(format!("Rerank 返回了无效候选索引 {index}")))?;
        let score = result
            .get("relevance_score")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite())
            .ok_or_else(|| GrapesError::Provider("Rerank 结果缺少 relevance_score".into()))?;
        scores.insert(candidate.asset_id.clone(), score.clamp(0.0, 1.0));
    }
    Ok(RerankOutcome {
        scores,
        mode: mode.into(),
    })
}

pub fn normalize_vector(mut vector: Vec<f32>) -> Result<Vec<f32>> {
    if vector.is_empty() {
        return Err(GrapesError::Provider("embedding 不能为空".into()));
    }
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if !norm.is_finite() || norm <= f32::EPSILON {
        return Err(GrapesError::Provider("embedding 范数无效".into()));
    }
    for value in &mut vector {
        *value /= norm;
    }
    Ok(vector)
}

pub fn cosine_similarity(left: &[f32], right: &[f32]) -> Option<f32> {
    if left.len() != right.len() || left.is_empty() {
        return None;
    }
    let dot = left.iter().zip(right).map(|(a, b)| a * b).sum::<f32>();
    let left_norm = left.iter().map(|value| value * value).sum::<f32>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f32>().sqrt();
    if left_norm <= f32::EPSILON || right_norm <= f32::EPSILON {
        None
    } else {
        Some((dot / (left_norm * right_norm)).clamp(-1.0, 1.0))
    }
}

fn data_url(jpeg: &[u8]) -> String {
    format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(jpeg)
    )
}

fn endpoint(base_url: &str, suffix: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    if base.ends_with(&format!("/{suffix}")) {
        base.to_owned()
    } else {
        format!("{base}/{suffix}")
    }
}

fn network_error(error: reqwest::Error) -> GrapesError {
    GrapesError::Network(error.to_string())
}

fn provider_error(status: u16, body: &str) -> GrapesError {
    let detail = body.chars().take(600).collect::<String>();
    GrapesError::Provider(format!("HTTP {status}: {detail}"))
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;

    use super::*;

    #[test]
    fn normalizes_vectors_and_measures_cosine_similarity() {
        let vector = normalize_vector(vec![3.0, 4.0]).expect("normalized");
        assert!((vector[0] - 0.6).abs() < 1e-6);
        assert!((vector[1] - 0.8).abs() < 1e-6);
        assert!((cosine_similarity(&vector, &vector).unwrap() - 1.0).abs() < 1e-6);
        assert!(cosine_similarity(&vector, &[1.0]).is_none());
    }

    #[test]
    fn builds_openai_compatible_endpoints() {
        assert_eq!(
            endpoint("https://api.example.com/v1/", "embeddings"),
            "https://api.example.com/v1/embeddings"
        );
        assert_eq!(
            endpoint("https://api.example.com/v1/rerank", "rerank"),
            "https://api.example.com/v1/rerank"
        );
    }

    fn mock_response(body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("mock listener");
        let address = listener.local_addr().expect("mock address");
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("mock connection");
            let mut request = [0_u8; 8192];
            let _ = stream.read(&mut request);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("mock response");
        });
        format!("http://{address}/v1")
    }

    fn mock_captured_response(body: &'static str) -> (String, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("mock listener");
        let address = listener.local_addr().expect("mock address");
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("mock connection");
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .expect("mock read timeout");
            let mut request = Vec::new();
            let mut chunk = [0_u8; 4096];
            loop {
                let read = stream.read(&mut chunk).expect("mock request");
                assert!(read > 0, "request ended before its body was complete");
                request.extend_from_slice(&chunk[..read]);
                if let Some(header_end) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..header_end]);
                    let content_length = headers
                        .lines()
                        .filter_map(|line| line.split_once(':'))
                        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                        .map(|(_, value)| value.trim().parse::<usize>().expect("content length"))
                        .expect("content-length header");
                    if request.len() >= header_end + 4 + content_length {
                        break;
                    }
                }
            }
            sender
                .send(String::from_utf8(request).expect("UTF-8 request"))
                .expect("capture request");
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("mock response");
        });
        (format!("http://{address}/v1"), receiver)
    }

    #[tokio::test]
    async fn sends_vl_images_as_jpeg_data_urls() {
        let client = SemanticProviderClient::new().expect("client");
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut jpeg)
            .encode(&[255, 0, 0], 1, 1, image::ExtendedColorType::Rgb8)
            .expect("JPEG fixture");
        for provider_type in ["siliconflow", "custom"] {
            let (base_url, request) = mock_captured_response(r#"{"data":[{"embedding":[1,0]}]}"#);
            let config = SemanticProviderConfig {
                kind: "embedding".into(),
                provider_type: provider_type.into(),
                base_url,
                model: "Qwen/Qwen3-VL-Embedding-8B".into(),
                enabled: true,
                api_key: None,
            };
            client
                .embed_image(&config, "secret", &jpeg)
                .await
                .expect("image embedding");
            let request = request.recv().expect("captured request");
            assert!(request.starts_with("POST /v1/embeddings HTTP/1.1\r\n"));
            let (_, body) = request.split_once("\r\n\r\n").expect("request body");
            let body: Value = serde_json::from_str(body).expect("JSON request");
            assert_eq!(body["model"], config.model);
            let encoded = body["input"]["image"]
                .as_str()
                .expect("single image content object")
                .strip_prefix("data:image/jpeg;base64,")
                .expect("JPEG MIME type and base64 encoding");
            assert_eq!(
                base64::engine::general_purpose::STANDARD
                    .decode(encoded)
                    .expect("base64 image"),
                jpeg
            );
        }
    }

    #[tokio::test]
    async fn keeps_text_embedding_input_as_a_string() {
        let client = SemanticProviderClient::new().expect("client");
        let (base_url, request) = mock_captured_response(r#"{"data":[{"embedding":[1,0]}]}"#);
        let config = SemanticProviderConfig {
            kind: "embedding".into(),
            provider_type: "siliconflow".into(),
            base_url,
            model: "Qwen/Qwen3-VL-Embedding-8B".into(),
            enabled: true,
            api_key: None,
        };
        client
            .embed_text(&config, "secret", "红色的树")
            .await
            .expect("text embedding");
        let request = request.recv().expect("captured request");
        let (_, body) = request.split_once("\r\n\r\n").expect("request body");
        let body: Value = serde_json::from_str(body).expect("JSON request");
        assert_eq!(body["input"], "红色的树");
    }

    #[tokio::test]
    async fn parses_embedding_and_rerank_responses() {
        let client = SemanticProviderClient::new().expect("client");
        let config = SemanticProviderConfig {
            kind: "embedding".into(),
            provider_type: "custom".into(),
            base_url: mock_response(r#"{"data":[{"embedding":[3,4]}]}"#),
            model: "mock".into(),
            enabled: true,
            api_key: None,
        };
        let vector = client
            .embed_text(&config, "secret", "test")
            .await
            .expect("embedding response");
        assert!((vector[0] - 0.6).abs() < 1e-6);

        let rerank_config = SemanticProviderConfig {
            kind: "rerank".into(),
            base_url: mock_response(r#"{"results":[{"index":0,"relevance_score":0.87}]}"#),
            ..config
        };
        let outcome = client
            .rerank(
                &rerank_config,
                "secret",
                "red tree",
                &[RerankCandidate {
                    asset_id: "asset:1".into(),
                    text: "tree".into(),
                    image_jpeg: None,
                }],
            )
            .await
            .expect("rerank response");
        assert_eq!(outcome.mode, "multimodal");
        assert!((outcome.scores["asset:1"] - 0.87).abs() < 1e-6);
    }
}
