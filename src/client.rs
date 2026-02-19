use std::collections::HashMap;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Local, NaiveDate, Timelike};
use reqwest::Client;
use serde_json::{json, Value};

use crate::auth::FirebaseAuth;
use crate::firestore::{
    parse_document, parse_firestore_fields, to_firestore_fields, FirestoreClient,
};
use crate::models::*;

const TYPESENSE_HOST: &str = "https://oewdzs50x93n2c4mp.a1.typesense.net";
const TYPESENSE_API_KEY: &str = "4tKoPwBN6YaPXZDeQ7AyDfZbrjPbGMmG";

#[derive(Clone)]
pub struct MacroFactorClient {
    pub auth: FirebaseAuth,
    pub firestore: FirestoreClient,
    user_id: Option<String>,
}

impl MacroFactorClient {
    pub fn new(refresh_token: String) -> Self {
        let auth = FirebaseAuth::new(refresh_token);
        let firestore = FirestoreClient::new(auth.clone());
        Self {
            auth,
            firestore,
            user_id: None,
        }
    }

    /// Sign in with email and password.
    pub async fn login(email: &str, password: &str) -> Result<Self> {
        let auth = FirebaseAuth::sign_in_with_email(email, password).await?;
        let firestore = FirestoreClient::new(auth.clone());
        Ok(Self {
            auth,
            firestore,
            user_id: None,
        })
    }

    pub async fn get_user_id(&mut self) -> Result<String> {
        if let Some(ref uid) = self.user_id {
            return Ok(uid.clone());
        }
        let uid = self.auth.get_user_id().await?;
        self.user_id = Some(uid.clone());
        Ok(uid)
    }

    /// Get the user profile document.
    pub async fn get_profile(&mut self) -> Result<Value> {
        let uid = self.get_user_id().await?;
        let doc = self
            .firestore
            .get_document(&format!("users/{}", uid))
            .await?;
        Ok(parse_document(&doc))
    }

    /// List sub-collections under the user document.
    pub async fn list_subcollections(&self, document_path: &str) -> Result<Vec<String>> {
        self.firestore
            .list_collection_ids(Some(document_path))
            .await
    }

    /// Get a few documents from a collection for schema discovery.
    pub async fn sample_collection(&self, collection_path: &str, limit: u32) -> Result<Vec<Value>> {
        let (docs, _) = self
            .firestore
            .list_documents(collection_path, Some(limit), None)
            .await?;
        Ok(docs.iter().map(parse_document).collect())
    }

    /// Get a raw document by path and return parsed fields.
    pub async fn get_raw_document(&self, path: &str) -> Result<Value> {
        let doc = self.firestore.get_document(path).await?;
        Ok(parse_document(&doc))
    }

