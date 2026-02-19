use std::collections::HashMap;

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

/// A serving option for a food item (from Typesense search results).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoodServing {
    /// Serving description (e.g. "oz", "cup", "bar")
    pub description: String,
    /// Amount in display units (e.g. 3.0)
    pub amount: f64,
    /// Weight in grams for this serving (e.g. 85.0)
    pub gram_weight: f64,
}

/// A food search result from Typesense.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchFoodResult {
    /// Food ID (e.g. "uc_5749" or "m_19306281")
    pub food_id: String,
    /// Food description/name
    pub name: String,
    /// Brand name (None if generic/common food)
    pub brand: Option<String>,
    /// Calories per 100g
    pub calories_per_100g: f64,
    /// Protein per 100g
    pub protein_per_100g: f64,
    /// Fat per 100g
    pub fat_per_100g: f64,
    /// Carbs per 100g
    pub carbs_per_100g: f64,
    /// Default serving option
    pub default_serving: Option<FoodServing>,
    /// All available serving options
    pub servings: Vec<FoodServing>,
    /// Image ID for the bundled SVG icon (maps to assets/foods/i{id}.svg in the app)
    pub image_id: Option<String>,
    /// All nutrient values per 100g, keyed by USDA nutrient code (e.g. "269"=sugar, "291"=fiber)
    pub nutrients_per_100g: HashMap<String, f64>,
    /// Data source (e.g. "USDAC", "MND")
    pub source: Option<String>,
    /// Whether this is a branded food (vs common)
    pub branded: bool,
}

/// An individual food log entry.
///
/// Raw values (`calories_raw`, `protein_raw`, etc.) are per serving size (`serving_grams`).
/// Use the accessor methods (`.calories()`, `.protein()`, etc.) to get actual consumed amounts,
/// which apply the quantity multiplier: `raw * (user_qty * unit_weight) / serving_grams`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoodEntry {
    pub date: NaiveDate,
    /// Entry timestamp ID
    pub entry_id: String,
    /// Food name
    pub name: Option<String>,
    /// Brand
    pub brand: Option<String>,
    /// Calories per serving size (kcal)
    pub calories_raw: Option<f64>,
    /// Protein per serving size (g)
    pub protein_raw: Option<f64>,
    /// Carbs per serving size (g)
    pub carbs_raw: Option<f64>,
    /// Fat per serving size (g)
    pub fat_raw: Option<f64>,
    /// Grams per serving size ("g" field)
    pub serving_grams: Option<f64>,
    /// User quantity in display units ("y" field)
    pub user_qty: Option<f64>,
    /// Grams per display unit ("w" field)
    pub unit_weight: Option<f64>,
    /// Quantity in serving units ("q" field)
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
    /// Whether this entry has been deleted
    pub deleted: Option<bool>,
}

impl FoodEntry {
    /// Multiplier to convert per-serving values to actual consumed amounts.
    pub fn multiplier(&self) -> Option<f64> {
        match (self.serving_grams, self.user_qty, self.unit_weight) {
            (Some(g), Some(y), Some(w)) if g > 0.0 => Some((y * w) / g),
            _ => None,
        }
    }

    /// Actual calories consumed.
    pub fn calories(&self) -> Option<f64> {
        match (self.calories_raw, self.multiplier()) {
            (Some(v), Some(m)) => Some(v * m),
            _ => self.calories_raw,
        }
    }

    /// Actual protein consumed (g).
    pub fn protein(&self) -> Option<f64> {
        match (self.protein_raw, self.multiplier()) {
            (Some(v), Some(m)) => Some(v * m),
            _ => self.protein_raw,
        }
    }

    /// Actual carbs consumed (g).
    pub fn carbs(&self) -> Option<f64> {
        match (self.carbs_raw, self.multiplier()) {
            (Some(v), Some(m)) => Some(v * m),
            _ => self.carbs_raw,
        }
    }

    /// Actual fat consumed (g).
    pub fn fat(&self) -> Option<f64> {
        match (self.fat_raw, self.multiplier()) {
            (Some(v), Some(m)) => Some(v * m),
            _ => self.fat_raw,
        }
    }

    /// Actual weight consumed (g).
    pub fn weight_grams(&self) -> Option<f64> {
        match (self.user_qty, self.unit_weight) {
            (Some(y), Some(w)) => Some(y * w),
            _ => None,
        }
    }
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

/// Daily macro/calorie goals from the planner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goals {
    /// Daily calorie targets per day of week (Mon=0..Sun=6)
    pub calories: Vec<f64>,
    /// Daily protein targets (g) per day of week
    pub protein: Vec<f64>,
    /// Daily carbs targets (g) per day of week
    pub carbs: Vec<f64>,
    /// Daily fat targets (g) per day of week
    pub fat: Vec<f64>,
    /// Current TDEE estimate
    pub tdee: Option<f64>,
    /// Program style (e.g. "coached")
    pub program_style: Option<String>,
    /// Program type (e.g. "performance")
    pub program_type: Option<String>,
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
