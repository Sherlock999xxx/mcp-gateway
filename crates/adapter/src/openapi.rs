//! `OpenAPI` backend implementation.
//!
//! This backend converts `OpenAPI` operations into MCP tools and executes outbound HTTP requests
//! for `tools/call`.
//!
//! NOTE: the `OpenAPI` resolver + tool discovery + execution logic is implemented in the shared
//! crate `unrelated-openapi-tools`. The Adapter backend is a thin wrapper that preserves the
//! existing Adapter behavior and `Backend` trait integration.

use crate::backend::{Backend, BackendState, BackendStatus, BackendType, ToolInfo};
use crate::config::ApiServerConfig;
use crate::error::{AdapterError, Result};
use async_trait::async_trait;
use parking_lot::RwLock;
use rmcp::model::{CallToolResult, GetPromptResult, ReadResourceResult};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use unrelated_openapi_tools::error::OpenApiToolsError;
use unrelated_openapi_tools::runtime::OpenApiToolSource;

pub struct OpenApiBackend {
    name: String,
    config: ApiServerConfig,
    state: Arc<RwLock<BackendState>>,
    source: OpenApiToolSource,
}

impl OpenApiBackend {
    #[must_use]
    pub fn new(
        name: String,
        config: ApiServerConfig,
        default_timeout: Duration,
        startup_timeout: Duration,
        probe_enabled: bool,
        probe_timeout: Duration,
    ) -> Self {
        let source = OpenApiToolSource::new(
            name.clone(),
            config.clone(),
            default_timeout,
            startup_timeout,
            probe_enabled,
            probe_timeout,
        );
        Self {
            name,
            config,
            state: Arc::new(RwLock::new(BackendState::Dead)),
            source,
        }
    }
}

fn map_openapi_tools_error(e: OpenApiToolsError) -> AdapterError {
    match e {
        OpenApiToolsError::Config(s) => AdapterError::Config(s),
        OpenApiToolsError::Startup(s) => AdapterError::Startup(s),
        OpenApiToolsError::Runtime(s) => AdapterError::Runtime(s),
        OpenApiToolsError::Http(s) => AdapterError::Http(s),
        OpenApiToolsError::OpenApi(s) => AdapterError::OpenApi(s),
        OpenApiToolsError::OpenApiSpecFetch { url, message } => {
            AdapterError::OpenApi(format!("failed to fetch spec from '{url}': {message}"))
        }
        OpenApiToolsError::OpenApiSpecReadBody { url, message } => {
            AdapterError::OpenApi(format!("failed to read spec body from '{url}': {message}"))
        }
        OpenApiToolsError::OpenApiSpecReadFile { path, source } => {
            AdapterError::OpenApi(format!("failed to read spec file '{path}': {source}"))
        }
        OpenApiToolsError::OpenApiSpecParse { location, source } => AdapterError::OpenApi(format!(
            "failed to parse OpenAPI spec from '{location}': {source}"
        )),
        OpenApiToolsError::ParamCollision(s) => AdapterError::ParamCollision(s),
        OpenApiToolsError::Io(e) => AdapterError::Io(e),
        OpenApiToolsError::Json(e) => AdapterError::Json(e),
        OpenApiToolsError::Yaml(e) => AdapterError::Yaml(e),
        OpenApiToolsError::Request(msg) => AdapterError::Http(msg),
    }
}

#[async_trait]
impl Backend for OpenApiBackend {
    fn name(&self) -> &str {
        &self.name
    }

    fn backend_type(&self) -> BackendType {
        BackendType::OpenApi
    }

    fn state(&self) -> BackendState {
        *self.state.read()
    }

    fn status(&self) -> BackendStatus {
        let tool_count = self.source.list_tools().len();
        BackendStatus {
            name: self.name.clone(),
            backend_type: BackendType::OpenApi,
            state: self.state(),
            tool_count,
            spec_url: Some(self.config.spec.clone()),
            restart_count: 0,
            last_restart: None,
        }
    }

    async fn list_tools(&self) -> Result<Vec<ToolInfo>> {
        Ok(self
            .source
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
        let fut = self.source.call_tool(name, arguments);
        if let Some(t) = timeout.filter(|t| *t > Duration::from_millis(0)) {
            match tokio::time::timeout(t, fut).await {
                Ok(r) => r.map_err(map_openapi_tools_error),
                Err(_) => Err(AdapterError::Runtime(format!(
                    "Tool call timed out after {}ms",
                    t.as_millis()
                ))),
            }
        } else {
            fut.await.map_err(map_openapi_tools_error)
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
            "OpenAPI backend does not support resources".to_string(),
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
            "OpenAPI backend does not support prompts".to_string(),
        ))
    }

    async fn start(&self) -> Result<()> {
        *self.state.write() = BackendState::Starting;
        match self.source.start().await {
            Ok(()) => {
                *self.state.write() = BackendState::Running;
                Ok(())
            }
            Err(e) => {
                *self.state.write() = BackendState::Dead;
                Err(map_openapi_tools_error(e))
            }
        }
    }

    async fn shutdown(&self) {
        *self.state.write() = BackendState::Dead;
    }
}
