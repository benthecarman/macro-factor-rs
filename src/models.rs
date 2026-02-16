use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// A weight/scale measurement entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaleEntry {
    pub date: NaiveDate,
    /// Weight in kg
    pub weight: f64,
    /// Body fat percentage
    pub body_fat: Option<f64>,
    /// Source (e.g. "m" = manual, "a" = Apple Health)
    pub source: Option<String>,
}

/// A daily nutrition summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NutritionSummary {
    pub date: NaiveDate,
    /// Calories (kcal)
    pub calories: Option<f64>,
    /// Protein (g)
    pub protein: Option<f64>,
    /// Carbs (g)
    pub carbs: Option<f64>,
    /// Fat (g)
    pub fat: Option<f64>,
    /// Sugar (g) — nutrient code 269
    pub sugar: Option<f64>,
    /// Fiber (g) — nutrient code 291
    pub fiber: Option<f64>,
    /// Source
    pub source: Option<String>,
}

/// An individual food log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoodEntry {
    pub date: NaiveDate,
    /// Entry timestamp ID
    pub entry_id: String,
    /// Food name
    pub name: Option<String>,
    /// Brand
    pub brand: Option<String>,
    /// Calories (kcal)
    pub calories: Option<f64>,
    /// Protein (g)
    pub protein: Option<f64>,
    /// Carbs (g)
    pub carbs: Option<f64>,
    /// Fat (g)
    pub fat: Option<f64>,
    /// Weight in grams
    pub weight_grams: Option<f64>,
    /// Quantity
    pub quantity: Option<f64>,
    /// Serving unit
    pub serving_unit: Option<String>,
    /// Hour logged
    pub hour: Option<String>,
    /// Minute logged
    pub minute: Option<String>,
    /// Source type: "t" = typesense, "n" = custom
    pub source_type: Option<String>,
    /// Food ID
    pub food_id: Option<String>,
}

/// Daily step count entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepEntry {
    pub date: NaiveDate,
    /// Step count
    pub steps: u64,
    /// Source
    pub source: Option<String>,
}

/// User profile from the top-level user document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub id: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub sex: Option<String>,
    pub dob: Option<String>,
    pub height: Option<f64>,
    pub height_units: Option<String>,
    pub weight_units: Option<String>,
    pub calorie_units: Option<String>,
}
