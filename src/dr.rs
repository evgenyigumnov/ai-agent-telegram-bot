use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;

use crate::ai::emb;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Document {
    pub id: i32,
    pub text: String,
    pub distance: f32,
}

#[derive(Serialize)]
#[allow(dead_code)]
struct Point {
    id: i32,
    vector: Vec<f32>,
    payload: Value,
}

#[allow(dead_code)]
pub fn add_document(id: i32, text: &str) -> anyhow::Result<()> {
    let embedding = emb(text)?;
    let point = Point {
        id,
        vector: embedding,
        payload: json!({ "text": text }),
    };
    let qdrant_url = env::var("QDRANT_URL")?;
    let collection_name =
        env::var("QDRANT_COLLECTION_NAME")?;
    let client = Client::new();
    let url = format!(
        "{}/collections/{}/points?wait=true",
        qdrant_url,collection_name
    );
    let payload = json!({
        "points": [point]
    });

    let _response = client.put(&url).json(&payload).send()?;
    // println!("Document added: {:?}", _response.text()?);
    Ok(())
}

#[allow(dead_code)]
pub fn delete_document(id: i32) -> anyhow::Result<()> {
    let client = Client::new();
    let qdrant_url = env::var("QDRANT_URL")?;
    let collection_name =
        env::var("QDRANT_COLLECTION_NAME")?;
    let url = format!(
        "{}/collections/{}/points/delete",
        qdrant_url,collection_name
    );
    let payload = json!({
        "points": [id]
    });

    let _response = client
        .post(&url)
        .json(&payload)
        .send()?
        .error_for_status()?;

    // println!("Document deleted: {:?}", _response.text()?);
    Ok(())
}

pub fn create_collection() -> anyhow::Result<()> {
    let qdrant_url = env::var("QDRANT_URL")?;
    let collection_name =
        env::var("QDRANT_COLLECTION_NAME")?;
    let embeddings_lenghth: i32 =
        env::var("EMBEDDINGS_LENGTH")?.parse()?;
    let client = Client::new();
    let _response = client
        .put(format!("{}/collections/{}", qdrant_url,collection_name))
        .json(&json!({
            "vectors": {
                "size": embeddings_lenghth,
                "distance": "Cosine"
            }
        }))
        .send()?
        .error_for_status()?;
    // println!("Collection created: {:?}", _response.text()?);
    Ok(())
}
#[allow(dead_code)]
pub fn delete_collection() -> anyhow::Result<()> {
    let qdrant_url = env::var("QDRANT_URL")?;
    let collection_name =
        env::var("QDRANT_COLLECTION_NAME")?;
    let client = Client::new();
    let _response = client
        .delete(format!("{}/collections/{}", qdrant_url,collection_name))
        .send()?
        .error_for_status()?;
    println!("Collection deleted: {:?}", _response.text()?);
    Ok(())
}

pub fn exists_collection() -> anyhow::Result<bool> {
    let qdrant_url = env::var("QDRANT_URL")?;
    let collection_name =
        env::var("QDRANT_COLLECTION_NAME")?;
    let client = Client::new();
    let response = client
        .get(format!("{}/collections/{}", qdrant_url,collection_name))
        .send()?;
    Ok(response.status().is_success())
}

#[derive(Deserialize)]
struct QdrantSearchResultItem {
    id: i32,
    score: f32,
    payload: Value,
}

#[derive(Deserialize)]
struct QdrantSearchResponse {
    result: Vec<QdrantSearchResultItem>,
}

#[derive(Deserialize)]
struct QdrantGetPointResult {
    payload: Value,
}

#[derive(Deserialize)]
struct QdrantGetPointResponse {
    result: QdrantGetPointResult,
}

