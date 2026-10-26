//! Tool schemas derive from the typed argument structs via rmcp's `#[tool]`
//! macro; `serde(rename)` keeps wire names like `diagramId`.

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig,
};
use rmcp::{ErrorData as McpError, ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

use crate::staruml::Client;
use crate::{SERVER_NAME, SERVER_VERSION};

#[derive(Clone)]
pub struct StarUmlTools {
    client: Client,
    tool_router: ToolRouter<Self>,
}

#[derive(Deserialize, JsonSchema)]
pub struct GenerateDiagramArgs {
    #[schemars(description = "Mermaid code to generate the diagram. Supported diagrams are classDiagram, sequenceDiagram, flowchart, erDiagram, mindmap, requirementDiagram and stateDiagram. Other diagrams are not supported.")]
    pub code: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct DiagramImageArgs {
    #[serde(rename = "diagramId")]
    #[schemars(description = "ID of the diagram to get the image for. You can get the ID from the 'get_all_diagrams_info' tool.")]
    pub diagram_id: String,
}

#[tool_router]
impl StarUmlTools {
    pub fn new(client: Client) -> Self {
        Self { client, tool_router: Self::tool_router() }
    }

    #[tool(description = "Generate a diagram in StarUML.")]
    async fn generate_diagram(
        &self,
        Parameters(args): Parameters<GenerateDiagramArgs>,
    ) -> Result<CallToolResult, McpError> {
        match self.client.call("/generate_diagram", &json!({ "code": args.code })).await {
            Ok(_) => Ok(text("Diagram is successfully generated.")),
            Err(err) => Ok(failure(err)),
        }
    }

    #[tool(description = "Get information for all diagrams in StarUML.")]
    async fn get_all_diagrams_info(&self) -> Result<CallToolResult, McpError> {
        match self.client.call("/get_all_diagrams_info", &json!({})).await {
            Ok(data) => Ok(text(format!("All diagrams: {data}"))),
            Err(err) => Ok(failure(err)),
        }
    }

    #[tool(description = "Get information for the current diagram in StarUML.")]
    async fn get_current_diagram_info(&self) -> Result<CallToolResult, McpError> {
        let body = match self.client.call("/get_current_diagram_info", &json!({})).await {
            Ok(data) if data.is_null() => "No current diagram found.".to_string(),
            Ok(data) => format!("Current diagram: {data}"),
            Err(err) => return Ok(failure(err)),
        };
        Ok(text(body))
    }

    #[tool(description = "Get the image of a diagram by its ID in StarUML.")]
    async fn get_diagram_image_by_id(
        &self,
        Parameters(args): Parameters<DiagramImageArgs>,
    ) -> Result<CallToolResult, McpError> {
        let data = match self
            .client
            .call(
                "/get_diagram_image_by_id",
                &json!({ "diagramId": args.diagram_id }),
            )
            .await
        {
            Ok(data) => data,
            Err(err) => return Ok(failure(err)),
        };
        match data.as_str() {
            Some(base64) => Ok(CallToolResult::success(vec![ContentBlock::image(
                base64,
                "image/png",
            )])),
            None => Ok(failure("expected base64 image string from StarUML")),
        }
    }
}

/// Domain failures become `isError` results so the LLM sees the reason;
/// protocol errors stay reserved for protocol problems.
fn failure(err: impl std::fmt::Display) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(err.to_string())])
}

fn text(content: impl Into<String>) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(content)])
}

// Route through the field built once in `new` — the macro default
// (`Self::tool_router()`) rebuilds the router on every list/call.
#[tool_handler(router = self.tool_router)]
impl ServerHandler for StarUmlTools {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions("Tools proxy StarUML v7's built-in API server; StarUML must be running.")
            .with_server_info(Implementation::new(SERVER_NAME, SERVER_VERSION))
    }
}
