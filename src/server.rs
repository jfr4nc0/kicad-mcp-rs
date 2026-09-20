use crate::kicad::{KiCadService, Point};
use rmcp::{
    ErrorData, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        GetPromptRequestParams, GetPromptResponse, GetPromptResult, Implementation,
        ListPromptsResult, ListResourcesResult, PaginatedRequestParams, Prompt, PromptArgument,
        PromptMessage, ReadResourceRequestParams, ReadResourceResponse, ReadResourceResult,
        Resource, ResourceContents, Role, ServerCapabilities, ServerConfig,
    },
    schemars,
    service::{RequestContext, RoleServer},
    tool, tool_handler, tool_router,
};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct PageRequest {
    #[serde(default)]
    cursor: usize,
    #[serde(default = "default_page_size")]
    limit: usize,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct FootprintPageRequest {
    #[serde(default)]
    cursor: usize,
    #[serde(default = "default_page_size")]
    limit: usize,
    /// Optional subset of summary fields. `id` and `revision` are always retained.
    #[serde(default)]
    fields: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct IdRequest {
    id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct MoveFootprintRequest {
    id: String,
    x_nm: i64,
    y_nm: i64,
    #[serde(default)]
    orientation_degrees: Option<f64>,
    expected_revision: String,
    #[serde(default = "default_dry_run")]
    dry_run: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SetPropertyRequest {
    id: String,
    property: String,
    value: String,
    expected_revision: String,
    #[serde(default = "default_dry_run")]
    dry_run: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CreateTrackRequest {
    start_x_nm: i64,
    start_y_nm: i64,
    end_x_nm: i64,
    end_y_nm: i64,
    width_nm: i64,
    /// Numeric KiCad BoardLayer enum value (for example 3 for F.Cu).
    layer: i32,
    #[serde(default)]
    net_name: String,
    #[serde(default = "default_dry_run")]
    dry_run: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct UpdateTrackRequest {
    id: String,
    start_x_nm: i64,
    start_y_nm: i64,
    end_x_nm: i64,
    end_y_nm: i64,
    width_nm: i64,
    layer: i32,
    #[serde(default)]
    net_name: String,
    expected_revision: String,
    #[serde(default = "default_dry_run")]
    dry_run: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CreateViaRequest {
    x_nm: i64,
    y_nm: i64,
    diameter_nm: i64,
    drill_nm: i64,
    /// Numeric KiCad BoardLayer enum value.
    start_layer: i32,
    /// Numeric KiCad BoardLayer enum value.
    end_layer: i32,
    #[serde(default)]
    net_name: String,
    #[serde(default = "default_dry_run")]
    dry_run: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct DeleteItemRequest {
    id: String,
    expected_revision: String,
    #[serde(default = "default_dry_run")]
    dry_run: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct PlaceSymbolRequest {
    library_nickname: String,
    entry_name: String,
    x_nm: i64,
    y_nm: i64,
    #[serde(default)]
    orientation: Option<i32>,
    #[serde(default)]
    reference: Option<String>,
    /// Canonical sheet KIID chain returned as `path_ids` by `schematic_hierarchy`.
    /// If omitted, KiCad's currently active sheet is used.
    #[serde(default)]
    sheet_path_ids: Option<Vec<String>>,
    #[serde(default = "default_dry_run")]
    dry_run: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct OutputRequest {
    /// Output path inside KICAD_MCP_PROJECT_ROOT.
    output: PathBuf,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ExportRequest {
    /// One of gerbers, pdf, svg, step, or vrml.
    format: String,
    /// Output path inside KICAD_MCP_PROJECT_ROOT.
    output: PathBuf,
}

#[derive(Debug, Clone)]
pub struct KicadMcp {
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
    kicad: KiCadService,
}

impl KicadMcp {
    pub fn new() -> Self {
        let kicad = KiCadService::from_env();
        Self {
            tool_router: Self::tool_router(),
            kicad,
        }
    }

    fn current_tool_router(&self) -> ToolRouter<Self> {
        let mut tool_router = self.tool_router.clone();
        if self
            .kicad
            .status()
            .version
            .is_none_or(|version| version.major < 11)
        {
            for name in [
                "schematic_hierarchy",
                "schematic_netlist",
                "place_symbol",
                "native_export_step",
            ] {
                tool_router.disable_route(name.to_string());
            }
        }
        tool_router
    }
}

#[tool_router]
impl KicadMcp {
    #[tool(
        description = "Report real KiCad IPC connectivity, version, capability gates, and safety state without exposing API tokens"
    )]
    fn kicad_status(&self) -> String {
        json(&self.kicad.status())
    }

    #[tool(
        description = "Report this server's implementation version and supported milestone scope"
    )]
    fn server_info(&self) -> String {
        serde_json::json!({
            "name": env!("CARGO_PKG_NAME"),
            "version": env!("CARGO_PKG_VERSION"),
            "transport": "stdio",
            "milestones_implemented": ["M0", "M1", "M2", "M3", "M4", "M5", "M6"],
            "verified": {
                "kicad_10_0_6_live": true,
                "kicad_11_live": false,
                "kicad_11_contract": "preview schemas; runtime-gated"
            },
            "ipc": "KiCad Protobuf over NNG REQ0",
            "writes": "disabled unless KICAD_MCP_ALLOW_WRITE=true and KICAD_MCP_PROJECT_ROOT is set",
            "save_policy": "explicit only"
        }).to_string()
    }

    #[tool(
        description = "Summarize the active PCB: document, footprint/track/via/zone counts, and net count"
    )]
    fn get_board_summary(&self) -> Result<String, String> {
        encode(self.kicad.board_summary())
    }

    #[tool(description = "Return project metadata for the active PCB or schematic")]
    fn get_active_project(&self) -> Result<String, String> {
        encode(self.kicad.active_project())
    }

    #[tool(
        description = "List footprints from the active PCB with bounded cursor pagination and revision hashes"
    )]
    fn list_footprints(
        &self,
        Parameters(request): Parameters<FootprintPageRequest>,
    ) -> Result<String, String> {
        encode_selected(
            self.kicad.list_footprints(request.cursor, request.limit),
            &request.fields,
            &[
                "id",
                "reference",
                "value",
                "library",
                "position_nm",
                "orientation_degrees",
                "layer",
                "revision",
            ],
        )
    }

    #[tool(description = "Get one footprint by KiCad KIID, including geometry and revision hash")]
    fn get_footprint(&self, Parameters(request): Parameters<IdRequest>) -> Result<String, String> {
        encode(self.kicad.get_footprint(&request.id))
    }

    #[tool(description = "List PCB nets with bounded cursor pagination")]
    fn list_nets(&self, Parameters(request): Parameters<PageRequest>) -> Result<String, String> {
        encode(self.kicad.list_nets(request.cursor, request.limit))
    }

    #[tool(description = "Return a bounded summary of items currently selected in PCB Editor")]
    fn get_selection(&self) -> Result<String, String> {
        encode(self.kicad.selection())
    }

    #[tool(
        description = "Run KiCad CLI DRC on the active PCB; project access is restricted to KICAD_MCP_PROJECT_ROOT"
    )]
    fn run_drc(&self) -> Result<String, String> {
        encode(self.kicad.run_drc())
    }

    #[tool(
        description = "Move/rotate a footprint using revision preconditions. Defaults to dry-run and never saves implicitly"
    )]
    fn move_footprint(
        &self,
        Parameters(request): Parameters<MoveFootprintRequest>,
    ) -> Result<String, String> {
        encode(self.kicad.move_footprint(
            &request.id,
            request.x_nm,
            request.y_nm,
            request.orientation_degrees,
            &request.expected_revision,
            request.dry_run,
        ))
    }

    #[tool(
        description = "Set a footprint reference/value/datasheet/description with revision preconditions. Defaults to dry-run"
    )]
    fn set_footprint_property(
        &self,
        Parameters(request): Parameters<SetPropertyRequest>,
    ) -> Result<String, String> {
        encode(self.kicad.set_footprint_property(
            &request.id,
            &request.property,
            &request.value,
            &request.expected_revision,
            request.dry_run,
        ))
    }

    #[tool(
        description = "Create a PCB track inside an undoable KiCad commit. Defaults to dry-run and does not save"
    )]
    fn create_track(
        &self,
        Parameters(request): Parameters<CreateTrackRequest>,
    ) -> Result<String, String> {
        encode(self.kicad.create_track(
            Point {
                x_nm: request.start_x_nm,
                y_nm: request.start_y_nm,
            },
            Point {
                x_nm: request.end_x_nm,
                y_nm: request.end_y_nm,
            },
            request.width_nm,
            request.layer,
            request.net_name,
            request.dry_run,
        ))
    }

    #[tool(
        description = "Update an existing PCB track with a revision precondition. Defaults to dry-run and does not save"
    )]
    fn update_track(
        &self,
        Parameters(request): Parameters<UpdateTrackRequest>,
    ) -> Result<String, String> {
        encode(self.kicad.update_track(
            &request.id,
            Point {
                x_nm: request.start_x_nm,
                y_nm: request.start_y_nm,
            },
            Point {
                x_nm: request.end_x_nm,
                y_nm: request.end_y_nm,
            },
            request.width_nm,
            request.layer,
            request.net_name,
            &request.expected_revision,
            request.dry_run,
        ))
    }

    #[tool(
        description = "Create a through via inside an undoable KiCad commit. Defaults to dry-run and does not save"
    )]
    fn create_via(
        &self,
        Parameters(request): Parameters<CreateViaRequest>,
    ) -> Result<String, String> {
        encode(self.kicad.create_via(
            Point {
                x_nm: request.x_nm,
                y_nm: request.y_nm,
            },
            request.diameter_nm,
            request.drill_nm,
            request.start_layer,
            request.end_layer,
            request.net_name,
            request.dry_run,
        ))
    }

    #[tool(
        description = "Delete an item using a revision precondition inside an undoable commit. Defaults to dry-run"
    )]
    fn delete_item(
        &self,
        Parameters(request): Parameters<DeleteItemRequest>,
    ) -> Result<String, String> {
        encode(
            self.kicad
                .delete_item(&request.id, &request.expected_revision, request.dry_run),
        )
    }

    #[tool(description = "Explicitly save the active PCB. No mutation tool saves automatically")]
    fn save_active_board(&self) -> Result<String, String> {
        encode(self.kicad.save_active_board())
    }

    #[tool(description = "Read the KiCad 11 schematic hierarchy through the official IPC API")]
    fn schematic_hierarchy(&self) -> Result<String, String> {
        encode(self.kicad.schematic_hierarchy())
    }

    #[tool(
        description = "Read the KiCad 11 schematic netlist through the official IPC API with bounded pagination"
    )]
    fn schematic_netlist(
        &self,
        Parameters(request): Parameters<PageRequest>,
    ) -> Result<String, String> {
        encode(self.kicad.schematic_netlist(request.cursor, request.limit))
    }

    #[tool(
        description = "Place a KiCad 11 schematic symbol through IPC. Requires write opt-in and defaults to dry-run"
    )]
    fn place_symbol(
        &self,
        Parameters(request): Parameters<PlaceSymbolRequest>,
    ) -> Result<String, String> {
        encode(self.kicad.place_symbol(
            request.library_nickname,
            request.entry_name,
            Point {
                x_nm: request.x_nm,
                y_nm: request.y_nm,
            },
            request.orientation,
            request.reference,
            request.sheet_path_ids,
            request.dry_run,
        ))
    }

    #[tool(
        description = "Run the native KiCad 11 IPC 3D export job for STEP output under the approved project root"
    )]
    fn native_export_step(
        &self,
        Parameters(request): Parameters<OutputRequest>,
    ) -> Result<String, String> {
        encode(self.kicad.native_export_step(&request.output))
    }

    #[tool(description = "Run KiCad CLI ERC on the active schematic with project-root confinement")]
    fn run_erc(&self) -> Result<String, String> {
        encode(self.kicad.run_erc())
    }

    #[tool(
        description = "Export the active PCB using KiCad CLI to an output path under the approved project root"
    )]
    fn export_board(
        &self,
        Parameters(request): Parameters<ExportRequest>,
    ) -> Result<String, String> {
        encode(self.kicad.export_board(&request.format, &request.output))
    }
}