    /// Get scale/weight entries for a date range.
    /// Data is stored in `scale/{year}` docs with MMDD keys.
    pub async fn get_weight_entries(
        &mut self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<ScaleEntry>> {
        let uid = self.get_user_id().await?;
        let mut entries = Vec::new();

        // Collect all years in the range
        let start_year = start.format("%Y").to_string().parse::<i32>()?;
        let end_year = end.format("%Y").to_string().parse::<i32>()?;

        for year in start_year..=end_year {
            let path = format!("users/{}/scale/{}", uid, year);
            let doc = match self.firestore.get_document(&path).await {
                Ok(d) => d,
                Err(_) => continue,
            };

            if let Some(ref fields) = doc.fields {
                let parsed = parse_firestore_fields(&Value::Object(fields.clone()));
                if let Some(map) = parsed.as_object() {
                    for (key, val) in map {
                        if key.starts_with('_') || key.len() != 4 {
                            continue;
                        }
                        // Parse MMDD key
                        let month: u32 = key[..2].parse().unwrap_or(0);
                        let day: u32 = key[2..].parse().unwrap_or(0);
                        if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
                            if date >= start && date <= end {
                                if let Some(obj) = val.as_object() {
                                    let weight =
                                        obj.get("w").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                    let body_fat = obj.get("f").and_then(|v| v.as_f64());
                                    let source =
                                        obj.get("s").and_then(|v| v.as_str()).map(String::from);

                                    entries.push(ScaleEntry {
                                        date,
                                        weight,
                                        body_fat,
                                        source,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        entries.sort_by_key(|e| e.date);
        Ok(entries)
    }

    /// Get nutrition summaries for a date range.
    /// Data is stored in `nutrition/{year}` docs with MMDD keys.
    pub async fn get_nutrition(
        &mut self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<NutritionSummary>> {
        let uid = self.get_user_id().await?;
        let mut entries = Vec::new();

        let start_year = start.format("%Y").to_string().parse::<i32>()?;
        let end_year = end.format("%Y").to_string().parse::<i32>()?;

        for year in start_year..=end_year {
            let path = format!("users/{}/nutrition/{}", uid, year);
            let doc = match self.firestore.get_document(&path).await {
                Ok(d) => d,
                Err(_) => continue,
            };

            if let Some(ref fields) = doc.fields {
                let parsed = parse_firestore_fields(&Value::Object(fields.clone()));
                if let Some(map) = parsed.as_object() {
                    for (key, val) in map {
                        if key.starts_with('_') || key.len() != 4 {
                            continue;
                        }
                        let month: u32 = key[..2].parse().unwrap_or(0);
                        let day: u32 = key[2..].parse().unwrap_or(0);
                        if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
                            if date >= start && date <= end {
                                if let Some(obj) = val.as_object() {
                                    let parse_num = |k: &str| -> Option<f64> {
                                        obj.get(k).and_then(|v| {
                                            v.as_f64()
                                                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                                        })
                                    };

                                    entries.push(NutritionSummary {
                                        date,
                                        calories: parse_num("k"),
                                        protein: parse_num("p"),
                                        carbs: parse_num("c"),
                                        fat: parse_num("f"),
                                        sugar: parse_num("269"),
                                        fiber: parse_num("291"),
                                        source: obj
                                            .get("s")
                                            .and_then(|v| v.as_str())
                                            .map(String::from),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        entries.sort_by_key(|e| e.date);
        Ok(entries)
    }

    /// Get food log entries for a specific date.
    /// Data is stored in `food/{YYYY-MM-DD}` docs.
    pub async fn get_food_log(&mut self, date: NaiveDate) -> Result<Vec<FoodEntry>> {
        let uid = self.get_user_id().await?;
        let date_str = date.format("%Y-%m-%d").to_string();
        let path = format!("users/{}/food/{}", uid, date_str);

        let doc = match self.firestore.get_document(&path).await {
            Ok(d) => d,
            Err(e) if e.to_string().contains("404") => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut entries = Vec::new();

        if let Some(ref fields) = doc.fields {
            let parsed = parse_firestore_fields(&Value::Object(fields.clone()));
            if let Some(map) = parsed.as_object() {
                for (key, val) in map {
                    if key.starts_with('_') {
                        continue;
                    }
                    if let Some(obj) = val.as_object() {
                        let parse_num = |k: &str| -> Option<f64> {
                            obj.get(k).and_then(|v| {
                                v.as_f64()
                                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                            })
                        };
                        let parse_str =
                            |k: &str| obj.get(k).and_then(|v| v.as_str()).map(String::from);

                        let serving_grams = parse_num("g");
                        let user_qty = parse_num("y");
                        let unit_weight = parse_num("w");

                        let deleted = obj.get("d").and_then(|v| v.as_bool());

                        entries.push(FoodEntry {
                            date,
                            entry_id: key.clone(),
                            name: parse_str("t"),
                            brand: parse_str("b"),
                            calories_raw: parse_num("c"),
                            protein_raw: parse_num("p"),
                            carbs_raw: parse_num("e"),
                            fat_raw: parse_num("f"),
                            serving_grams,
                            user_qty,
                            unit_weight,
                            quantity: parse_num("q"),
                            serving_unit: parse_str("s"),
                            hour: parse_str("h"),
                            minute: parse_str("mi"),
                            source_type: parse_str("k"),
                            food_id: parse_str("id"),
                            deleted,
                        });
                    }
                }
            }
        }

        // Sort by hour:minute
        entries.sort_by(|a, b| {
            let time_a = (
                a.hour.as_deref().unwrap_or("0").parse::<u32>().unwrap_or(0),
                a.minute
                    .as_deref()
                    .unwrap_or("0")
                    .parse::<u32>()
                    .unwrap_or(0),
            );
            let time_b = (
                b.hour.as_deref().unwrap_or("0").parse::<u32>().unwrap_or(0),
                b.minute
                    .as_deref()
                    .unwrap_or("0")
                    .parse::<u32>()
                    .unwrap_or(0),
            );
            time_a.cmp(&time_b)
        });

        Ok(entries)
    }

    /// Get step counts for a date range.
    /// Data is stored in `steps/{year}` docs with MMDD keys.
    pub async fn get_steps(&mut self, start: NaiveDate, end: NaiveDate) -> Result<Vec<StepEntry>> {
        let uid = self.get_user_id().await?;
        let mut entries = Vec::new();

        let start_year = start.format("%Y").to_string().parse::<i32>()?;
        let end_year = end.format("%Y").to_string().parse::<i32>()?;

        for year in start_year..=end_year {
            let path = format!("users/{}/steps/{}", uid, year);
            let doc = match self.firestore.get_document(&path).await {
                Ok(d) => d,
                Err(_) => continue,
            };

            if let Some(ref fields) = doc.fields {
                let parsed = parse_firestore_fields(&Value::Object(fields.clone()));
                if let Some(map) = parsed.as_object() {
                    for (key, val) in map {
                        if key.starts_with('_') || key.len() != 4 {
                            continue;
                        }
                        let month: u32 = key[..2].parse().unwrap_or(0);
                        let day: u32 = key[2..].parse().unwrap_or(0);
                        if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
                            if date >= start && date <= end {
                                if let Some(obj) = val.as_object() {
                                    let steps = obj
                                        .get("st")
                                        .and_then(|v| {
                                            v.as_u64()
                                                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                                        })
                                        .unwrap_or(0);
                                    let source =
                                        obj.get("s").and_then(|v| v.as_str()).map(String::from);

                                    entries.push(StepEntry {
                                        date,
                                        steps,
                                        source,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        entries.sort_by_key(|e| e.date);
        Ok(entries)
    }

    /// Get the current macro/calorie goals from the user's planner.
    pub async fn get_goals(&mut self) -> Result<Goals> {
        let profile = self.get_profile().await?;

        let planner = profile
            .get("planner")
            .ok_or_else(|| anyhow!("No planner field in user profile"))?;

        let parse_vec = |key: &str| -> Vec<f64> {
            planner
                .get(key)
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| {
                            v.as_f64()
                                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                        })
                        .collect()
                })
                .unwrap_or_default()
        };

        Ok(Goals {
            calories: parse_vec("calories"),
            protein: parse_vec("protein"),
            carbs: parse_vec("carbs"),
            fat: parse_vec("fat"),
            tdee: planner.get("tdeeValue").and_then(|v| {
                v.as_f64()
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            }),
            program_style: planner
                .get("programStyle")
                .and_then(|v| v.as_str())
                .map(String::from),
            program_type: planner
                .get("programType")
                .and_then(|v| v.as_str())
                .map(String::from),
        })
    }

    /// Log a food entry for a given date and time.
    ///
    /// Pass `logged_at` as the local datetime when the food was consumed —
    /// the caller is responsible for providing the correct timezone.
    /// Use `chrono::Local::now()` for the current time.
    ///
    /// Fields like `calories`, `protein`, `carbs`, `fat` are required.
    /// The entry will be created with a timestamp-based ID.
    pub async fn log_food(
        &mut self,
        logged_at: DateTime<Local>,
        name: &str,
        calories: f64,
        protein: f64,
        carbs: f64,
        fat: f64,
    ) -> Result<()> {
        let uid = self.get_user_id().await?;
        let date_str = logged_at.format("%Y-%m-%d").to_string();
        let path = format!("users/{}/food/{}", uid, date_str);

        let ts = logged_at.timestamp_millis();
        let entry_id = format!("{}", ts * 1000);
        let food_id = format!("{}", ts * 1000 + 10);

        let hour = logged_at.hour().to_string();
        let minute = logged_at.minute().to_string();

        let entry = json!({
            "t": name,
            "b": "Quick Add",
            "c": format!("{:.1}", calories),
            "p": format!("{:.1}", protein),
            "e": format!("{:.1}", carbs),
            "f": format!("{:.1}", fat),
            "w": "100.0",
            "g": "100.0",
            "q": "1.0",
            "y": "1.0",
            "s": "serving",
            "u": "serving",
            "h": hour,
            "mi": minute,
            "k": "n",
            "id": food_id,
            "ca": &entry_id,
            "ua": &entry_id,
            "ef": Value::Null,
            "d": false,
            "x": "13",
            "m": [{"m": "serving", "q": "1.0", "w": "100.0"}]
        });

        let fields = to_firestore_fields(&json!({ &entry_id: entry }));

        // Firestore rejects field paths that start with a digit unless backtick-quoted
        let field_mask = format!("`{}`", entry_id);
        self.firestore
            .patch_document(&path, fields, &[&field_mask])
            .await?;

        Ok(())
    }

    /// Log a weight entry for a given date.
    /// Weight should be in kg.
    pub async fn log_weight(
        &mut self,
        date: NaiveDate,
        weight_kg: f64,
        body_fat: Option<f64>,
    ) -> Result<()> {
        let uid = self.get_user_id().await?;
        let year = date.format("%Y").to_string();
        let mmdd = date.format("%m%d").to_string();
        let path = format!("users/{}/scale/{}", uid, year);

        let mut entry = json!({
            "w": weight_kg,
            "s": "m",
            "do": null
        });
        if let Some(bf) = body_fat {
            entry["f"] = json!(bf);
        } else {
            entry["f"] = Value::Null;
        }

        let fields = to_firestore_fields(&json!({ &mmdd: entry }));

        let field_mask = format!("`{}`", mmdd);
        self.firestore
            .patch_document(&path, fields, &[&field_mask])
            .await?;

        Ok(())
    }

    /// Delete a weight entry for a given date.
    ///
    /// Removes the MMDD field from the `scale/{year}` document.
    pub async fn delete_weight_entry(&mut self, date: NaiveDate) -> Result<()> {
        let uid = self.get_user_id().await?;
        let year = date.format("%Y").to_string();
        let mmdd = date.format("%m%d").to_string();
        let path = format!("users/{}/scale/{}", uid, year);

        let fields = serde_json::Map::new();
        let field_mask = format!("`{}`", mmdd);
        self.firestore
            .patch_document(&path, fields, &[&field_mask])
            .await?;

        Ok(())
    }

    /// Import a manual nutrition summary for a given date.
    ///
    /// This writes to the `nutrition/{year}` collection, which is used for
    /// **externally imported** nutrition data (e.g. Apple Health syncs).
    /// The app computes daily totals from individual food entries automatically —
    /// you do NOT need to call this after logging food.
    pub async fn log_nutrition(
        &mut self,
        date: NaiveDate,
        calories: f64,
        protein: Option<f64>,
        carbs: Option<f64>,
        fat: Option<f64>,
    ) -> Result<()> {
        let uid = self.get_user_id().await?;
        let year = date.format("%Y").to_string();
        let mmdd = date.format("%m%d").to_string();
        let path = format!("users/{}/nutrition/{}", uid, year);

        let entry = json!({
            "k": format!("{:.0}", calories),
            "p": protein.map(|v| format!("{:.0}", v)).unwrap_or_default(),
            "c": carbs.map(|v| format!("{:.0}", v)).unwrap_or_default(),
            "f": fat.map(|v| format!("{:.0}", v)).unwrap_or_default(),
            "s": "m",
            "do": null
        });

        let fields = to_firestore_fields(&json!({ &mmdd: entry }));

        let field_mask = format!("`{}`", mmdd);
        self.firestore
            .patch_document(&path, fields, &[&field_mask])
            .await?;

        Ok(())
    }

    /// Search the food database using Typesense.
    ///
    /// Searches both `common_foods` and `branded_foods` collections.
    /// No authentication required — uses the Typesense API key directly.
    pub async fn search_foods(&self, query: &str) -> Result<Vec<SearchFoodResult>> {
        let client = Client::new();
        let url = format!("{}/multi_search", TYPESENSE_HOST);

        let body = json!({
            "searches": [
                {
                    "collection": "common_foods",
                    "q": query,
                    "query_by": "foodDesc",
                    "per_page": 10
                },
                {
                    "collection": "branded_foods",
                    "q": query,
                    "query_by": "foodDesc,brandName",
                    "per_page": 10
                }
            ]
        });

        let resp = client
            .post(&url)
            .header("x-typesense-api-key", TYPESENSE_API_KEY)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Typesense search failed: {} - {}", status, text));
        }

        let data: Value = resp.json().await?;
        let mut results = Vec::new();

        if let Some(searches) = data.get("results").and_then(|v| v.as_array()) {
            for (idx, search) in searches.iter().enumerate() {
                let branded = idx == 1;
                if let Some(hits) = search.get("hits").and_then(|v| v.as_array()) {
                    for hit in hits {
                        if let Some(doc) = hit.get("document") {
                            if let Some(result) = parse_typesense_hit(doc, branded) {
                                results.push(result);
                            }
                        }
                    }
                }
            }
        }

        Ok(results)
    }

    /// Log a food entry from a search result.
    ///
    /// `serving` specifies which serving option to use (from `food.servings` or `food.default_serving`).
    /// `quantity` is how many of that serving (e.g. 1.0 for one serving).
    pub async fn log_searched_food(
        &mut self,
        logged_at: DateTime<Local>,
        food: &SearchFoodResult,
        serving: &FoodServing,
        quantity: f64,
    ) -> Result<()> {
        let uid = self.get_user_id().await?;
        let date_str = logged_at.format("%Y-%m-%d").to_string();
        let path = format!("users/{}/food/{}", uid, date_str);

        let ts = logged_at.timestamp_millis();
        let entry_id = format!("{}", ts * 1000);

        let hour = logged_at.hour().to_string();
        let minute = logged_at.minute().to_string();

        // Serving gram weight (this becomes the "g" field — the base for macro values)
        let serving_grams = serving.gram_weight;
        // Scale factor from per-100g to per-serving
        let scale = serving_grams / 100.0;

        // Grams per one display unit
        let unit_weight = serving.gram_weight / serving.amount;
        // Total display units
        let total_units = quantity * serving.amount;

        let measurements: Vec<Value> = food
            .servings
            .iter()
            .map(|s| {
                json!({
                    "m": s.description,
                    "q": format!("{:.1}", s.amount),
                    "w": format!("{}", s.gram_weight)
                })
            })
            .collect();

        let mut entry = json!({
            "t": food.name,
            "b": food.brand.as_deref().unwrap_or(""),
            "c": format!("{}", food.calories_per_100g * scale),
            "p": format!("{}", food.protein_per_100g * scale),
            "e": format!("{}", food.carbs_per_100g * scale),
            "f": format!("{}", food.fat_per_100g * scale),
            "g": format!("{}", serving_grams),
            "w": format!("{}", unit_weight),
            "y": format!("{}", total_units),
            "q": format!("{}", serving.amount),
            "s": serving.description,
            "u": serving.description,
            "h": hour,
            "mi": minute,
            "k": "t",
            "id": food.food_id,
            "ca": &entry_id,
            "ua": &entry_id,
            "ef": Value::Null,
            "d": false,
            "o": false,
            "fav": false,
            "x": food.image_id.as_deref().unwrap_or("13"),
            "m": measurements
        });

        // Copy all micronutrient values, scaled to serving size
        if let Some(obj) = entry.as_object_mut() {
            for (code, val_per_100g) in &food.nutrients_per_100g {
                // Skip the main macro codes — already handled above
                if matches!(code.as_str(), "203" | "204" | "205" | "208") {
                    continue;
                }
                obj.insert(code.clone(), json!(format!("{}", val_per_100g * scale)));
            }
        }

        let fields = to_firestore_fields(&json!({ &entry_id: entry }));

        let field_mask = format!("`{}`", entry_id);
        self.firestore
            .patch_document(&path, fields, &[&field_mask])
            .await?;

        Ok(())
    }

    /// Delete a food entry by marking it as deleted.
    ///
    /// Sets the `d` (deleted) field to `true` on the entry.
    pub async fn delete_food_entry(&mut self, date: NaiveDate, entry_id: &str) -> Result<()> {
        let uid = self.get_user_id().await?;
        let date_str = date.format("%Y-%m-%d").to_string();
        let path = format!("users/{}/food/{}", uid, date_str);

        let fields = to_firestore_fields(&json!({
            entry_id: { "d": true }
        }));

        let field_mask = format!("`{}`.d", entry_id);
        self.firestore
            .patch_document(&path, fields, &[&field_mask])
            .await?;

        Ok(())
    }
}

/// Parse a Typesense document hit into a SearchFoodResult.
fn parse_typesense_hit(doc: &Value, branded: bool) -> Option<SearchFoodResult> {
    let food_id = doc.get("id").and_then(|v| v.as_str())?.to_string();
    let name = doc.get("foodDesc").and_then(|v| v.as_str())?.to_string();

    let brand = doc
        .get("brandName")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let nutrient = |code: &str| -> f64 {
        doc.get(code)
            .and_then(|v| {
                v.as_f64()
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            })
            .unwrap_or(0.0)
    };

    let calories_per_100g = nutrient("208");
    let protein_per_100g = nutrient("203");
    let fat_per_100g = nutrient("204");
    let carbs_per_100g = nutrient("205");

    let default_serving = doc.get("dfSrv").and_then(|ds| {
        let desc = ds.get("msreDesc").and_then(|v| v.as_str())?.to_string();
        let amount = ds.get("amount").and_then(|v| {
            v.as_f64()
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        })?;
        let gram_weight = ds.get("gmWgt").and_then(|v| {
            v.as_f64()
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        })?;
        Some(FoodServing {
            description: desc,
            amount,
            gram_weight,
        })
    });

    let servings = doc
        .get("weights")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|w| {
                    let desc = w.get("msreDesc").and_then(|v| v.as_str())?.to_string();
                    let amount = w.get("amount").and_then(|v| {
                        v.as_f64()
                            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                    })?;
                    let gram_weight = w.get("gmWgt").and_then(|v| {
                        v.as_f64()
                            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                    })?;
                    Some(FoodServing {
                        description: desc,
                        amount,
                        gram_weight,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let image_id = doc
        .get("imageId")
        .and_then(|v| {
            v.as_str()
                .map(String::from)
                .or_else(|| v.as_i64().map(|n| n.to_string()))
        })
        .filter(|s| !s.is_empty());

    // Collect all numeric-keyed nutrient values (USDA nutrient codes)
    let mut nutrients_per_100g = HashMap::new();
    if let Some(obj) = doc.as_object() {
        for (key, val) in obj {
            if key.chars().all(|c| c.is_ascii_digit()) {
                if let Some(v) = val
                    .as_f64()
                    .or_else(|| val.as_str().and_then(|s| s.parse().ok()))
                {
                    nutrients_per_100g.insert(key.clone(), v);
                }
            }
        }
    }

    let source = doc
        .get("source")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    Some(SearchFoodResult {
        food_id,
        name,
        brand,
        calories_per_100g,
        protein_per_100g,
        fat_per_100g,
        carbs_per_100g,
        default_serving,
        servings,
        image_id,
        nutrients_per_100g,
        source,
        branded,
    })
}