pub fn last_document_id() -> anyhow::Result<i32> {
    let mut last_id = 0;
    let all_docs = all_documents()?;
    for doc in all_docs {
        if doc.id > last_id {
            last_id = doc.id;
        }
    }
    Ok(last_id)
}
pub fn all_documents() -> anyhow::Result<Vec<Document>> {
    let qdrant_url = env::var("QDRANT_URL")?;
    let collection_name =
        env::var("QDRANT_COLLECTION_NAME")?;
    let client = Client::new();
    let url = format!(
        "{}/collections/{}/points/scroll",
        qdrant_url,collection_name
    );
    let mut documents = Vec::new();
    let mut offset: Option<usize> = None;

    loop {
        // Формируем полезную нагрузку: если offset установлен, включаем его в запрос.
        let mut payload = json!({
            "limit": 100,
            "with_payload": true,
            "with_vector": false,
        });
        if let Some(off) = offset {
            payload["offset"] = json!(off);
        }

        let response = client
            .post(&url)
            .json(&payload)
            .send()?
            .error_for_status()?;
        let scroll_response: serde_json::Value = response.json()?;

        if let Some(result) = scroll_response.get("result") {
            let empty_vec = vec![];
            let points = result
                .get("points")
                .and_then(|p| p.as_array())
                .unwrap_or(&empty_vec);

            for item in points {
                if let (Some(id), Some(payload)) =
                    (item.get("id").and_then(|v| v.as_i64()), item.get("payload"))
                {
                    let text = payload
                        .get("text")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    documents.push(Document {
                        id: id as i32,
                        text,
                        distance: 0.0,
                    });
                }
            }

            // Если есть значение offset для следующей страницы, обновляем его,
            // иначе прерываем цикл
            offset = result
                .get("next_page_offset")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);
            if offset.is_none() || points.is_empty() {
                break;
            }
        } else {
            break;
        }
    }

    Ok(documents)
}

#[allow(dead_code)]
pub fn find_document(id: i32) -> anyhow::Result<Document> {
    let client = Client::new();
    let qdrant_url = env::var("QDRANT_URL")?;
    let collection_name =
        env::var("QDRANT_COLLECTION_NAME")?;
    let url = format!(
        "{}/collections/{}/points/{}",
        qdrant_url,collection_name, id
    );
    let response = client.get(&url).send()?.error_for_status()?;

    // Deserialize into the new struct
    let get_response: QdrantGetPointResponse = response.json()?;

    // Extract the text payload
    let text = get_response
        .result
        .payload
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Ok(Document {
        id,
        text,
        distance: 0.0, // The GET endpoint may not return a score; adjust as needed
    })
}
#[allow(dead_code)]
pub fn search_one(query: &str) -> anyhow::Result<Document> {
    let documents = search(query, 1)?;
    if documents.is_empty() {
        Err(anyhow::anyhow!("No documents found"))
    } else {
        Ok(documents[0].clone())
    }
}

// distance > 0.6
// если ничего нет то просто берет первый документ
pub fn search_smart(query: &str) -> anyhow::Result<Vec<Document>> {
    let documents = search(query, 3)?;
    if documents.is_empty() {
        let closest = closest(query)?;
        Ok(closest)
    } else {
        let mut result = Vec::new();
        for doc in &documents {
            if doc.distance > 0.6 {
                result.push(doc.clone());
            }
        }
        if result.is_empty() {
            result.push(documents[0].clone());
        }
        Ok(result)
    }
}

#[allow(dead_code)]
pub fn search(query: &str, limit: usize) -> anyhow::Result<Vec<Document>> {
    let query_vector = emb(query)?;
    let client = Client::new();
    let qdrant_url = env::var("QDRANT_URL")?;
    let collection_name =
        env::var("QDRANT_COLLECTION_NAME")?;
    let url = format!(
        "{}/collections/{}/points/search",
        qdrant_url,collection_name
    );
    let payload = json!({
        "vector": query_vector,
        "limit": limit,
        "with_payload": true,
        "with_vector": false,
    });

    let response = client
        .post(&url)
        .json(&payload)
        .send()?;
    let status = response.status();
    if !status.is_success() {
        return Err(anyhow::anyhow!("Search failed: {}", response.text().unwrap_or(status.to_string())));
    }

    let search_response: QdrantSearchResponse = response.json()?;

    let documents = search_response
        .result
        .into_iter()
        .map(|item| {
            let text = item
                .payload
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Document {
                id: item.id,
                text,
                distance: item.score,
            }
        })
        .collect();
    Ok(documents)
}

#[allow(dead_code)]
pub fn closest(query: &str) -> anyhow::Result<Vec<Document>> {
    search(query, 10)
}
#[allow(dead_code)]
pub fn closest_limit(query: &str, limit: usize) -> anyhow::Result<Vec<Document>> {
    search(query, limit)
}

