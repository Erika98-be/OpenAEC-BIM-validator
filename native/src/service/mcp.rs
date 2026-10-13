use super::{tools, Service};
use rmcp::{model::*, service::RequestContext, ErrorData, RoleServer, ServerHandler, ServiceExt};
use serde_json::{json, Value};
#[derive(Clone)]
pub struct Mcp(pub Service);
impl ServerHandler for Mcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(Implementation::new("openaec-bim-validator",env!("CARGO_PKG_VERSION"))).with_instructions("Native IFC analysis. Access is limited to already loaded models. Export artifacts are transient. view_select only works when attached to the desktop; wait for view_state to confirm application. IDS support is partial.")
    }
    fn get_tool(&self, name: &str) -> Option<Tool> {
        tools()
            .into_iter()
            .find(|t| t["name"] == name)
            .and_then(|t| serde_json::from_value(t).ok())
    }
    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        if request.is_some_and(|r| r.cursor.is_some()) {
            return Err(ErrorData::invalid_params(
                "No pagination cursor is supported",
                None,
            ));
        }
        serde_json::from_value(json!({"tools":tools()}))
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        if self.get_tool(&request.name).is_none() {
            return Err(ErrorData::invalid_params("Unknown tool", None));
        }
        let args = Value::Object(request.arguments.unwrap_or_default());
        let result = match self.0.call(request.name.to_string(), args).await {
            Ok(value) => CallToolResult::structured(value),
            Err(error) => CallToolResult::structured_error(
                json!({"error":error.message,"status":error.status}),
            ),
        };
        Ok(result.into())
    }
}
pub async fn stdio(service: Service) -> Result<(), String> {
    let server = Mcp(service)
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|e| e.to_string())?;
    server.waiting().await.map_err(|e| e.to_string())?;
    Ok(())
}