#[tool_handler(router = self.current_tool_router())]
impl ServerHandler for KicadMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
        )
            .with_server_info(
                Implementation::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
                    .with_title("KiCad MCP")
                    .with_description(env!("CARGO_PKG_DESCRIPTION"))
                    .with_website_url(env!("CARGO_PKG_REPOSITORY")),
            )
            .with_instructions(
                "Local-first KiCad MCP over the official IPC API. Read tools are bounded. Writes require explicit opt-in, project-root confinement, revision preconditions, and default to dry-run; saving is always explicit."
                    .to_string(),
            )
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourcesResult, ErrorData>> + Send + '_ {
        let mut resources = vec![
            Resource::new("kicad://status", "status")
                .with_title("KiCad connection status")
                .with_description("Live IPC connectivity, version, and capability gates")
                .with_mime_type("application/json"),
            Resource::new("kicad://active-board/summary", "active-board-summary")
                .with_title("Active PCB summary")
                .with_description("Counts and identity for the PCB open in PCB Editor")
                .with_mime_type("application/json"),
            Resource::new("kicad://active-project", "active-project")
                .with_title("Active project metadata")
                .with_description("Project identity from the active PCB or schematic")
                .with_mime_type("application/json"),
            Resource::new("kicad://drc/latest", "latest-drc")
                .with_title("Latest DRC report")
                .with_description("Most recent bounded DRC result produced by run_drc")
                .with_mime_type("application/json"),
            Resource::new("kicad://capabilities", "capabilities")
                .with_title("Server and KiCad capabilities")
                .with_description("Version-gated feature and mutation availability")
                .with_mime_type("application/json"),
            Resource::new("kicad://safety-policy", "safety-policy")
                .with_title("Mutation safety policy")
                .with_description("Write opt-in, project confinement, revision and save rules")
                .with_mime_type("text/markdown"),
        ];
        if self
            .kicad
            .status()
            .version
            .is_some_and(|version| version.major >= 11)
        {
            resources.push(
                Resource::new("kicad://active-schematic/hierarchy", "schematic-hierarchy")
                    .with_title("KiCad 11 schematic hierarchy")
                    .with_description("Sheet hierarchy from the KiCad 11 IPC API")
                    .with_mime_type("application/json"),
            );
        }
        std::future::ready(Ok(ListResourcesResult::with_all_items(resources)))
    }

    fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ReadResourceResponse, ErrorData>> + Send + '_ {
        let uri = request.uri;
        let mime_type = if uri.ends_with("policy") {
            "text/markdown"
        } else {
            "application/json"
        };
        let text = match uri.as_str() {
            "kicad://status" => json(&self.kicad.status()),
            "kicad://active-board/summary" => resource_result(self.kicad.board_summary()),
            "kicad://active-project" => resource_result(self.kicad.active_project()),
            "kicad://drc/latest" => json(&self.kicad.latest_drc()),
            "kicad://capabilities" => json(&self.kicad.status().capabilities),
            "kicad://active-schematic/hierarchy" => {
                resource_result(self.kicad.schematic_hierarchy())
            }
            "kicad://safety-policy" => SAFETY_POLICY.to_string(),
            _ => {
                return std::future::ready(Err(ErrorData::invalid_params(
                    format!("unknown resource URI: {uri}"),
                    None,
                )));
            }
        };
        let result = ReadResourceResult::new(vec![
            ResourceContents::text(text, uri).with_mime_type(mime_type),
        ]);
        std::future::ready(Ok(result.into()))
    }

    fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListPromptsResult, ErrorData>> + Send + '_ {
        let goal = PromptArgument::new("goal")
            .with_description("Desired PCB change")
            .with_required(true);
        let mut prompts = vec![
            Prompt::new(
                "inspect-board",
                Some("Inspect the active PCB without modifying it"),
                None,
            ),
            Prompt::new(
                "safe-board-change",
                Some("Plan and execute a guarded PCB change"),
                Some(vec![goal]),
            ),
            Prompt::new(
                "validate-design",
                Some("Run DRC/ERC and summarize actionable findings"),
                None,
            ),
            Prompt::new(
                "review-footprint-placement",
                Some("Review PCB footprint placement without changing it"),
                None,
            ),
            Prompt::new(
                "review-routing",
                Some("Review tracks, vias, nets, and DRC routing findings"),
                None,
            ),
            Prompt::new(
                "manufacturing-preflight",
                Some("Perform a read-only manufacturing preflight"),
                None,
            ),
        ];
        if self
            .kicad
            .status()
            .version
            .is_some_and(|version| version.major >= 11)
        {
            prompts.push(Prompt::new(
                "inspect-schematic",
                Some("Inspect KiCad 11 schematic hierarchy and netlist"),
                None,
            ));
        }
        std::future::ready(Ok(ListPromptsResult::with_all_items(prompts)))
    }

    fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<GetPromptResponse, ErrorData>> + Send + '_ {
        let goal = request
            .arguments
            .as_ref()
            .and_then(|arguments| arguments.get("goal"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("the requested change");
        let text = match request.name.as_str() {
            "inspect-board" => INSPECT_BOARD_PROMPT.to_string(),
            "safe-board-change" => SAFE_CHANGE_PROMPT.replace("{goal}", goal),
            "validate-design" => VALIDATE_DESIGN_PROMPT.to_string(),
            "review-footprint-placement" => FOOTPRINT_PLACEMENT_PROMPT.to_string(),
            "review-routing" => ROUTING_REVIEW_PROMPT.to_string(),
            "manufacturing-preflight" => MANUFACTURING_PREFLIGHT_PROMPT.to_string(),
            "inspect-schematic" => INSPECT_SCHEMATIC_PROMPT.to_string(),
            _ => {
                return std::future::ready(Err(ErrorData::invalid_params(
                    format!("unknown prompt: {}", request.name),
                    None,
                )));
            }
        };
        let result = GetPromptResult::new(vec![PromptMessage::new_text(Role::User, text)]);
        std::future::ready(Ok(result.into()))
    }
}

