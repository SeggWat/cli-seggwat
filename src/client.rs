//! HTTP client for communicating with the SeggWat API.

use std::time::Duration;

use reqwest::{Client, StatusCode};

use crate::error::CliError;

/// Build an HTTP client with standard timeout and user-agent.
pub(crate) fn build_http_client() -> Result<Client, CliError> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(format!("seggwat-cli/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| CliError::ClientBuild(e.to_string()))
}
use crate::models::{
    ApiError, Feedback, FeedbackCounts, FeedbackCreateRequest, FeedbackListResponse,
    FeedbackUpdateRequest, HelpfulStats, NpsStats, Project, ProjectListResponse, ProjectSummary,
    Rating, RatingListResponse, StarStats, WhoamiResponse,
};

/// Authentication method for the client.
#[derive(Clone)]
enum AuthMethod {
    ApiKey(String),
    Bearer(String),
}

/// Client for the SeggWat REST API.
#[derive(Clone)]
pub struct SeggwatClient {
    client: Client,
    base_url: String,
    auth: AuthMethod,
}

impl SeggwatClient {
    /// Create a new client authenticated with an API key.
    ///
    /// `base_url` should be the root URL (e.g. `https://seggwat.com`).
    /// `/api/v1` is appended automatically.
    pub fn with_api_key(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Result<Self, CliError> {
        let base = base_url.into();
        let base = base.trim_end_matches('/');
        let client = Self::build_http_client()?;
        Ok(Self {
            client,
            base_url: format!("{base}/api/v1"),
            auth: AuthMethod::ApiKey(api_key.into()),
        })
    }

    /// Create a new client authenticated with a Bearer token.
    pub fn with_bearer_token(
        base_url: impl Into<String>,
        token: impl Into<String>,
    ) -> Result<Self, CliError> {
        let base = base_url.into();
        let base = base.trim_end_matches('/');
        let client = Self::build_http_client()?;
        Ok(Self {
            client,
            base_url: format!("{base}/api/v1"),
            auth: AuthMethod::Bearer(token.into()),
        })
    }

    fn build_http_client() -> Result<Client, CliError> {
        build_http_client()
    }

    async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T, CliError> {
        let status = response.status();
        if status.is_success() {
            response
                .json()
                .await
                .map_err(|e| CliError::ParseError(e.to_string()))
        } else {
            Err(self.map_error(status, response).await)
        }
    }

    async fn handle_no_content_response(
        &self,
        response: reqwest::Response,
    ) -> Result<(), CliError> {
        let status = response.status();
        if status == StatusCode::NO_CONTENT || status.is_success() {
            Ok(())
        } else {
            Err(self.map_error(status, response).await)
        }
    }

    async fn map_error(&self, status: StatusCode, response: reqwest::Response) -> CliError {
        let error: ApiError = response.json().await.unwrap_or_else(|_| ApiError {
            error: "unknown".to_string(),
            message: format!("HTTP {status}"),
        });
        match status {
            StatusCode::UNAUTHORIZED => CliError::Unauthorized {
                message: error.message,
            },
            StatusCode::FORBIDDEN => CliError::Forbidden {
                message: error.message,
            },
            StatusCode::NOT_FOUND => CliError::NotFound {
                message: error.message,
            },
            StatusCode::BAD_REQUEST => CliError::BadRequest {
                message: error.message,
            },
            _ => CliError::ServerError {
                status: status.as_u16(),
                message: error.message,
            },
        }
    }

    fn auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.auth {
            AuthMethod::ApiKey(key) => req.header("X-API-Key", key),
            AuthMethod::Bearer(token) => req.header("Authorization", format!("Bearer {token}")),
        }
    }

    // ========================================================================
    // Projects
    // ========================================================================

    pub async fn list_projects(&self) -> Result<ProjectListResponse, CliError> {
        let url = format!("{}/projects", self.base_url);
        let resp = self.auth(self.client.get(&url)).send().await?;
        self.handle_response(resp).await
    }

    pub async fn get_project(&self, project_id: &str) -> Result<Project, CliError> {
        let url = format!("{}/projects/{project_id}", self.base_url);
        let resp = self.auth(self.client.get(&url)).send().await?;
        self.handle_response(resp).await
    }

    pub async fn get_project_summary(&self, project_id: &str) -> Result<ProjectSummary, CliError> {
        let url = format!("{}/projects/{project_id}/summary", self.base_url);
        let resp = self.auth(self.client.get(&url)).send().await?;
        self.handle_response(resp).await
    }

    // ========================================================================
    // Feedback
    // ========================================================================

    pub async fn list_feedback(
        &self,
        project_id: &str,
        page: u64,
        limit: u64,
        status: Option<&str>,
        feedback_type: Option<&str>,
        search: Option<&str>,
    ) -> Result<FeedbackListResponse, CliError> {
        let url = format!("{}/projects/{project_id}/feedback", self.base_url);
        let mut req = self.auth(self.client.get(&url));

        req = req
            .query(&[("page", page.to_string().as_str())])
            .query(&[("limit", limit.to_string().as_str())]);

        if let Some(s) = status {
            req = req.query(&[("status", s)]);
        }
        if let Some(t) = feedback_type {
            req = req.query(&[("type", t)]);
        }
        if let Some(s) = search {
            req = req.query(&[("search", s)]);
        }

        let resp = req.send().await?;
        self.handle_response(resp).await
    }

    pub async fn get_feedback(
        &self,
        project_id: &str,
        feedback_id: &str,
    ) -> Result<Feedback, CliError> {
        let url = format!(
            "{}/projects/{project_id}/feedback/{feedback_id}",
            self.base_url
        );
        let resp = self.auth(self.client.get(&url)).send().await?;
        self.handle_response(resp).await
    }

    pub async fn create_feedback(
        &self,
        project_id: &str,
        body: &FeedbackCreateRequest,
    ) -> Result<Feedback, CliError> {
        let url = format!("{}/projects/{project_id}/feedback", self.base_url);
        let resp = self.auth(self.client.post(&url)).json(body).send().await?;
        self.handle_response(resp).await
    }

    pub async fn update_feedback(
        &self,
        project_id: &str,
        feedback_id: &str,
        body: &FeedbackUpdateRequest,
    ) -> Result<Feedback, CliError> {
        let url = format!(
            "{}/projects/{project_id}/feedback/{feedback_id}",
            self.base_url
        );
        let resp = self.auth(self.client.patch(&url)).json(body).send().await?;
        self.handle_response(resp).await
    }

    pub async fn delete_feedback(
        &self,
        project_id: &str,
        feedback_id: &str,
    ) -> Result<(), CliError> {
        let url = format!(
            "{}/projects/{project_id}/feedback/{feedback_id}",
            self.base_url
        );
        let resp = self.auth(self.client.delete(&url)).send().await?;
        self.handle_no_content_response(resp).await
    }

    pub async fn get_feedback_stats(&self, project_id: &str) -> Result<FeedbackCounts, CliError> {
        let url = format!("{}/projects/{project_id}/feedback/stats", self.base_url);
        let resp = self.auth(self.client.get(&url)).send().await?;
        self.handle_response(resp).await
    }

    // ========================================================================
    // Ratings
    // ========================================================================

    pub async fn list_ratings(
        &self,
        project_id: &str,
        page: u64,
        limit: u64,
        rating_type: Option<&str>,
        path: Option<&str>,
    ) -> Result<RatingListResponse, CliError> {
        let url = format!("{}/projects/{project_id}/ratings", self.base_url);
        let mut req = self.auth(self.client.get(&url));

        req = req
            .query(&[("page", page.to_string().as_str())])
            .query(&[("limit", limit.to_string().as_str())]);

        if let Some(t) = rating_type {
            req = req.query(&[("type", t)]);
        }
        if let Some(p) = path {
            req = req.query(&[("path", p)]);
        }

        let resp = req.send().await?;
        self.handle_response(resp).await
    }

    pub async fn get_rating(&self, project_id: &str, rating_id: &str) -> Result<Rating, CliError> {
        let url = format!(
            "{}/projects/{project_id}/ratings/{rating_id}",
            self.base_url
        );
        let resp = self.auth(self.client.get(&url)).send().await?;
        self.handle_response(resp).await
    }

    pub async fn delete_rating(&self, project_id: &str, rating_id: &str) -> Result<(), CliError> {
        let url = format!(
            "{}/projects/{project_id}/ratings/{rating_id}",
            self.base_url
        );
        let resp = self.auth(self.client.delete(&url)).send().await?;
        self.handle_no_content_response(resp).await
    }

    pub async fn get_helpful_stats(&self, project_id: &str) -> Result<HelpfulStats, CliError> {
        let url = format!("{}/projects/{project_id}/ratings/stats", self.base_url);
        let resp = self.auth(self.client.get(&url)).send().await?;
        self.handle_response(resp).await
    }

    pub async fn get_star_stats(&self, project_id: &str) -> Result<StarStats, CliError> {
        let url = format!("{}/projects/{project_id}/ratings/stats/star", self.base_url);
        let resp = self.auth(self.client.get(&url)).send().await?;
        self.handle_response(resp).await
    }

    pub async fn get_nps_stats(&self, project_id: &str) -> Result<NpsStats, CliError> {
        let url = format!("{}/projects/{project_id}/ratings/stats/nps", self.base_url);
        let resp = self.auth(self.client.get(&url)).send().await?;
        self.handle_response(resp).await
    }

    // ========================================================================
    // User
    // ========================================================================

    pub async fn whoami(&self) -> Result<WhoamiResponse, CliError> {
        let url = format!("{}/me", self.base_url);
        let resp = self.auth(self.client.get(&url)).send().await?;
        self.handle_response(resp).await
    }
}
