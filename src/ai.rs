use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde_json::{json, Value};
use std::env;
use std::time::Duration;

#[allow(dead_code)]
pub fn llm(system: &str, user: &str) -> anyhow::Result<String> {
    let api_key = env::var("OPENAI_API_KEY").expect("SECRET_KEY не установлена!");
    let model = env::var("CHAT_COMPLETIONS_MODEL").expect("CHAT_COMPLETIONS_MODEL не установлена!");
    let payload = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user }
        ],
        "temperature": 0.7,
        "max_tokens": 1000,
        "stream": false,
    });

    let url = env::var("CHAT_COMPLETIONS_URL").expect("CHAT_COMPLETIONS_URL не установлена!");
    let mut headers = HeaderMap::new();
    let auth_value = format!("Bearer {}", api_key);
    headers.insert(AUTHORIZATION, HeaderValue::from_str(&auth_value)?);
    let client = Client::builder().timeout(Duration::from_secs(60)).build()?;
    let response = client.post(url).headers(headers).json(&payload).send()?;
    let resp_json: Value = response.json()?;
    let content = resp_json["choices"][0]["message"]["content"]
        .as_str()
        .ok_or(anyhow::anyhow!("No content in response"))?;
    Ok(content.to_string())
}

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Embedding {
    object: String,
    embedding: Vec<f32>,
    index: usize,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct EmbeddingResponse {
    object: String,
    data: Vec<Embedding>,
    model: String,
    usage: serde_json::Value,
}

pub fn emb(input: &str) -> anyhow::Result<Vec<f32>> {
    let api_key = env::var("OPENAI_API_KEY").expect("SECRET_KEY не установлена!");

    let mut headers = HeaderMap::new();
    let auth_value = format!("Bearer {}", api_key);
    headers.insert(AUTHORIZATION, HeaderValue::from_str(&auth_value)?);
    let client = Client::builder().timeout(Duration::from_secs(60)).build()?;

    // lm-kit/text-embedding-bge-m3
    let model = env::var("EMBEDDINGS_MODEL").expect("EMBEDDINGS_MODEL не установлена!");
    let payload = json!({
        "model": model,
        "input": input,
    });
    let url = env::var("EMBEDDINGS_URL").expect("EMBEDDINGS_URL не установлена!");
    let response = client
        .post(url)
        .headers(headers)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()?
        .error_for_status()?;

    let embedding_response: EmbeddingResponse = response.json()?;

    if let Some(embedding) = embedding_response.data.into_iter().next() {
        Ok(embedding.embedding)
    } else {
        Err(anyhow::anyhow!("No embedding data found"))
    }
}

#[allow(unused_imports)]
mod tests {
    use super::*;

    #[test]
    fn test_emb() {
        let input = "Hello, world!";
        let embedding = emb(input).unwrap();
        assert_eq!(embedding.len(), 1024);
    }

    #[test]
    fn test_llm() {
        let system = "Always give short answer without explanations and other details";
        let user = "Who president of USA?";
        let assistant = llm(system, user).unwrap();
        assert!(assistant.len() > 5);
    }
}