const SAFETY_POLICY: &str = r#"# KiCad MCP safety policy

- Mutations are disabled unless `KICAD_MCP_ALLOW_WRITE=true`.
- Filesystem access is confined to canonical paths under `KICAD_MCP_PROJECT_ROOT`.
- Mutation tools default to dry-run and require the revision returned by a read tool.
- Each mutation uses a KiCad begin/end commit so it remains undoable.
- Mutations never save implicitly; call `save_active_board` explicitly.
- Validate with DRC/ERC before declaring a design complete.
- API tokens are never returned or logged.
"#;

const INSPECT_BOARD_PROMPT: &str = "Inspect the active PCB without changing it. First call kicad_status, then get_board_summary, list_footprints, list_nets, and get_selection as relevant. Paginate rather than requesting unbounded output. Report measured facts separately from recommendations.";
const SAFE_CHANGE_PROMPT: &str = "Safely implement this PCB goal: {goal}. Read the affected items and capture their revision hashes. Explain the intended change, call mutation tools in dry-run mode first, then apply only with the same revision if writes are enabled. Run DRC after changes. Do not save unless the user explicitly requested persistence.";
const VALIDATE_DESIGN_PROMPT: &str = "Validate the open KiCad design. Check kicad_status, summarize the active PCB, run DRC, and run ERC if a schematic is open. Distinguish tool execution failures from actual design violations. Do not mutate or save anything.";
const INSPECT_SCHEMATIC_PROMPT: &str = "Inspect a KiCad 11 schematic using schematic_hierarchy and schematic_netlist, using pagination. Preserve the hierarchy path_ids when recommending a target sheet for place_symbol. Run ERC when filesystem access is approved. Do not change the design.";
const FOOTPRINT_PLACEMENT_PROMPT: &str = "Review footprint placement read-only. Inspect the board summary and paginated footprints, checking measured positions, orientations, layers, reference/value consistency, edge clearance risk, clustering, and likely assembly-access issues. Separate observed facts from recommendations. Do not mutate or save.";
const ROUTING_REVIEW_PROMPT: &str = "Review routing read-only. Inspect board counts, nets, current selection, tracks/vias where available, and run DRC. Prioritize shorts, unconnected items, width/clearance issues, excessive vias, layer transitions, and return-path risks. Do not mutate or save.";
const MANUFACTURING_PREFLIGHT_PROMPT: &str = "Perform a read-only manufacturing preflight. Confirm KiCad connectivity and project identity, run DRC and ERC when applicable, review board outline and footprint/net summaries, and identify blockers before generating fabrication artifacts. Distinguish verified results from checks that require human fabrication-rule review. Do not mutate, export, or save.";

