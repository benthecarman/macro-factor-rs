# macro-factor-rs

Unofficial Rust client library for reading [MacroFactor](https://macrofactorapp.com/) data via the Firestore REST API.

## Authentication

### Email/Password (recommended)

Add a password to your MacroFactor account, then sign in directly:

```rust
let mut client = MacroFactorClient::login("email", "password").await?;
```

### Refresh Token

If you already have a Firebase refresh token:

```rust
let mut client = MacroFactorClient::new("your_refresh_token".to_string());
```

## Usage

```rust
use macro_factor_api::client::MacroFactorClient;
use chrono::NaiveDate;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut client = MacroFactorClient::login("email", "password").await?;

    // Get user profile
    let profile = client.get_profile().await?;

    // Get weight entries
    let start = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(2025, 1, 31).unwrap();
    let weights = client.get_weight_entries(start, end).await?;

    // Get nutrition summaries
    let nutrition = client.get_nutrition(start, end).await?;

    // Get food log for a specific day
    let foods = client.get_food_log(start).await?;

    // Get step counts
    let steps = client.get_steps(start, end).await?;

    Ok(())
}
```

## API Methods

| Method | Description |
|--------|-------------|
| `login(email, password)` | Sign in with email/password |
| `new(refresh_token)` | Create client with a refresh token |
| `get_profile()` | User profile (name, height, weight units, goals, etc.) |
| `get_goals()` | Current calorie/macro targets and TDEE |
| `get_weight_entries(start, end)` | Scale entries with weight in kg |
| `get_nutrition(start, end)` | Daily summaries (calories, protein, carbs, fat, sugar, fiber) |
| `get_food_log(date)` | Individual food entries for a day |
| `get_steps(start, end)` | Daily step counts |
| `log_food(date, name, cal, protein, carbs, fat)` | Log a food entry |
| `log_weight(date, weight_kg, body_fat)` | Log a weight entry |
| `log_nutrition(date, cal, protein, carbs, fat)` | Log a nutrition summary |
| `get_raw_document(path)` | Fetch any Firestore document by path |

## Firestore Schema

Data lives under `users/{uid}/`:

- **`scale/{year}`** — weight entries keyed by `MMDD`
- **`nutrition/{year}`** — daily macro summaries keyed by `MMDD`
- **`food/{YYYY-MM-DD}`** — food log entries per day
- **`steps/{year}`** — step counts keyed by `MMDD`
- **`body/{year}`** — body measurements keyed by `MMDD`
- **`customFoods/{id}`** — user-created foods and recipes
