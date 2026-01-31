//! GitHub API interactions for token validation.
//!
//! This module provides lightweight validation functions for GitHub tokens:
//! - `validate_github_user`: Validates a PAT via GET /user (cheap, no AI tokens)

use serde::Deserialize;
use thiserror::Error;

/// GitHub API base URL
const GITHUB_API_BASE: &str = "https://api.github.com";

/// User-Agent header required by GitHub API
const USER_AGENT: &str = "binnacle-cli";

/// Errors that can occur during GitHub token validation.
#[derive(Debug, Error)]
pub enum TokenValidationError {
    /// Token is invalid or expired (401 Unauthorized)
    #[error("Invalid or expired token: GitHub returned 401 Unauthorized")]
    Unauthorized,

    /// Token lacks required permissions (403 Forbidden)
    #[error("Token lacks required permissions: GitHub returned 403 Forbidden")]
    Forbidden,

    /// Network or other HTTP error
    #[error("HTTP request failed: {0}")]
    HttpError(String),

    /// Failed to parse response
    #[error("Failed to parse GitHub response: {0}")]
    ParseError(String),
}

/// Response from GitHub GET /user endpoint (only fields we care about).
#[derive(Debug, Deserialize)]
pub struct GitHubUser {
    /// GitHub login/username
    pub login: String,
    /// User ID
    pub id: u64,
    /// Display name (optional)
    pub name: Option<String>,
}

/// Result of successful token validation.
#[derive(Debug)]
pub struct TokenValidationResult {
    /// The authenticated GitHub user
    pub user: GitHubUser,
    /// Original token (for storage)
    pub token: String,
}

/// Validate a GitHub token via the GET /user endpoint.
///
/// This is a lightweight validation that confirms the token is valid without
/// burning any AI tokens. It's suitable for CI/automated setups.
///
/// # Arguments
/// * `token` - GitHub Personal Access Token to validate
///
/// # Returns
/// * `Ok(TokenValidationResult)` - Token is valid, includes user info
/// * `Err(TokenValidationError)` - Token is invalid or request failed
///
/// # Example
/// ```ignore
/// use binnacle::github::validate_github_user;
///
/// match validate_github_user("ghp_xxxxxxxxxxxx") {
///     Ok(result) => println!("Authenticated as {}", result.user.login),
///     Err(e) => eprintln!("Token validation failed: {}", e),
/// }
/// ```
pub fn validate_github_user(token: &str) -> Result<TokenValidationResult, TokenValidationError> {
    let url = format!("{}/user", GITHUB_API_BASE);

    let response = ureq::get(&url)
        .set("Authorization", &format!("Bearer {}", token))
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", USER_AGENT)
        .set("X-GitHub-Api-Version", "2022-11-28")
        .call();

    match response {
        Ok(resp) => {
            let user: GitHubUser = resp
                .into_json()
                .map_err(|e| TokenValidationError::ParseError(e.to_string()))?;

            Ok(TokenValidationResult {
                user,
                token: token.to_string(),
            })
        }
        Err(ureq::Error::Status(401, _)) => Err(TokenValidationError::Unauthorized),
        Err(ureq::Error::Status(403, _)) => Err(TokenValidationError::Forbidden),
        Err(ureq::Error::Status(code, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            Err(TokenValidationError::HttpError(format!(
                "HTTP {}: {}",
                code, body
            )))
        }
        Err(e) => Err(TokenValidationError::HttpError(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_token_returns_unauthorized() {
        // Using an obviously invalid token should return 401
        let result = validate_github_user("invalid_token_12345");

        assert!(result.is_err());
        match result.unwrap_err() {
            TokenValidationError::Unauthorized => {} // Expected
            TokenValidationError::Forbidden => {}    // Also acceptable
            other => panic!("Expected Unauthorized or Forbidden, got: {:?}", other),
        }
    }

    #[test]
    fn test_empty_token_returns_error() {
        let result = validate_github_user("");

        assert!(result.is_err());
        // Empty token should result in 401 or similar
    }

    #[test]
    fn test_github_user_deserialize() {
        let json = r#"{
            "login": "testuser",
            "id": 12345,
            "name": "Test User"
        }"#;

        let user: GitHubUser = serde_json::from_str(json).unwrap();
        assert_eq!(user.login, "testuser");
        assert_eq!(user.id, 12345);
        assert_eq!(user.name, Some("Test User".to_string()));
    }

    #[test]
    fn test_github_user_deserialize_without_name() {
        let json = r#"{
            "login": "testuser",
            "id": 12345
        }"#;

        let user: GitHubUser = serde_json::from_str(json).unwrap();
        assert_eq!(user.login, "testuser");
        assert_eq!(user.id, 12345);
        assert!(user.name.is_none());
    }
}