fn resource_result<T: serde::Serialize, E: std::fmt::Display>(result: Result<T, E>) -> String {
    result.map(|value| json(&value)).unwrap_or_else(|error| {
        serde_json::json!({ "ok": false, "error": error.to_string() }).to_string()
    })
}

fn default_page_size() -> usize {
    50
}
fn default_dry_run() -> bool {
    true
}

fn max_response_bytes() -> usize {
    std::env::var("KICAD_MCP_MAX_OUTPUT_BYTES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(64 * 1024)
        .clamp(1_024, 1024 * 1024)
}

fn enforce_response_limit(output: String) -> Result<String, String> {
    let limit = max_response_bytes();
    if output.len() <= limit {
        Ok(output)
    } else {
        Err(serde_json::json!({
            "ok": false,
            "error": "response exceeds configured byte limit; paginate or request fewer fields",
            "limit_bytes": limit,
        })
        .to_string())
    }
}

fn json<T: serde::Serialize>(value: &T) -> String {
    let output = serde_json::to_string_pretty(value)
        .unwrap_or_else(|error| serde_json::json!({ "error": error.to_string() }).to_string());
    enforce_response_limit(output).unwrap_or_else(|error| error)
}

fn encode<T: serde::Serialize, E: std::fmt::Display>(
    result: Result<T, E>,
) -> Result<String, String> {
    let value = result.map_err(|error| {
        serde_json::json!({ "ok": false, "error": error.to_string() }).to_string()
    })?;
    let output = serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?;
    enforce_response_limit(output)
}

fn encode_selected<T: serde::Serialize, E: std::fmt::Display>(
    result: Result<T, E>,
    fields: &[String],
    allowed: &[&str],
) -> Result<String, String> {
    if fields.is_empty() {
        return encode(result);
    }
    if let Some(field) = fields
        .iter()
        .find(|field| !allowed.contains(&field.as_str()))
    {
        return Err(serde_json::json!({
            "ok": false,
            "error": format!("unknown field: {field}"),
            "allowed_fields": allowed,
        })
        .to_string());
    }
    let value = result.map_err(|error| {
        serde_json::json!({ "ok": false, "error": error.to_string() }).to_string()
    })?;
    let mut value = serde_json::to_value(value).map_err(|error| error.to_string())?;
    if let Some(items) = value
        .get_mut("items")
        .and_then(serde_json::Value::as_array_mut)
    {
        for item in items {
            if let Some(object) = item.as_object_mut() {
                object.retain(|key, _| {
                    key == "id" || key == "revision" || fields.iter().any(|field| field == key)
                });
            }
        }
    }
    let output = serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?;
    enforce_response_limit(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_responses_are_rejected() {
        assert!(enforce_response_limit("x".repeat(1024 * 1024 + 1)).is_err());
    }

    #[test]
    fn disconnected_server_hides_kicad_11_tools() {
        let server = KicadMcp::new();
        assert!(!server.tool_router.is_disabled("schematic_hierarchy"));
        let router = server.current_tool_router();
        for name in [
            "schematic_hierarchy",
            "schematic_netlist",
            "place_symbol",
            "native_export_step",
        ] {
            assert!(router.is_disabled(name), "{name} was visible");
        }
    }
}
