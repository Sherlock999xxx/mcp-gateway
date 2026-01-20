//! Manual HTTP backend implementation.
//!
//! This backend exposes declaratively configured HTTP tools (no `OpenAPI` spec).
//!
//! NOTE: the HTTP DSL + execution logic is implemented in the shared crate
//! `unrelated-http-tools`. The Adapter backend is a thin wrapper that preserves
//! the existing Adapter behavior and `Backend` trait integration.

use crate::backend::{Backend, BackendState, BackendStatus, BackendType, ToolInfo};
use crate::config::HttpServerConfig;
use crate::error::{AdapterError, Result};
use async_trait::async_trait;
use parking_lot::RwLock;
use rmcp::model::{CallToolResult, GetPromptResult, ReadResourceResult};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use unrelated_http_tools::runtime::{HttpToolSource, HttpToolsError};

pub struct HttpBackend {
    name: String,
    config: HttpServerConfig,
    state: Arc<RwLock<BackendState>>,
    default_timeout: Duration,
    source: Arc<RwLock<Option<HttpToolSource>>>,
}

impl HttpBackend {
    #[must_use]
    pub fn new(name: String, config: HttpServerConfig, default_timeout: Duration) -> Self {
        Self {
            name,
            config,
            state: Arc::new(RwLock::new(BackendState::Dead)),
            default_timeout,
            source: Arc::new(RwLock::new(None)),
        }
    }
}

fn map_http_tools_error(e: HttpToolsError) -> AdapterError {
    match e {
        HttpToolsError::Config(s) => AdapterError::Config(s),
        HttpToolsError::Runtime(s) => AdapterError::Runtime(s),
        HttpToolsError::Http(s) | HttpToolsError::Transport(s) => AdapterError::Http(s),
    }
}

#[async_trait]
impl Backend for HttpBackend {
    fn name(&self) -> &str {
        &self.name
    }

    fn backend_type(&self) -> BackendType {
        BackendType::Http
    }

    fn state(&self) -> BackendState {
        *self.state.read()
    }

    fn status(&self) -> BackendStatus {
        let tool_count = self
            .source
            .read()
            .as_ref()
            .map(HttpToolSource::list_tools)
            .map_or(0, |t| t.len());

        BackendStatus {
            name: self.name.clone(),
            backend_type: BackendType::Http,
            state: self.state(),
            tool_count,
            spec_url: None,
            restart_count: 0,
            last_restart: None,
        }
    }

    async fn list_tools(&self) -> Result<Vec<ToolInfo>> {
        let Some(source) = self.source.read().clone() else {
            return Ok(Vec::new());
        };

        Ok(source
            .list_tools()
            .into_iter()
            .map(|t| {
                let name = t.name.to_string();
                ToolInfo {
                    name: name.clone(),
                    original_name: name,
                    description: t.description.as_deref().map(str::to_string),
                    input_schema: Value::Object(t.input_schema.as_ref().clone()),
                    output_schema: t
                        .output_schema
                        .as_ref()
                        .map(|s| Value::Object(s.as_ref().clone())),
                    annotations: t.annotations.clone(),
                }
            })
            .collect())
    }

    async fn call_tool(
        &self,
        _session_id: Option<&str>,
        name: &str,
        arguments: Value,
        timeout: Option<Duration>,
    ) -> Result<CallToolResult> {
        let Some(source) = self.source.read().clone() else {
            return Err(AdapterError::Runtime(format!(
                "HTTP backend '{}' is not started",
                self.name
            )));
        };

        let fut = source.call_tool(name, arguments);
        if let Some(t) = timeout.filter(|t| *t > Duration::from_millis(0)) {
            match tokio::time::timeout(t, fut).await {
                Ok(r) => r.map_err(map_http_tools_error),
                Err(_) => Err(AdapterError::Runtime(format!(
                    "Tool call timed out after {}ms",
                    t.as_millis()
                ))),
            }
        } else {
            fut.await.map_err(map_http_tools_error)
        }
    }

    async fn list_resources(&self) -> Result<Vec<crate::backend::ResourceInfo>> {
        Ok(Vec::new())
    }

    async fn read_resource(
        &self,
        _session_id: Option<&str>,
        _uri: &str,
    ) -> Result<ReadResourceResult> {
        Err(AdapterError::Runtime(
            "HTTP backend does not support resources".to_string(),
        ))
    }

    async fn list_prompts(&self) -> Result<Vec<crate::backend::PromptInfo>> {
        Ok(Vec::new())
    }

    async fn get_prompt(
        &self,
        _session_id: Option<&str>,
        _name: &str,
        _arguments: Option<serde_json::Map<String, Value>>,
    ) -> Result<GetPromptResult> {
        Err(AdapterError::Runtime(
            "HTTP backend does not support prompts".to_string(),
        ))
    }

    async fn start(&self) -> Result<()> {
        *self.state.write() = BackendState::Starting;

        let src = HttpToolSource::new(self.name.clone(), self.config.clone(), self.default_timeout)
            .map_err(map_http_tools_error)?;

        *self.source.write() = Some(src);
        *self.state.write() = BackendState::Running;
        Ok(())
    }

    async fn shutdown(&self) {
        *self.state.write() = BackendState::Dead;
        *self.source.write() = None;
    }
}
