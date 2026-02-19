use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Public Firebase Web API key for the MacroFactor project.
/// This is not a secret — it is embedded in every copy of the app
/// and only usable with Firebase's configured auth providers.
const FIREBASE_WEB_API_KEY: &str = "AIzaSyA17Uwy37irVEQSwz6PIyX3wnkHrDBeleA";
pub const PROJECT_ID: &str = "sbs-diet-app";

#[derive(Debug, Deserialize)]
struct RefreshTokenResponse {
    id_token: String,
    refresh_token: String,
    expires_in: String,
}

#[derive(Debug, Clone)]
struct CachedToken {
    id_token: String,
    expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone)]
pub struct FirebaseAuth {
    client: Client,
    refresh_token: Arc<Mutex<String>>,
    cached_token: Arc<Mutex<Option<CachedToken>>>,
}

#[derive(Debug, Deserialize)]
struct SignInResponse {
    #[serde(rename = "idToken")]
    id_token: String,
    #[serde(rename = "refreshToken")]
    refresh_token: String,
    #[serde(rename = "expiresIn")]
    expires_in: String,
    #[allow(dead_code)]
    #[serde(rename = "localId")]
    local_id: String,
}

impl FirebaseAuth {
    pub fn new(refresh_token: String) -> Self {
        Self {
            client: Client::new(),
            refresh_token: Arc::new(Mutex::new(refresh_token)),
            cached_token: Arc::new(Mutex::new(None)),
        }
    }

    /// Sign in with email and password, returning a FirebaseAuth with a fresh refresh token.
    pub async fn sign_in_with_email(email: &str, password: &str) -> Result<Self> {
        let client = Client::new();
        let url = format!(
            "https://identitytoolkit.googleapis.com/v1/accounts:signInWithPassword?key={}",
            FIREBASE_WEB_API_KEY
        );

        let resp = client
            .post(&url)
            .header("X-Ios-Bundle-Identifier", "com.sbs.diet")
            .json(&serde_json::json!({
                "email": email,
                "password": password,
                "returnSecureToken": true
            }))
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Sign-in failed: {} - {}", status, body));
        }

        let sign_in: SignInResponse = resp.json().await?;

        let expires_in: i64 = sign_in.expires_in.parse().unwrap_or(3600);
        let expires_at = chrono::Utc::now() + chrono::Duration::seconds(expires_in);

        Ok(Self {
            client,
            refresh_token: Arc::new(Mutex::new(sign_in.refresh_token)),
            cached_token: Arc::new(Mutex::new(Some(CachedToken {
                id_token: sign_in.id_token,
                expires_at,
            }))),
        })
    }

    pub async fn get_id_token(&self) -> Result<String> {
        // Check if we have a valid cached token (with 60s margin)
        {
            let cached = self.cached_token.lock().await;
            if let Some(ref token) = *cached {
                if token.expires_at > chrono::Utc::now() + chrono::Duration::seconds(60) {
                    return Ok(token.id_token.clone());
                }
            }
        }

        self.refresh_id_token().await
    }

    async fn refresh_id_token(&self) -> Result<String> {
        let refresh_token = self.refresh_token.lock().await.clone();

        let url = format!(
            "https://securetoken.googleapis.com/v1/token?key={}",
            FIREBASE_WEB_API_KEY
        );

        let resp = self
            .client
            .post(&url)
            .header("X-Ios-Bundle-Identifier", "com.sbs.diet")
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", &refresh_token),
            ])
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Failed to refresh token: {} - {}", status, body));
        }

        let token_resp: RefreshTokenResponse = resp.json().await?;

        let expires_in: i64 = token_resp.expires_in.parse().unwrap_or(3600);
        let expires_at = chrono::Utc::now() + chrono::Duration::seconds(expires_in);

        // Update refresh token if it changed
        *self.refresh_token.lock().await = token_resp.refresh_token;

        // Cache the new ID token
        let id_token = token_resp.id_token.clone();
        *self.cached_token.lock().await = Some(CachedToken {
            id_token: token_resp.id_token,
            expires_at,
        });

        Ok(id_token)
    }

    pub async fn get_user_id(&self) -> Result<String> {
        let token = self.get_id_token().await?;
        // Decode the JWT payload (middle part) to get the user ID
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(anyhow!("Invalid JWT format"));
        }

        // Add padding if needed for base64
        let payload = parts[1];
        let padded = match payload.len() % 4 {
            2 => format!("{}==", payload),
            3 => format!("{}=", payload),
            _ => payload.to_string(),
        };

        let decoded = base64_decode(&padded)?;
        let claims: serde_json::Value = serde_json::from_slice(&decoded)?;
        claims["user_id"]
            .as_str()
            .or_else(|| claims["sub"].as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("No user_id or sub claim in token"))
    }
}

fn base64_decode(input: &str) -> Result<Vec<u8>> {
    // URL-safe base64 decode without pulling in a base64 crate
    let input = input.replace('-', "+").replace('_', "/");
    let mut result = Vec::new();
    let chars: Vec<u8> = input.bytes().collect();

    let decode_char = |c: u8| -> Result<u8> {
        match c {
            b'A'..=b'Z' => Ok(c - b'A'),
            b'a'..=b'z' => Ok(c - b'a' + 26),
            b'0'..=b'9' => Ok(c - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            b'=' => Ok(0),
            _ => Err(anyhow!("Invalid base64 character: {}", c as char)),
        }
    };

    for chunk in chars.chunks(4) {
        if chunk.len() < 4 {
            break;
        }
        let b0 = decode_char(chunk[0])?;
        let b1 = decode_char(chunk[1])?;
        let b2 = decode_char(chunk[2])?;
        let b3 = decode_char(chunk[3])?;

        result.push((b0 << 2) | (b1 >> 4));
        if chunk[2] != b'=' {
            result.push((b1 << 4) | (b2 >> 2));
        }
        if chunk[3] != b'=' {
            result.push((b2 << 6) | b3);
        }
    }

    Ok(result)
}
