use crate::discovery;
use rmcp::{
    ServerHandler,
    model::{Implementation, ServerCapabilities, ServerConfig},
    tool, tool_handler, tool_router,
};

#[derive(Debug, Clone)]
pub struct KicadMcp;

#[tool_router]
impl KicadMcp {
    #[tool(
        description = "Report KiCad IPC discovery and safety status without exposing the API token"
    )]
    fn kicad_status(&self) -> String {
        serde_json::to_string_pretty(&discovery::discover())
            .unwrap_or_else(|error| format!(r#"{{"error":"{error}"}}"#))
    }

    #[tool(description = "Report this server's implementation version and current scope")]
    fn server_info(&self) -> String {
        serde_json::json!({
            "name": env!("CARGO_PKG_NAME"),
            "version": env!("CARGO_PKG_VERSION"),
            "transport": "stdio",
            "scope": "scaffold: discovery only; KiCad IPC transport arrives in M1",
            "write_default": "disabled"
        })
        .to_string()
    }
}

#[tool_handler]
impl ServerHandler for KicadMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
                    .with_title("KiCad MCP")
                    .with_description(env!("CARGO_PKG_DESCRIPTION"))
                    .with_website_url(env!("CARGO_PKG_REPOSITORY")),
            )
            .with_instructions(
                "Local-first KiCad MCP server. This scaffold provides discovery only and never mutates a project."
                    .to_string(),
            )
    }
}
