use chrono::Local;
use macro_factor_api::client::MacroFactorClient;

fn get_credentials() -> Option<(String, String)> {
    dotenvy::dotenv().ok();
    let email = std::env::var("MF_EMAIL").ok()?;
    let password = std::env::var("MF_PASSWORD").ok()?;
    Some((email, password))
}

async fn authenticated_client() -> Option<MacroFactorClient> {
    let (email, password) = get_credentials()?;
    MacroFactorClient::login(&email, &password).await.ok()
}

#[tokio::test]
async fn search_foods_returns_results() {
    let client = MacroFactorClient::new("unused".to_string());
    let results = client.search_foods("chicken breast").await.unwrap();

    assert!(!results.is_empty(), "search should return results");

    let first = &results[0];
    assert!(!first.food_id.is_empty());
    assert!(!first.name.is_empty());
    assert!(first.calories_per_100g > 0.0);
    assert!(first.protein_per_100g > 0.0);
    assert!(first.image_id.is_some());
    assert!(!first.nutrients_per_100g.is_empty());
}

#[tokio::test]
async fn search_foods_has_servings() {
    let client = MacroFactorClient::new("unused".to_string());
    let results = client.search_foods("chicken breast").await.unwrap();

    let with_servings = results.iter().find(|r| !r.servings.is_empty());
    assert!(
        with_servings.is_some(),
        "at least one result should have servings"
    );

    let food = with_servings.unwrap();
    assert!(
        food.default_serving.is_some(),
        "should have a default serving"
    );
    let ds = food.default_serving.as_ref().unwrap();
    assert!(ds.amount > 0.0);
    assert!(ds.gram_weight > 0.0);
    assert!(!ds.description.is_empty());
}

#[tokio::test]
async fn search_foods_includes_branded() {
    let client = MacroFactorClient::new("unused".to_string());
    let results = client.search_foods("protein bar").await.unwrap();

    let branded = results.iter().find(|r| r.branded);
    assert!(branded.is_some(), "should include branded results");
}

#[tokio::test]
async fn log_and_delete_searched_food() {
    let Some(mut client) = authenticated_client().await else {
        eprintln!("skipping log_and_delete_searched_food: no credentials");
        return;
    };

    let now = Local::now();
    let today = now.date_naive();

    // Search
    let results = client.search_foods("chicken breast").await.unwrap();
    let food = &results[0];
    let serving = food.default_serving.as_ref().unwrap();

    // Log
    client
        .log_searched_food(now, food, serving, 1.0)
        .await
        .unwrap();

    // Read back and find our entry
    let entries = client.get_food_log(today).await.unwrap();
    let entry = entries
        .iter()
        .rev()
        .find(|e| e.food_id.as_deref() == Some(&food.food_id))
        .expect("should find logged entry");

    assert_eq!(entry.name.as_deref(), Some(food.name.as_str()));
    assert_eq!(entry.deleted, Some(false));

    let expected_cal = food.calories_per_100g * serving.gram_weight / 100.0;
    let actual_cal = entry.calories().unwrap();
    assert!(
        (actual_cal - expected_cal).abs() < 1.0,
        "calories should match: expected {expected_cal:.1}, got {actual_cal:.1}"
    );

    // Delete
    let entry_id = entry.entry_id.clone();
    client.delete_food_entry(today, &entry_id).await.unwrap();

    // Verify deleted (hard delete — entry should be gone)
    let entries = client.get_food_log(today).await.unwrap();
    assert!(
        entries.iter().all(|e| e.entry_id != entry_id),
        "entry should be removed after delete"
    );
}
