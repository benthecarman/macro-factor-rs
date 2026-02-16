use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::auth::{FirebaseAuth, PROJECT_ID};

const BASE_URL: &str = "https://firestore.googleapis.com/v1";

#[derive(Clone)]
pub struct FirestoreClient {
    client: Client,
    auth: FirebaseAuth,
}

#[derive(Debug, Deserialize)]
pub struct Document {
    pub name: String,
    pub fields: Option<Map<String, Value>>,
    #[serde(rename = "createTime")]
    pub create_time: Option<String>,
    #[serde(rename = "updateTime")]
    pub update_time: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListDocumentsResponse {
    documents: Option<Vec<Document>>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RunQueryResponse {
    document: Option<Document>,
    #[allow(dead_code)]
    #[serde(rename = "readTime")]
    read_time: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListCollectionIdsResponse {
    #[serde(rename = "collectionIds")]
    collection_ids: Option<Vec<String>>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}

impl FirestoreClient {
    pub fn new(auth: FirebaseAuth) -> Self {
        Self {
            client: Client::new(),
            auth,
        }
    }

    fn documents_base(&self) -> String {
        format!(
            "{}/projects/{}/databases/(default)/documents",
            BASE_URL, PROJECT_ID
        )
    }

    pub async fn get_document(&self, path: &str) -> Result<Document> {
        let token = self.auth.get_id_token().await?;
        let url = format!("{}/{}", self.documents_base(), path);

        let resp = self
            .client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("GET {} failed: {} - {}", path, status, body));
        }

        Ok(resp.json().await?)
    }

    pub async fn list_documents(
        &self,
        collection_path: &str,
        page_size: Option<u32>,
        page_token: Option<&str>,
    ) -> Result<(Vec<Document>, Option<String>)> {
        let token = self.auth.get_id_token().await?;
        let url = format!("{}/{}", self.documents_base(), collection_path);

        let mut req = self.client.get(&url).bearer_auth(&token);

        if let Some(size) = page_size {
            req = req.query(&[("pageSize", size.to_string())]);
        }
        if let Some(pt) = page_token {
            req = req.query(&[("pageToken", pt)]);
        }

        let resp = req.send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!(
                "LIST {} failed: {} - {}",
                collection_path,
                status,
                body
            ));
        }

        let list_resp: ListDocumentsResponse = resp.json().await?;
        Ok((
            list_resp.documents.unwrap_or_default(),
            list_resp.next_page_token,
        ))
    }

    pub async fn list_collection_ids(
        &self,
        parent_path: Option<&str>,
    ) -> Result<Vec<String>> {
        let token = self.auth.get_id_token().await?;
        let parent = match parent_path {
            Some(p) => format!("{}/{}", self.documents_base(), p),
            None => self.documents_base(),
        };
        let url = format!("{}:listCollectionIds", parent);

        let mut all_ids = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let mut body = json!({});
            if let Some(ref pt) = page_token {
                body["pageToken"] = json!(pt);
            }

            let resp = self
                .client
                .post(&url)
                .bearer_auth(&token)
                .json(&body)
                .send()
                .await?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                return Err(anyhow!(
                    "listCollectionIds failed: {} - {}",
                    status,
                    body
                ));
            }

            let list_resp: ListCollectionIdsResponse = resp.json().await?;
            if let Some(ids) = list_resp.collection_ids {
                all_ids.extend(ids);
            }

            match list_resp.next_page_token {
                Some(pt) if !pt.is_empty() => page_token = Some(pt),
                _ => break,
            }
        }

        Ok(all_ids)
    }

    pub async fn run_query(
        &self,
        parent_path: Option<&str>,
        structured_query: Value,
    ) -> Result<Vec<Document>> {
        let token = self.auth.get_id_token().await?;
        let parent = match parent_path {
            Some(p) => format!("{}/{}", self.documents_base(), p),
            None => self.documents_base(),
        };
        let url = format!("{}:runQuery", parent);

        let body = json!({
            "structuredQuery": structured_query
        });

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("runQuery failed: {} - {}", status, body));
        }

        let results: Vec<RunQueryResponse> = resp.json().await?;
        Ok(results.into_iter().filter_map(|r| r.document).collect())
    }
}

/// Parse a Firestore typed value into a serde_json::Value.
pub fn parse_firestore_value(val: &Value) -> Value {
    if let Some(s) = val.get("stringValue") {
        return s.clone();
    }
    if let Some(i) = val.get("integerValue") {
        // Firestore sends integers as strings
        if let Some(s) = i.as_str() {
            if let Ok(n) = s.parse::<i64>() {
                return json!(n);
            }
        }
        return i.clone();
    }
    if let Some(d) = val.get("doubleValue") {
        return d.clone();
    }
    if let Some(b) = val.get("booleanValue") {
        return b.clone();
    }
    if let Some(_) = val.get("nullValue") {
        return Value::Null;
    }
    if let Some(ts) = val.get("timestampValue") {
        return ts.clone();
    }
    if let Some(r) = val.get("referenceValue") {
        return r.clone();
    }
    if let Some(geo) = val.get("geoPointValue") {
        return geo.clone();
    }
    if let Some(bytes) = val.get("bytesValue") {
        return bytes.clone();
    }
    if let Some(map) = val.get("mapValue") {
        if let Some(fields) = map.get("fields") {
            return parse_firestore_fields(fields);
        }
        return json!({});
    }
    if let Some(arr) = val.get("arrayValue") {
        if let Some(values) = arr.get("values").and_then(|v| v.as_array()) {
            return Value::Array(values.iter().map(parse_firestore_value).collect());
        }
        return json!([]);
    }

    // Unknown format, return as-is
    val.clone()
}

/// Parse Firestore document fields into a flat JSON object.
pub fn parse_firestore_fields(fields: &Value) -> Value {
    if let Some(map) = fields.as_object() {
        let mut result = Map::new();
        for (key, val) in map {
            result.insert(key.clone(), parse_firestore_value(val));
        }
        Value::Object(result)
    } else {
        Value::Null
    }
}

/// Parse a full Firestore document into a JSON object with parsed fields.
pub fn parse_document(doc: &Document) -> Value {
    let mut result = Map::new();

    // Extract document ID from the name path
    let name = &doc.name;
    if let Some(id) = name.rsplit('/').next() {
        result.insert("_id".to_string(), json!(id));
    }
    result.insert("_path".to_string(), json!(name));

    if let Some(ref fields) = doc.fields {
        if let Value::Object(parsed) = parse_firestore_fields(&Value::Object(fields.clone())) {
            for (key, val) in parsed {
                result.insert(key, val);
            }
        }
    }

    Value::Object(result)
}
