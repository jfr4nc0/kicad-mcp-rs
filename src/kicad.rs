use crate::proto::v11::kiapi::{
    board::jobs::{Board3DFormat, RunBoardJobExport3D},
    common::{
        commands::{
            BeginCommit as BeginCommitV11, BeginCommitResponse as BeginCommitResponseV11,
            PlaceFromLibraryResponse,
        },
        types::{
            ItemHeader as ItemHeaderV11, JobStatus, LibraryIdentifier, RunJobResponse,
            RunJobSettings, Vector2 as Vector2V11,
        },
    },
    schematic::{
        commands::{
            GetSchematicHierarchy, GetSchematicNetlist, PlaceSymbolFromLibrary,
            SchematicHierarchyResponse, SchematicNetlistResponse,
        },
        types::{SchematicSymbolInstance, SchematicSymbolOrientation, SheetInstance},
    },
};
use crate::{
    ipc::{CallPolicy, IpcClient, IpcConfig, IpcError},
    proto::v10::kiapi::{
        board::{
            commands::{GetNets, NetsResponse},
            types::{
                BoardLayer, FootprintInstance, Net, PadStack, PadStackLayer, PadStackType, Track,
                Via, ViaType,
            },
        },
        common::{
            commands::{
                BeginCommit, BeginCommitResponse, CommitAction, CreateItems, CreateItemsResponse,
                DeleteItems, DeleteItemsResponse, EndCommit, EndCommitResponse, GetItems,
                GetItemsById, GetItemsResponse, GetKiCadBinaryPath, GetOpenDocuments,
                GetOpenDocumentsResponse, GetSelection, GetVersion, GetVersionResponse,
                ItemStatusCode, PathResponse, SaveDocument, SelectionResponse, UpdateItems,
                UpdateItemsResponse,
            },
            types::{
                Angle, Distance, DocumentSpecifier, DocumentType, ItemHeader, ItemRequestStatus,
                KiCadObjectType, Kiid, LockedState, Vector2,
            },
        },
    },
};
use prost::Message;
use prost_types::{Any, FieldMask};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    cmp::min,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use uuid::Uuid;
use wait_timeout::ChildExt;

const GET_VERSION: &str = "kiapi.common.commands.GetVersion";
const GET_VERSION_RESPONSE: &str = "kiapi.common.commands.GetVersionResponse";
const GET_OPEN_DOCUMENTS: &str = "kiapi.common.commands.GetOpenDocuments";
const GET_OPEN_DOCUMENTS_RESPONSE: &str = "kiapi.common.commands.GetOpenDocumentsResponse";
const GET_ITEMS: &str = "kiapi.common.commands.GetItems";
const GET_ITEMS_BY_ID: &str = "kiapi.common.commands.GetItemsById";
const GET_ITEMS_RESPONSE: &str = "kiapi.common.commands.GetItemsResponse";
const GET_SELECTION: &str = "kiapi.common.commands.GetSelection";
const SELECTION_RESPONSE: &str = "kiapi.common.commands.SelectionResponse";
const GET_NETS: &str = "kiapi.board.commands.GetNets";
const NETS_RESPONSE: &str = "kiapi.board.commands.NetsResponse";
const GET_BINARY: &str = "kiapi.common.commands.GetKiCadBinaryPath";
const PATH_RESPONSE: &str = "kiapi.common.commands.PathResponse";
const BEGIN_COMMIT: &str = "kiapi.common.commands.BeginCommit";
const BEGIN_COMMIT_RESPONSE: &str = "kiapi.common.commands.BeginCommitResponse";
const END_COMMIT: &str = "kiapi.common.commands.EndCommit";
const END_COMMIT_RESPONSE: &str = "kiapi.common.commands.EndCommitResponse";
const CREATE_ITEMS: &str = "kiapi.common.commands.CreateItems";
const CREATE_ITEMS_RESPONSE: &str = "kiapi.common.commands.CreateItemsResponse";
const UPDATE_ITEMS: &str = "kiapi.common.commands.UpdateItems";
const UPDATE_ITEMS_RESPONSE: &str = "kiapi.common.commands.UpdateItemsResponse";
const DELETE_ITEMS: &str = "kiapi.common.commands.DeleteItems";
const DELETE_ITEMS_RESPONSE: &str = "kiapi.common.commands.DeleteItemsResponse";
const SAVE_DOCUMENT: &str = "kiapi.common.commands.SaveDocument";
const EMPTY_RESPONSE: &str = "google.protobuf.Empty";
const GET_SCHEMATIC_HIERARCHY: &str = "kiapi.schematic.commands.GetSchematicHierarchy";
const SCHEMATIC_HIERARCHY_RESPONSE: &str = "kiapi.schematic.commands.SchematicHierarchyResponse";
const GET_SCHEMATIC_NETLIST: &str = "kiapi.schematic.commands.GetSchematicNetlist";
const SCHEMATIC_NETLIST_RESPONSE: &str = "kiapi.schematic.commands.SchematicNetlistResponse";
const PLACE_SYMBOL: &str = "kiapi.schematic.commands.PlaceSymbolFromLibrary";
const PLACE_FROM_LIBRARY_RESPONSE: &str = "kiapi.common.commands.PlaceFromLibraryResponse";
const RUN_BOARD_EXPORT_3D: &str = "kiapi.board.jobs.RunBoardJobExport3D";
const RUN_JOB_RESPONSE: &str = "kiapi.common.types.RunJobResponse";
const FOOTPRINT_TYPE: &str = "kiapi.board.types.FootprintInstance";
const TRACK_TYPE: &str = "kiapi.board.types.Track";
const VIA_TYPE: &str = "kiapi.board.types.Via";

#[derive(Debug, Clone)]
pub struct KiCadService {
    pub ipc: IpcClient,
    pub safety: SafetyConfig,
    latest_drc: Arc<Mutex<Option<CheckReport>>>,
    mutation_lock: Arc<Mutex<()>>,
}

#[derive(Debug, Clone)]
pub struct SafetyConfig {
    pub write_enabled: bool,
    pub project_root: Option<PathBuf>,
    pub max_page_size: usize,
    pub max_output_bytes: usize,
    pub cli_timeout: Duration,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionInfo {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub full_version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocumentInfo {
    pub kind: String,
    pub project_name: Option<String>,
    pub project_path: Option<String>,
    pub board_filename: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Capabilities {
    pub pcb_ipc: bool,
    pub schematic_ipc: bool,
    pub headless_ipc: bool,
    pub native_export_jobs: bool,
    pub cli_drc_erc: bool,
    pub mutations_enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusReport {
    pub connected: bool,
    pub socket_path: Option<String>,
    pub api_token_present: bool,
    pub version: Option<VersionInfo>,
    pub capabilities: Capabilities,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<usize>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct FootprintSummary {
    pub id: String,
    pub reference: String,
    pub value: String,
    pub library: String,
    pub position_nm: Point,
    pub orientation_degrees: f64,
    pub layer: String,
    pub revision: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetSummary {
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ItemSummary {
    pub id: Option<String>,
    pub item_type: String,
    pub type_url: String,
    pub revision: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Point {
    pub x_nm: i64,
    pub y_nm: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BoardSummary {
    pub document: DocumentInfo,
    pub footprints: usize,
    pub tracks: usize,
    pub vias: usize,
    pub zones: usize,
    pub nets: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct MutationReceipt {
    pub operation: String,
    pub affected_ids: Vec<String>,
    pub before_revision: Option<String>,
    pub after_revision: Option<String>,
    pub committed: bool,
    pub saved: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckReport {
    pub command: Vec<String>,
    pub exit_code: i32,
    pub report: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SchematicSheetSummary {
    pub name: String,
    pub filename: String,
    pub page_number: String,
    pub path: String,
    pub path_ids: Vec<String>,
    pub children: Vec<SchematicSheetSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SchematicNetSummary {
    pub name: String,
    pub sheet_count: usize,
    pub item_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct NativeJobReceipt {
    pub status: String,
    pub output_paths: Vec<String>,
    pub message: String,
}

impl Default for SafetyConfig {
    fn default() -> Self {
        let project_root = std::env::var_os("KICAD_MCP_PROJECT_ROOT")
            .map(PathBuf::from)
            .and_then(|path| path.canonicalize().ok());
        Self {
            write_enabled: matches!(
                std::env::var("KICAD_MCP_ALLOW_WRITE").as_deref(),
                Ok("1" | "true" | "yes")
            ),
            project_root,
            max_page_size: env_usize("KICAD_MCP_MAX_PAGE_SIZE", 200).clamp(1, 1_000),
            max_output_bytes: env_usize("KICAD_MCP_MAX_OUTPUT_BYTES", 64 * 1024)
                .clamp(1_024, 1024 * 1024),
            cli_timeout: Duration::from_secs(
                env_usize("KICAD_MCP_CLI_TIMEOUT_SECS", 120).clamp(1, 3_600) as u64,
            ),
        }
    }
}

impl KiCadService {
    pub fn from_env() -> Self {
        Self {
            ipc: IpcClient::new(IpcConfig::default()),
            safety: SafetyConfig::default(),
            latest_drc: Arc::new(Mutex::new(None)),
            mutation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn status(&self) -> StatusReport {
        match self.version() {
            Ok(version) => StatusReport {
                connected: true,
                socket_path: self
                    .ipc
                    .configured_socket()
                    .map(|path| path.display().to_string()),
                api_token_present: self.ipc.token_present(),
                capabilities: capabilities_for(&version, self.safety.write_enabled),
                version: Some(version),
                error: None,
            },
            Err(error) => StatusReport {
                connected: false,
                socket_path: self
                    .ipc
                    .configured_socket()
                    .map(|path| path.display().to_string()),
                api_token_present: self.ipc.token_present(),
                version: None,
                capabilities: capabilities_for(
                    &VersionInfo {
                        major: 0,
                        minor: 0,
                        patch: 0,
                        full_version: String::new(),
                    },
                    self.safety.write_enabled,
                ),
                error: Some(error.to_string()),
            },
        }
    }

    pub fn version(&self) -> Result<VersionInfo, IpcError> {
        let response: GetVersionResponse = self.ipc.call(
            &GetVersion {},
            GET_VERSION,
            GET_VERSION_RESPONSE,
            CallPolicy::ReadOnly,
        )?;
        let version = response.version.ok_or(IpcError::MissingResponse)?;
        Ok(VersionInfo {
            major: version.major,
            minor: version.minor,
            patch: version.patch,
            full_version: version.full_version,
        })
    }

    pub fn active_board(&self) -> Result<DocumentSpecifier, IpcError> {
        let response: GetOpenDocumentsResponse = self.ipc.call(
            &GetOpenDocuments {
                r#type: DocumentType::DoctypePcb as i32,
            },
            GET_OPEN_DOCUMENTS,
            GET_OPEN_DOCUMENTS_RESPONSE,
            CallPolicy::ReadOnly,
        )?;
        response.documents.into_iter().next().ok_or(IpcError::Api {
            status: "NO_OPEN_BOARD".to_string(),
            message: "open a board in PCB Editor first".to_string(),
        })
    }

    pub fn active_schematic(&self) -> Result<DocumentSpecifier, IpcError> {
        let response: GetOpenDocumentsResponse = self.ipc.call(
            &GetOpenDocuments {
                r#type: DocumentType::DoctypeSchematic as i32,
            },
            GET_OPEN_DOCUMENTS,
            GET_OPEN_DOCUMENTS_RESPONSE,
            CallPolicy::ReadOnly,
        )?;
        response.documents.into_iter().next().ok_or(IpcError::Api {
            status: "NO_OPEN_SCHEMATIC".to_string(),
            message: "open a schematic in Schematic Editor first".to_string(),
        })
    }

    pub fn active_project(&self) -> Result<DocumentInfo, IpcError> {
        let document = self.active_board().or_else(|_| self.active_schematic())?;
        if self.safety.project_root.is_some() {
            self.ensure_document_allowed(&document)?;
        }
        Ok(Self::document_info(&document))
    }

    pub fn document_info(document: &DocumentSpecifier) -> DocumentInfo {
        DocumentInfo {
            kind: DocumentType::try_from(document.r#type)
                .map(|kind| kind.as_str_name().to_string())
                .unwrap_or_else(|_| "DOCTYPE_UNKNOWN".to_string()),
            project_name: document.project.as_ref().map(|project| project.name.clone()),
            project_path: document.project.as_ref().map(|project| project.path.clone()),
            board_filename: match &document.identifier {
                Some(crate::proto::v10::kiapi::common::types::document_specifier::Identifier::BoardFilename(name)) => Some(name.clone()),
                _ => None,
            },
        }
    }

    pub fn board_summary(&self) -> Result<BoardSummary, IpcError> {
        let document = self.active_board()?;
        if self.safety.project_root.is_some() {
            self.ensure_document_allowed(&document)?;
        }
        let counts = [
            KiCadObjectType::KotPcbFootprint,
            KiCadObjectType::KotPcbTrace,
            KiCadObjectType::KotPcbVia,
            KiCadObjectType::KotPcbZone,
        ]
        .map(|kind| {
            self.get_items(&document, &[kind as i32])
                .map(|items| items.len())
        });
        let nets = self.get_nets(&document)?;
        Ok(BoardSummary {
            document: Self::document_info(&document),
            footprints: counts[0].clone()?,
            tracks: counts[1].clone()?,
            vias: counts[2].clone()?,
            zones: counts[3].clone()?,
            nets: nets.len(),
        })
    }

    pub fn list_footprints(
        &self,
        cursor: usize,
        limit: usize,
    ) -> Result<Page<FootprintSummary>, IpcError> {
        let document = self.active_board()?;
        let items = self.get_items(&document, &[KiCadObjectType::KotPcbFootprint as i32])?;
        let mut footprints = Vec::with_capacity(items.len());
        for item in items {
            footprints.push(decode_footprint(&item)?);
        }
        Ok(paginate(
            footprints,
            cursor,
            limit,
            self.safety.max_page_size,
        ))
    }

    pub fn get_footprint(&self, id: &str) -> Result<FootprintSummary, IpcError> {
        let document = self.active_board()?;
        let response: GetItemsResponse = self.ipc.call(
            &GetItemsById {
                header: Some(item_header(document, Vec::new())),
                items: vec![Kiid {
                    value: id.to_string(),
                }],
            },
            GET_ITEMS_BY_ID,
            GET_ITEMS_RESPONSE,
            CallPolicy::ReadOnly,
        )?;
        validate_item_request_status(response.status)?;
        let item = response.items.first().ok_or(IpcError::Api {
            status: "ITEM_NOT_FOUND".to_string(),
            message: format!("footprint {id} was not found"),
        })?;
        decode_footprint(item)
    }

    pub fn list_nets(&self, cursor: usize, limit: usize) -> Result<Page<NetSummary>, IpcError> {
        let document = self.active_board()?;
        let mut nets: Vec<NetSummary> = self
            .get_nets(&document)?
            .into_iter()
            .map(|net| NetSummary { name: net.name })
            .collect();
        nets.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(paginate(nets, cursor, limit, self.safety.max_page_size))
    }

    pub fn selection(&self) -> Result<Vec<ItemSummary>, IpcError> {
        let document = self.active_board()?;
        let response: SelectionResponse = self.ipc.call(
            &GetSelection {
                header: Some(item_header(document, Vec::new())),
                types: Vec::new(),
            },
            GET_SELECTION,
            SELECTION_RESPONSE,
            CallPolicy::ReadOnly,
        )?;
        response
            .items
            .iter()
            .take(self.safety.max_page_size)
            .map(summarize_any)
            .collect()
    }

    pub fn move_footprint(
        &self,
        id: &str,
        x_nm: i64,
        y_nm: i64,
        orientation_degrees: Option<f64>,
        expected_revision: &str,
        dry_run: bool,
    ) -> Result<MutationReceipt, IpcError> {
        let _guard = self.lock_mutation()?;
        self.require_write()?;
        let document = self.active_board()?;
        self.ensure_document_allowed(&document)?;
        let current_any = self.get_item_any(&document, id)?;
        let before = revision(&current_any.value);
        if before != expected_revision {
            return Err(conflict(expected_revision, &before));
        }
        let mut footprint = decode_any::<FootprintInstance>(&current_any, FOOTPRINT_TYPE)?;
        footprint.position = Some(Vector2 { x_nm, y_nm });
        if let Some(value_degrees) = orientation_degrees {
            footprint.orientation = Some(Angle { value_degrees });
        }
        let updated_any = pack(FOOTPRINT_TYPE, &footprint);
        let mut after = revision(&updated_any.value);
        if dry_run {
            return Ok(MutationReceipt {
                operation: "move_footprint".to_string(),
                affected_ids: vec![id.to_string()],
                before_revision: Some(before),
                after_revision: Some(after),
                committed: false,
                saved: false,
            });
        }
        self.update_items(
            document.clone(),
            vec![updated_any],
            vec!["position".to_string(), "orientation".to_string()],
            "Move footprint from MCP",
        )?;
        after = revision(&self.get_item_any(&document, id)?.value);
        Ok(MutationReceipt {
            operation: "move_footprint".to_string(),
            affected_ids: vec![id.to_string()],
            before_revision: Some(before),
            after_revision: Some(after),
            committed: true,
            saved: false,
        })
    }

    pub fn set_footprint_property(
        &self,
        id: &str,
        property: &str,
        value: &str,
        expected_revision: &str,
        dry_run: bool,
    ) -> Result<MutationReceipt, IpcError> {
        let _guard = self.lock_mutation()?;
        self.require_write()?;
        let document = self.active_board()?;
        self.ensure_document_allowed(&document)?;
        let current_any = self.get_item_any(&document, id)?;
        let before = revision(&current_any.value);
        if before != expected_revision {
            return Err(conflict(expected_revision, &before));
        }
        let mut footprint = decode_any::<FootprintInstance>(&current_any, FOOTPRINT_TYPE)?;
        let field = match property {
            "reference" => &mut footprint.reference_field,
            "value" => &mut footprint.value_field,
            "datasheet" => &mut footprint.datasheet_field,
            "description" => &mut footprint.description_field,
            _ => {
                return Err(IpcError::Api {
                    status: "UNSUPPORTED_PROPERTY".to_string(),
                    message: "property must be reference, value, datasheet, or description"
                        .to_string(),
                });
            }
        };
        let field = field.as_mut().ok_or(IpcError::Api {
            status: "MISSING_FIELD".to_string(),
            message: format!("footprint does not contain {property} field"),
        })?;
        let board_text = field.text.as_mut().ok_or(IpcError::Api {
            status: "MISSING_TEXT".to_string(),
            message: format!("footprint {property} field has no text object"),
        })?;
        let text = board_text.text.as_mut().ok_or(IpcError::Api {
            status: "MISSING_TEXT".to_string(),
            message: format!("footprint {property} field has no text value"),
        })?;
        text.text = value.to_string();

        let updated_any = pack(FOOTPRINT_TYPE, &footprint);
        let mut after = revision(&updated_any.value);
        if !dry_run {
            self.update_items(
                document.clone(),
                vec![updated_any],
                vec![format!("{property}_field")],
                "Update footprint property from MCP",
            )?;
            after = revision(&self.get_item_any(&document, id)?.value);
        }
        Ok(MutationReceipt {
            operation: "set_footprint_property".to_string(),
            affected_ids: vec![id.to_string()],
            before_revision: Some(before),
            after_revision: Some(after),
            committed: !dry_run,
            saved: false,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_track(
        &self,
        start: Point,
        end: Point,
        width_nm: i64,
        layer: i32,
        net_name: String,
        dry_run: bool,
    ) -> Result<MutationReceipt, IpcError> {
        let _guard = self.lock_mutation()?;
        self.require_write()?;
        validate_positive("width_nm", width_nm)?;
        BoardLayer::try_from(layer).map_err(|_| invalid("layer", layer))?;
        let document = self.active_board()?;
        self.ensure_document_allowed(&document)?;
        let track = Track {
            id: None,
            start: Some(start.into()),
            end: Some(end.into()),
            width: Some(Distance { value_nm: width_nm }),
            locked: LockedState::LsUnlocked as i32,
            layer,
            net: Some(Net {
                code: None,
                name: net_name,
            }),
            parent: None,
        };
        let packed = pack(TRACK_TYPE, &track);
        if dry_run {
            return Ok(MutationReceipt {
                operation: "create_track".to_string(),
                affected_ids: Vec::new(),
                before_revision: None,
                after_revision: Some(revision(&packed.value)),
                committed: false,
                saved: false,
            });
        }
        let created = self.create_items(document.clone(), vec![packed], "Create track from MCP")?;
        let after_revision = created
            .first()
            .map(|id| {
                self.get_item_any(&document, id)
                    .map(|item| revision(&item.value))
            })
            .transpose()?;
        Ok(MutationReceipt {
            operation: "create_track".to_string(),
            affected_ids: created,
            before_revision: None,
            after_revision,
            committed: true,
            saved: false,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_track(
        &self,
        id: &str,
        start: Point,
        end: Point,
        width_nm: i64,
        layer: i32,
        net_name: String,
        expected_revision: &str,
        dry_run: bool,
    ) -> Result<MutationReceipt, IpcError> {
        let _guard = self.lock_mutation()?;
        self.require_write()?;
        validate_positive("width_nm", width_nm)?;
        BoardLayer::try_from(layer).map_err(|_| invalid("layer", layer))?;
        let document = self.active_board()?;
        self.ensure_document_allowed(&document)?;
        let current_any = self.get_item_any(&document, id)?;
        let before = revision(&current_any.value);
        if before != expected_revision {
            return Err(conflict(expected_revision, &before));
        }
        let mut track = decode_any::<Track>(&current_any, TRACK_TYPE)?;
        track.start = Some(start.into());
        track.end = Some(end.into());
        track.width = Some(Distance { value_nm: width_nm });
        track.layer = layer;
        track.net = Some(Net {
            code: None,
            name: net_name,
        });
        let packed = pack(TRACK_TYPE, &track);
        let mut after = revision(&packed.value);
        if !dry_run {
            self.update_items(
                document.clone(),
                vec![packed],
                vec![
                    "start".to_string(),
                    "end".to_string(),
                    "width".to_string(),
                    "layer".to_string(),
                    "net".to_string(),
                ],
                "Update track from MCP",
            )?;
            after = revision(&self.get_item_any(&document, id)?.value);
        }
        Ok(MutationReceipt {
            operation: "update_track".to_string(),
            affected_ids: vec![id.to_string()],
            before_revision: Some(before),
            after_revision: Some(after),
            committed: !dry_run,
            saved: false,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_via(
        &self,
        position: Point,
        diameter_nm: i64,
        drill_nm: i64,
        start_layer: i32,
        end_layer: i32,
        net_name: String,
        dry_run: bool,
    ) -> Result<MutationReceipt, IpcError> {
        let _guard = self.lock_mutation()?;
        self.require_write()?;
        validate_positive("diameter_nm", diameter_nm)?;
        validate_positive("drill_nm", drill_nm)?;
        if drill_nm >= diameter_nm {
            return Err(IpcError::Api {
                status: "INVALID_GEOMETRY".to_string(),
                message: "drill_nm must be smaller than diameter_nm".to_string(),
            });
        }
        let document = self.active_board()?;
        self.ensure_document_allowed(&document)?;
        let layers = vec![start_layer, end_layer];
        for layer in &layers {
            BoardLayer::try_from(*layer).map_err(|_| invalid("layer", *layer))?;
        }
        let shape = PadStackLayer {
            layer: start_layer,
            shape: crate::proto::v10::kiapi::board::types::PadStackShape::PssCircle as i32,
            size: Some(Vector2 {
                x_nm: diameter_nm,
                y_nm: diameter_nm,
            }),
            corner_rounding_ratio: 0.0,
            chamfer_ratio: 0.0,
            chamfered_corners: None,
            custom_shapes: Vec::new(),
            custom_anchor_shape: crate::proto::v10::kiapi::board::types::PadStackShape::PssCircle
                as i32,
            zone_settings: None,
            trapezoid_delta: None,
            offset: None,
        };
        let via = Via {
            id: None,
            position: Some(position.into()),
            pad_stack: Some(PadStack {
                r#type: PadStackType::PstNormal as i32,
                layers,
                drill: Some(crate::proto::v10::kiapi::board::types::DrillProperties {
                    start_layer,
                    end_layer,
                    diameter: Some(Vector2 {
                        x_nm: drill_nm,
                        y_nm: drill_nm,
                    }),
                    shape: crate::proto::v10::kiapi::board::types::DrillShape::DsCircle as i32,
                    capped: 0,
                    filled: 0,
                }),
                unconnected_layer_removal: 0,
                copper_layers: vec![shape],
                angle: Some(Angle { value_degrees: 0.0 }),
                front_outer_layers: None,
                back_outer_layers: None,
                zone_settings: None,
                secondary_drill: None,
                tertiary_drill: None,
                front_post_machining: None,
                back_post_machining: None,
            }),
            locked: LockedState::LsUnlocked as i32,
            net: Some(Net {
                code: None,
                name: net_name,
            }),
            r#type: ViaType::VtThrough as i32,
            parent: None,
        };
        let packed = pack(VIA_TYPE, &via);
        if dry_run {
            return Ok(MutationReceipt {
                operation: "create_via".to_string(),
                affected_ids: Vec::new(),
                before_revision: None,
                after_revision: Some(revision(&packed.value)),
                committed: false,
                saved: false,
            });
        }
        let created = self.create_items(document.clone(), vec![packed], "Create via from MCP")?;
        let after_revision = created
            .first()
            .map(|id| {
                self.get_item_any(&document, id)
                    .map(|item| revision(&item.value))
            })
            .transpose()?;
        Ok(MutationReceipt {
            operation: "create_via".to_string(),
            affected_ids: created,
            before_revision: None,
            after_revision,
            committed: true,
            saved: false,
        })
    }

    pub fn delete_item(
        &self,
        id: &str,
        expected_revision: &str,
        dry_run: bool,
    ) -> Result<MutationReceipt, IpcError> {
        let _guard = self.lock_mutation()?;
        self.require_write()?;
        let document = self.active_board()?;
        self.ensure_document_allowed(&document)?;
        let current = self.get_item_any(&document, id)?;
        let before = revision(&current.value);
        if before != expected_revision {
            return Err(conflict(expected_revision, &before));
        }
        if !dry_run {
            let commit = self.begin_commit(&document)?;
            let result: Result<DeleteItemsResponse, IpcError> = self
                .ipc
                .call(
                    &DeleteItems {
                        header: Some(item_header(document, Vec::new())),
                        item_ids: vec![Kiid {
                            value: id.to_string(),
                        }],
                    },
                    DELETE_ITEMS,
                    DELETE_ITEMS_RESPONSE,
                    CallPolicy::Mutation,
                )
                .and_then(validate_delete_response);
            self.finish_commit(commit, result.is_ok(), "Delete item from MCP")?;
            result?;
        }
        Ok(MutationReceipt {
            operation: "delete_item".to_string(),
            affected_ids: vec![id.to_string()],
            before_revision: Some(before),
            after_revision: None,
            committed: !dry_run,
            saved: false,
        })
    }

    pub fn save_active_board(&self) -> Result<MutationReceipt, IpcError> {
        let _guard = self.lock_mutation()?;
        self.require_write()?;
        let document = self.active_board()?;
        self.ensure_document_allowed(&document)?;
        let _: () = self.ipc.call(
            &SaveDocument {
                document: Some(document),
            },
            SAVE_DOCUMENT,
            EMPTY_RESPONSE,
            CallPolicy::Mutation,
        )?;
        Ok(MutationReceipt {
            operation: "save_active_board".to_string(),
            affected_ids: Vec::new(),
            before_revision: None,
            after_revision: None,
            committed: true,
            saved: true,
        })
    }

    pub fn run_drc(&self) -> Result<CheckReport, IpcError> {
        let document = self.active_board()?;
        let board = board_path(&document)?;
        self.ensure_allowed(&board)?;
        let cli = self.kicad_cli_path()?;
        let report = self.run_check_json(
            cli,
            vec![
                "pcb".to_string(),
                "drc".to_string(),
                "--format".to_string(),
                "json".to_string(),
                "--severity-all".to_string(),
                board.display().to_string(),
            ],
        )?;
        *self
            .latest_drc
            .lock()
            .map_err(|_| IpcError::Transport("DRC cache mutex poisoned".to_string()))? =
            Some(report.clone());
        Ok(report)
    }

    pub fn latest_drc(&self) -> Option<CheckReport> {
        self.latest_drc
            .lock()
            .ok()
            .and_then(|report| report.clone())
    }

    pub fn run_erc(&self) -> Result<CheckReport, IpcError> {
        let document = self.active_schematic()?;
        let schematic = schematic_path(&document)?;
        self.ensure_allowed(&schematic)?;
        let cli = self.kicad_cli_path()?;
        self.run_check_json(
            cli,
            vec![
                "sch".to_string(),
                "erc".to_string(),
                "--format".to_string(),
                "json".to_string(),
                "--severity-all".to_string(),
                schematic.display().to_string(),
            ],
        )
    }

    pub fn schematic_hierarchy(&self) -> Result<Vec<SchematicSheetSummary>, IpcError> {
        self.require_kicad_11()?;
        let document_v10 = self.active_schematic()?;
        let document = crate::proto::v11::kiapi::common::types::DocumentSpecifier::decode(
            document_v10.encode_to_vec().as_slice(),
        )
        .map_err(|error| IpcError::Codec(error.to_string()))?;
        let response: SchematicHierarchyResponse = self.ipc.call(
            &GetSchematicHierarchy {
                document: Some(document),
            },
            GET_SCHEMATIC_HIERARCHY,
            SCHEMATIC_HIERARCHY_RESPONSE,
            CallPolicy::ReadOnly,
        )?;
        let mut remaining = self.safety.max_page_size;
        Ok(response
            .top_level_sheets
            .iter()
            .filter_map(|sheet| summarize_sheet_bounded(sheet, &mut remaining))
            .collect())
    }

    pub fn schematic_netlist(
        &self,
        cursor: usize,
        limit: usize,
    ) -> Result<Page<SchematicNetSummary>, IpcError> {
        self.require_kicad_11()?;
        let document_v10 = self.active_schematic()?;
        let document = crate::proto::v11::kiapi::common::types::DocumentSpecifier::decode(
            document_v10.encode_to_vec().as_slice(),
        )
        .map_err(|error| IpcError::Codec(error.to_string()))?;
        let response: SchematicNetlistResponse = self.ipc.call(
            &GetSchematicNetlist {
                document: Some(document),
                types: Vec::new(),
            },
            GET_SCHEMATIC_NETLIST,
            SCHEMATIC_NETLIST_RESPONSE,
            CallPolicy::ReadOnly,
        )?;
        let mut nets: Vec<_> = response
            .nets
            .into_iter()
            .map(|net| SchematicNetSummary {
                name: net.name,
                sheet_count: net.sheets.len(),
                item_count: net.sheets.iter().map(|sheet| sheet.items.len()).sum(),
            })
            .collect();
        nets.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(paginate(nets, cursor, limit, self.safety.max_page_size))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn place_symbol(
        &self,
        library_nickname: String,
        entry_name: String,
        position: Point,
        orientation: Option<i32>,
        reference: Option<String>,
        sheet_path_ids: Option<Vec<String>>,
        dry_run: bool,
    ) -> Result<MutationReceipt, IpcError> {
        let _guard = self.lock_mutation()?;
        self.require_kicad_11()?;
        self.require_write()?;
        if let Some(value) = orientation {
            SchematicSymbolOrientation::try_from(value)
                .map_err(|_| invalid("orientation", value))?;
        }
        let document_v10 = self.active_schematic()?;
        self.ensure_document_allowed(&document_v10)?;
        let mut document = crate::proto::v11::kiapi::common::types::DocumentSpecifier::decode(
            document_v10.encode_to_vec().as_slice(),
        )
        .map_err(|error| IpcError::Codec(error.to_string()))?;
        if let Some(path_ids) = sheet_path_ids {
            if path_ids.is_empty() || path_ids.iter().any(String::is_empty) {
                return Err(IpcError::Api {
                    status: "INVALID_SHEET_PATH".to_string(),
                    message: "sheet_path_ids must contain one or more non-empty KIIDs".to_string(),
                });
            }
            document.identifier = Some(
                crate::proto::v11::kiapi::common::types::document_specifier::Identifier::SheetPath(
                    crate::proto::v11::kiapi::common::types::SheetPath {
                        path: path_ids
                            .into_iter()
                            .map(|value| crate::proto::v11::kiapi::common::types::Kiid { value })
                            .collect(),
                        path_human_readable: String::new(),
                    },
                ),
            );
        }
        if dry_run {
            return Ok(MutationReceipt {
                operation: "place_symbol".to_string(),
                affected_ids: Vec::new(),
                before_revision: None,
                after_revision: None,
                committed: false,
                saved: false,
            });
        }
        let response: PlaceFromLibraryResponse = self.ipc.call(
            &PlaceSymbolFromLibrary {
                header: Some(ItemHeaderV11 {
                    document: Some(document),
                    container: None,
                    field_mask: None,
                }),
                lib_id: Some(LibraryIdentifier {
                    library_nickname,
                    entry_name,
                }),
                position: Some(Vector2V11 {
                    x_nm: position.x_nm,
                    y_nm: position.y_nm,
                }),
                orientation,
                unit: None,
                reference,
            },
            PLACE_SYMBOL,
            PLACE_FROM_LIBRARY_RESPONSE,
            CallPolicy::Mutation,
        )?;
        let item = response.item.ok_or(IpcError::MissingResponse)?;
        let symbol_type = "kiapi.schematic.types.SchematicSymbolInstance";
        let symbol = decode_any::<SchematicSymbolInstance>(&item, symbol_type)?;
        let id = symbol.id.map(|value| value.value).unwrap_or_default();
        Ok(MutationReceipt {
            operation: "place_symbol".to_string(),
            affected_ids: vec![id],
            before_revision: None,
            after_revision: Some(revision(&item.value)),
            committed: true,
            saved: false,
        })
    }

    pub fn native_export_step(&self, output: &Path) -> Result<NativeJobReceipt, IpcError> {
        self.require_kicad_11()?;
        let (output, staged_output) = self.staged_output(output)?;
        let document_v10 = self.active_board()?;
        self.ensure_document_allowed(&document_v10)?;
        let document = crate::proto::v11::kiapi::common::types::DocumentSpecifier::decode(
            document_v10.encode_to_vec().as_slice(),
        )
        .map_err(|error| IpcError::Codec(error.to_string()))?;
        let response: RunJobResponse = self.ipc.call(
            &RunBoardJobExport3D {
                job_settings: Some(RunJobSettings {
                    document: Some(document),
                    output_path: staged_output.display().to_string(),
                }),
                format: Board3DFormat::B3dStep as i32,
                overwrite: false,
                export_board_body: true,
                export_components: true,
                export_tracks_and_vias: true,
                export_pads: true,
                export_zones: true,
                export_silkscreen: true,
                export_soldermask: true,
                include_unspecified: true,
                ..Default::default()
            },
            RUN_BOARD_EXPORT_3D,
            RUN_JOB_RESPONSE,
            CallPolicy::Mutation,
        )?;
        let status = JobStatus::try_from(response.status).map_err(|_| IpcError::Api {
            status: "INVALID_JOB_STATUS".to_string(),
            message: format!("unknown KiCad job status {}", response.status),
        })?;
        if matches!(status, JobStatus::JsError | JobStatus::JsUnspecified) {
            let _ = remove_output_path(&staged_output);
            return Err(IpcError::Api {
                status: status.as_str_name().to_string(),
                message: response.message,
            });
        }
        self.publish_staged_output(&staged_output, &output)?;
        Ok(NativeJobReceipt {
            status: status.as_str_name().to_string(),
            output_paths: vec![output.display().to_string()],
            message: response.message,
        })
    }

    pub fn export_board(&self, format: &str, output: &Path) -> Result<CheckReport, IpcError> {
        let document = self.active_board()?;
        let board = board_path(&document)?;
        self.ensure_allowed(&board)?;
        let (output, staged_output) = self.staged_output(output)?;
        let subcommand = match format {
            "gerbers" => "gerbers",
            "pdf" => "pdf",
            "svg" => "svg",
            "step" => "step",
            "vrml" => "vrml",
            _ => {
                return Err(IpcError::Api {
                    status: "UNSUPPORTED_EXPORT".to_string(),
                    message: "format must be gerbers, pdf, svg, step, or vrml".to_string(),
                });
            }
        };
        let cli = self.kicad_cli_path()?;
        let mut arguments = vec![
            "pcb".to_string(),
            "export".to_string(),
            subcommand.to_string(),
            "--output".to_string(),
            staged_output.display().to_string(),
        ];
        if matches!(format, "pdf" | "svg") {
            arguments.extend([
                "--layers".to_string(),
                "F.Cu,B.Cu,F.Silkscreen,B.Silkscreen,Edge.Cuts".to_string(),
                if format == "svg" {
                    "--mode-single".to_string()
                } else {
                    "--mode-multipage".to_string()
                },
            ]);
        }
        arguments.push(board.display().to_string());
        let mut report = self.run_check(cli, arguments)?;
        if report.exit_code != 0 {
            let _ = remove_output_path(&staged_output);
            return Err(IpcError::Api {
                status: "CLI_FAILED".to_string(),
                message: format!(
                    "kicad-cli export exited with {}: {}",
                    report.exit_code, report.report
                ),
            });
        }
        self.publish_staged_output(&staged_output, &output)?;
        let staged_text = staged_output.to_string_lossy();
        let output_text = output.to_string_lossy();
        report.report = report
            .report
            .replace(staged_text.as_ref(), output_text.as_ref());
        for argument in &mut report.command {
            if argument == staged_text.as_ref() {
                *argument = output_text.to_string();
            }
        }
        Ok(report)
    }

    fn get_items(&self, document: &DocumentSpecifier, types: &[i32]) -> Result<Vec<Any>, IpcError> {
        let response: GetItemsResponse = self.ipc.call(
            &GetItems {
                header: Some(item_header(document.clone(), Vec::new())),
                types: types.to_vec(),
            },
            GET_ITEMS,
            GET_ITEMS_RESPONSE,
            CallPolicy::ReadOnly,
        )?;
        validate_item_request_status(response.status)?;
        Ok(response.items)
    }

    fn get_item_any(&self, document: &DocumentSpecifier, id: &str) -> Result<Any, IpcError> {
        let response: GetItemsResponse = self.ipc.call(
            &GetItemsById {
                header: Some(item_header(document.clone(), Vec::new())),
                items: vec![Kiid {
                    value: id.to_string(),
                }],
            },
            GET_ITEMS_BY_ID,
            GET_ITEMS_RESPONSE,
            CallPolicy::ReadOnly,
        )?;
        validate_item_request_status(response.status)?;
        response.items.into_iter().next().ok_or(IpcError::Api {
            status: "ITEM_NOT_FOUND".to_string(),
            message: format!("item {id} was not found"),
        })
    }

    fn get_nets(&self, document: &DocumentSpecifier) -> Result<Vec<Net>, IpcError> {
        let response: NetsResponse = self.ipc.call(
            &GetNets {
                board: Some(document.clone()),
                netclass_filter: Vec::new(),
            },
            GET_NETS,
            NETS_RESPONSE,
            CallPolicy::ReadOnly,
        )?;
        Ok(response.nets)
    }

    fn begin_commit(&self, document: &DocumentSpecifier) -> Result<Kiid, IpcError> {
        if self.version()?.major >= 11 {
            let document_v11 = crate::proto::v11::kiapi::common::types::DocumentSpecifier::decode(
                document.encode_to_vec().as_slice(),
            )
            .map_err(|error| IpcError::Codec(error.to_string()))?;
            let response: BeginCommitResponseV11 = self.ipc.call(
                &BeginCommitV11 {
                    header: Some(ItemHeaderV11 {
                        document: Some(document_v11),
                        container: None,
                        field_mask: None,
                    }),
                },
                BEGIN_COMMIT,
                BEGIN_COMMIT_RESPONSE,
                CallPolicy::Mutation,
            )?;
            return response
                .id
                .map(|id| Kiid { value: id.value })
                .ok_or(IpcError::MissingResponse);
        }
        let response: BeginCommitResponse = self.ipc.call(
            &BeginCommit {},
            BEGIN_COMMIT,
            BEGIN_COMMIT_RESPONSE,
            CallPolicy::Mutation,
        )?;
        response.id.ok_or(IpcError::MissingResponse)
    }

    fn finish_commit(&self, id: Kiid, commit: bool, message: &str) -> Result<(), IpcError> {
        let _: EndCommitResponse = self.ipc.call(
            &EndCommit {
                id: Some(id),
                action: if commit {
                    CommitAction::CmaCommit as i32
                } else {
                    CommitAction::CmaDrop as i32
                },
                message: message.to_string(),
            },
            END_COMMIT,
            END_COMMIT_RESPONSE,
            CallPolicy::Mutation,
        )?;
        Ok(())
    }

    fn create_items(
        &self,
        document: DocumentSpecifier,
        items: Vec<Any>,
        message: &str,
    ) -> Result<Vec<String>, IpcError> {
        let commit = self.begin_commit(&document)?;
        let result: Result<CreateItemsResponse, IpcError> = self
            .ipc
            .call(
                &CreateItems {
                    header: Some(item_header(document, Vec::new())),
                    items,
                    container: None,
                },
                CREATE_ITEMS,
                CREATE_ITEMS_RESPONSE,
                CallPolicy::Mutation,
            )
            .and_then(validate_create_response);
        self.finish_commit(commit, result.is_ok(), message)?;
        let response = result?;
        response
            .created_items
            .into_iter()
            .map(|item| {
                item.item
                    .as_ref()
                    .and_then(any_id)
                    .ok_or(IpcError::MissingResponse)
            })
            .collect()
    }

    fn update_items(
        &self,
        document: DocumentSpecifier,
        items: Vec<Any>,
        fields: Vec<String>,
        message: &str,
    ) -> Result<UpdateItemsResponse, IpcError> {
        let commit = self.begin_commit(&document)?;
        let result = self
            .ipc
            .call(
                &UpdateItems {
                    header: Some(item_header(document, fields)),
                    items,
                },
                UPDATE_ITEMS,
                UPDATE_ITEMS_RESPONSE,
                CallPolicy::Mutation,
            )
            .and_then(validate_update_response);
        self.finish_commit(commit, result.is_ok(), message)?;
        result
    }

    fn lock_mutation(&self) -> Result<std::sync::MutexGuard<'_, ()>, IpcError> {
        self.mutation_lock.lock().map_err(|_| IpcError::Api {
            status: "INTERNAL_STATE".to_string(),
            message: "mutation workflow lock is poisoned; restart the MCP server".to_string(),
        })
    }

    fn ensure_document_allowed(&self, document: &DocumentSpecifier) -> Result<(), IpcError> {
        let path = match DocumentType::try_from(document.r#type) {
            Ok(DocumentType::DoctypePcb) => board_path(document)?,
            Ok(DocumentType::DoctypeSchematic) => schematic_path(document)?,
            _ => {
                return Err(IpcError::Api {
                    status: "UNSUPPORTED_DOCUMENT".to_string(),
                    message: "only PCB and schematic documents can be mutated".to_string(),
                });
            }
        };
        self.ensure_allowed(&path)
    }

    fn require_write(&self) -> Result<(), IpcError> {
        if !self.safety.write_enabled {
            return Err(IpcError::Api {
                status: "WRITE_DISABLED".to_string(),
                message: "set KICAD_MCP_ALLOW_WRITE=true to enable mutations".to_string(),
            });
        }
        if self.safety.project_root.is_none() {
            return Err(IpcError::Api {
                status: "PROJECT_ROOT_REQUIRED".to_string(),
                message: "set KICAD_MCP_PROJECT_ROOT to an approved project directory".to_string(),
            });
        }
        Ok(())
    }

    fn require_kicad_11(&self) -> Result<(), IpcError> {
        let version = self.version()?;
        if version.major >= 11 {
            Ok(())
        } else {
            Err(IpcError::Api {
                status: "KICAD_11_REQUIRED".to_string(),
                message: format!(
                    "this IPC operation requires KiCad 11 or newer; connected version is {}",
                    version.full_version
                ),
            })
        }
    }

    fn ensure_allowed(&self, path: &Path) -> Result<(), IpcError> {
        let root = self.safety.project_root.as_ref().ok_or(IpcError::Api {
            status: "PROJECT_ROOT_REQUIRED".to_string(),
            message: "set KICAD_MCP_PROJECT_ROOT before accessing project files".to_string(),
        })?;
        let canonical = path.canonicalize().map_err(|error| IpcError::Api {
            status: "PATH_NOT_FOUND".to_string(),
            message: error.to_string(),
        })?;
        if canonical.starts_with(root) {
            Ok(())
        } else {
            Err(IpcError::Api {
                status: "PATH_OUTSIDE_PROJECT_ROOT".to_string(),
                message: format!("{} is outside {}", canonical.display(), root.display()),
            })
        }
    }

    fn ensure_allowed_output(&self, path: &Path) -> Result<PathBuf, IpcError> {
        let file_name = path.file_name().ok_or(IpcError::Api {
            status: "INVALID_OUTPUT_PATH".to_string(),
            message: "output path must include a filename".to_string(),
        })?;
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let canonical_parent = parent.canonicalize().map_err(|error| IpcError::Api {
            status: "PATH_NOT_FOUND".to_string(),
            message: error.to_string(),
        })?;
        self.ensure_allowed(&canonical_parent)?;
        if let Ok(metadata) = fs::symlink_metadata(path) {
            let status = if metadata.file_type().is_symlink() {
                "OUTPUT_SYMLINK_REJECTED"
            } else {
                "OUTPUT_EXISTS"
            };
            return Err(IpcError::Api {
                status: status.to_string(),
                message: format!("refusing to overwrite existing output {}", path.display()),
            });
        }
        Ok(canonical_parent.join(file_name))
    }

    fn staged_output(&self, path: &Path) -> Result<(PathBuf, PathBuf), IpcError> {
        let final_path = self.ensure_allowed_output(path)?;
        let file_name = final_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(IpcError::Api {
                status: "INVALID_OUTPUT_PATH".to_string(),
                message: "output filename must be valid UTF-8".to_string(),
            })?;
        let staged =
            final_path.with_file_name(format!(".kicad-mcp-{}-{file_name}", Uuid::new_v4()));
        Ok((final_path, staged))
    }

    fn publish_staged_output(&self, staged: &Path, final_path: &Path) -> Result<(), IpcError> {
        let metadata = fs::symlink_metadata(staged).map_err(|error| IpcError::Api {
            status: "OUTPUT_MISSING".to_string(),
            message: format!("export did not create {}: {error}", staged.display()),
        })?;
        if metadata.file_type().is_symlink() {
            return Err(IpcError::Api {
                status: "OUTPUT_SYMLINK_REJECTED".to_string(),
                message: "export unexpectedly produced a symlink".to_string(),
            });
        }
        fs::rename(staged, final_path).map_err(|error| IpcError::Api {
            status: "OUTPUT_PUBLISH_FAILED".to_string(),
            message: error.to_string(),
        })
    }

    fn kicad_cli_path(&self) -> Result<PathBuf, IpcError> {
        if let Some(path) = std::env::var_os("KICAD_CLI").map(PathBuf::from) {
            return Ok(path);
        }
        let response: PathResponse = self.ipc.call(
            &GetKiCadBinaryPath {
                binary_name: "kicad-cli".to_string(),
            },
            GET_BINARY,
            PATH_RESPONSE,
            CallPolicy::ReadOnly,
        )?;
        if response.path.is_empty() {
            Err(IpcError::Api {
                status: "KICAD_CLI_NOT_FOUND".to_string(),
                message: "KiCad did not report a kicad-cli path".to_string(),
            })
        } else {
            Ok(PathBuf::from(response.path))
        }
    }

    fn run_check_json(
        &self,
        executable: PathBuf,
        mut arguments: Vec<String>,
    ) -> Result<CheckReport, IpcError> {
        let report_path = std::env::temp_dir().join(format!(
            "kicad-mcp-check-{}-{}.json",
            std::process::id(),
            Uuid::new_v4()
        ));
        let source_index = arguments.len().saturating_sub(1);
        arguments.splice(
            source_index..source_index,
            ["--output".to_string(), report_path.display().to_string()],
        );
        let mut result = self.run_check(executable, arguments)?;
        if result.exit_code == 0 {
            let mut file = fs::File::open(&report_path)
                .map_err(|error| IpcError::Transport(error.to_string()))?;
            let mut bytes = Vec::new();
            file.by_ref()
                .take(self.safety.max_output_bytes as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|error| IpcError::Transport(error.to_string()))?;
            result.truncated = bytes.len() > self.safety.max_output_bytes;
            bytes.truncate(self.safety.max_output_bytes);
            result.report = String::from_utf8_lossy(&bytes).into_owned();
        }
        let _ = fs::remove_file(report_path);
        Ok(result)
    }

    fn run_check(
        &self,
        executable: PathBuf,
        arguments: Vec<String>,
    ) -> Result<CheckReport, IpcError> {
        let mut child = Command::new(&executable)
            .args(&arguments)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| IpcError::Transport(error.to_string()))?;
        let stdout = child.stdout.take().ok_or(IpcError::MissingResponse)?;
        let stderr = child.stderr.take().ok_or(IpcError::MissingResponse)?;
        let stdout_reader = read_bounded(stdout, self.safety.max_output_bytes);
        let stderr_reader = read_bounded(stderr, self.safety.max_output_bytes);
        let status = match child
            .wait_timeout(self.safety.cli_timeout)
            .map_err(|error| IpcError::Transport(error.to_string()))?
        {
            Some(status) => status,
            None => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(IpcError::Api {
                    status: "CLI_TIMEOUT".to_string(),
                    message: format!(
                        "kicad-cli exceeded the {} second limit",
                        self.safety.cli_timeout.as_secs()
                    ),
                });
            }
        };
        let stdout = stdout_reader.join().map_err(|_| IpcError::Api {
            status: "INTERNAL_STATE".to_string(),
            message: "stdout reader thread panicked".to_string(),
        })?;
        let stderr = stderr_reader.join().map_err(|_| IpcError::Api {
            status: "INTERNAL_STATE".to_string(),
            message: "stderr reader thread panicked".to_string(),
        })?;
        let total_len = stdout.len().saturating_add(stderr.len());
        let mut report = String::from_utf8_lossy(&stdout).into_owned();
        if !stderr.is_empty() {
            report.push('\n');
            report.push_str(&String::from_utf8_lossy(&stderr));
        }
        let truncated = total_len > self.safety.max_output_bytes;
        if truncated {
            let mut boundary = self.safety.max_output_bytes.min(report.len());
            while !report.is_char_boundary(boundary) {
                boundary -= 1;
            }
            report.truncate(boundary);
        }
        let mut command = vec![executable.display().to_string()];
        command.extend(arguments);
        Ok(CheckReport {
            command,
            exit_code: status.code().unwrap_or(-1),
            report,
            truncated,
        })
    }
}

fn remove_output_path(path: &Path) -> std::io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn read_bounded<R: Read + Send + 'static>(
    mut reader: R,
    max_bytes: usize,
) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut kept = Vec::with_capacity(max_bytes.min(64 * 1024));
        let mut buffer = [0_u8; 8 * 1024];
        while let Ok(count) = reader.read(&mut buffer) {
            if count == 0 {
                break;
            }
            let remaining = max_bytes.saturating_add(1).saturating_sub(kept.len());
            kept.extend_from_slice(&buffer[..count.min(remaining)]);
        }
        kept
    })
}

impl From<Point> for Vector2 {
    fn from(value: Point) -> Self {
        Self {
            x_nm: value.x_nm,
            y_nm: value.y_nm,
        }
    }
}

fn summarize_sheet_bounded(
    sheet: &SheetInstance,
    remaining: &mut usize,
) -> Option<SchematicSheetSummary> {
    if *remaining == 0 {
        return None;
    }
    *remaining -= 1;
    Some(SchematicSheetSummary {
        name: sheet.name.clone(),
        filename: sheet.filename.clone(),
        page_number: sheet.page_number.clone(),
        path: sheet
            .path
            .as_ref()
            .map(|path| path.path_human_readable.clone())
            .unwrap_or_default(),
        path_ids: sheet
            .path
            .as_ref()
            .map(|path| path.path.iter().map(|id| id.value.clone()).collect())
            .unwrap_or_default(),
        children: sheet
            .children
            .iter()
            .filter_map(|child| summarize_sheet_bounded(child, remaining))
            .collect(),
    })
}

fn capabilities_for(version: &VersionInfo, write_enabled: bool) -> Capabilities {
    Capabilities {
        pcb_ipc: version.major >= 9,
        schematic_ipc: version.major >= 11,
        headless_ipc: version.major >= 11,
        native_export_jobs: version.major >= 11,
        cli_drc_erc: version.major >= 9,
        mutations_enabled: write_enabled,
    }
}

fn item_header(document: DocumentSpecifier, fields: Vec<String>) -> ItemHeader {
    ItemHeader {
        document: Some(document),
        container: None,
        field_mask: if fields.is_empty() {
            None
        } else {
            Some(FieldMask { paths: fields })
        },
    }
}

fn pack<M: Message>(full_name: &str, message: &M) -> Any {
    Any {
        type_url: format!("type.googleapis.com/{full_name}"),
        value: message.encode_to_vec(),
    }
}

fn decode_any<M: Message + Default>(item: &Any, full_name: &str) -> Result<M, IpcError> {
    let expected = format!("type.googleapis.com/{full_name}");
    if item.type_url != expected {
        return Err(IpcError::UnexpectedResponse {
            expected,
            actual: item.type_url.clone(),
        });
    }
    M::decode(item.value.as_slice()).map_err(|error| IpcError::Codec(error.to_string()))
}

fn decode_footprint(item: &Any) -> Result<FootprintSummary, IpcError> {
    let footprint = decode_any::<FootprintInstance>(item, FOOTPRINT_TYPE)?;
    let definition = footprint.definition.as_ref();
    let library = definition
        .and_then(|value| value.id.as_ref())
        .map(|id| format!("{}:{}", id.library_nickname, id.entry_name))
        .unwrap_or_default();
    Ok(FootprintSummary {
        id: footprint
            .id
            .as_ref()
            .map(|id| id.value.clone())
            .unwrap_or_default(),
        reference: field_value(footprint.reference_field.as_ref()),
        value: field_value(footprint.value_field.as_ref()),
        library,
        position_nm: footprint
            .position
            .as_ref()
            .map(point)
            .unwrap_or(Point { x_nm: 0, y_nm: 0 }),
        orientation_degrees: footprint
            .orientation
            .as_ref()
            .map_or(0.0, |angle| angle.value_degrees),
        layer: BoardLayer::try_from(footprint.layer)
            .map(|layer| layer.as_str_name().to_string())
            .unwrap_or_else(|_| "BL_UNKNOWN".to_string()),
        revision: revision(&item.value),
    })
}

fn field_value(field: Option<&crate::proto::v10::kiapi::board::types::Field>) -> String {
    field
        .and_then(|field| field.text.as_ref())
        .and_then(|text| text.text.as_ref())
        .map(|text| text.text.clone())
        .unwrap_or_default()
}

fn point(value: &Vector2) -> Point {
    Point {
        x_nm: value.x_nm,
        y_nm: value.y_nm,
    }
}

fn summarize_any(item: &Any) -> Result<ItemSummary, IpcError> {
    Ok(ItemSummary {
        id: any_id(item),
        item_type: item
            .type_url
            .rsplit('.')
            .next()
            .unwrap_or("Unknown")
            .to_string(),
        type_url: item.type_url.clone(),
        revision: revision(&item.value),
    })
}

fn any_id(item: &Any) -> Option<String> {
    match item.type_url.strip_prefix("type.googleapis.com/")? {
        FOOTPRINT_TYPE => decode_any::<FootprintInstance>(item, FOOTPRINT_TYPE)
            .ok()?
            .id
            .map(|id| id.value),
        TRACK_TYPE => decode_any::<Track>(item, TRACK_TYPE)
            .ok()?
            .id
            .map(|id| id.value),
        VIA_TYPE => decode_any::<Via>(item, VIA_TYPE)
            .ok()?
            .id
            .map(|id| id.value),
        _ => None,
    }
}

fn revision(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn paginate<T>(items: Vec<T>, cursor: usize, limit: usize, max_limit: usize) -> Page<T> {
    let total = items.len();
    let start = min(cursor, total);
    let page_size = limit.clamp(1, max_limit);
    let end = min(start + page_size, total);
    let next_cursor = (end < total).then_some(end);
    Page {
        items: items.into_iter().skip(start).take(page_size).collect(),
        next_cursor,
        total,
    }
}

fn conflict(expected: &str, actual: &str) -> IpcError {
    IpcError::Api {
        status: "REVISION_CONFLICT".to_string(),
        message: format!("expected revision {expected}, current revision is {actual}"),
    }
}

fn validate_item_request_status(status: i32) -> Result<(), IpcError> {
    let parsed = ItemRequestStatus::try_from(status).ok();
    if parsed == Some(ItemRequestStatus::IrsOk) {
        Ok(())
    } else {
        Err(IpcError::Api {
            status: parsed
                .map(|value| value.as_str_name().to_string())
                .unwrap_or_else(|| format!("ITEM_REQUEST_STATUS_{status}")),
            message: "KiCad rejected the item request".to_string(),
        })
    }
}

fn validate_create_response(
    response: CreateItemsResponse,
) -> Result<CreateItemsResponse, IpcError> {
    validate_item_request_status(response.status)?;
    for created in &response.created_items {
        let status = created.status.as_ref().ok_or(IpcError::MissingResponse)?;
        let code = ItemStatusCode::try_from(status.code).ok();
        if code != Some(ItemStatusCode::IscOk) {
            return Err(IpcError::Api {
                status: code
                    .map(|value| value.as_str_name().to_string())
                    .unwrap_or_else(|| format!("ITEM_STATUS_{}", status.code)),
                message: status.error_message.clone(),
            });
        }
    }
    Ok(response)
}

fn validate_update_response(
    response: UpdateItemsResponse,
) -> Result<UpdateItemsResponse, IpcError> {
    validate_item_request_status(response.status)?;
    for updated in &response.updated_items {
        let status = updated.status.as_ref().ok_or(IpcError::MissingResponse)?;
        let code = ItemStatusCode::try_from(status.code).ok();
        if code != Some(ItemStatusCode::IscOk) {
            return Err(IpcError::Api {
                status: code
                    .map(|value| value.as_str_name().to_string())
                    .unwrap_or_else(|| format!("ITEM_STATUS_{}", status.code)),
                message: status.error_message.clone(),
            });
        }
    }
    Ok(response)
}

fn validate_delete_response(
    response: DeleteItemsResponse,
) -> Result<DeleteItemsResponse, IpcError> {
    validate_item_request_status(response.status)?;
    for deleted in &response.deleted_items {
        let status = crate::proto::v10::kiapi::common::commands::ItemDeletionStatus::try_from(
            deleted.status,
        )
        .ok();
        if status != Some(crate::proto::v10::kiapi::common::commands::ItemDeletionStatus::IdsOk) {
            return Err(IpcError::Api {
                status: status
                    .map(|value| value.as_str_name().to_string())
                    .unwrap_or_else(|| format!("ITEM_DELETION_STATUS_{}", deleted.status)),
                message: "KiCad did not delete the requested item".to_string(),
            });
        }
    }
    Ok(response)
}

fn invalid(name: &str, value: i32) -> IpcError {
    IpcError::Api {
        status: "INVALID_ARGUMENT".to_string(),
        message: format!("invalid {name}: {value}"),
    }
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn validate_positive(name: &str, value: i64) -> Result<(), IpcError> {
    if value > 0 {
        Ok(())
    } else {
        Err(IpcError::Api {
            status: "INVALID_ARGUMENT".to_string(),
            message: format!("{name} must be positive"),
        })
    }
}

fn board_path(document: &DocumentSpecifier) -> Result<PathBuf, IpcError> {
    let info = KiCadService::document_info(document);
    let project = info.project_path.ok_or(IpcError::Api {
        status: "PROJECT_PATH_MISSING".to_string(),
        message: "active board has no project path".to_string(),
    })?;
    let board = info.board_filename.ok_or(IpcError::Api {
        status: "BOARD_PATH_MISSING".to_string(),
        message: "active board has no filename".to_string(),
    })?;
    Ok(PathBuf::from(project).join(board))
}

fn schematic_path(document: &DocumentSpecifier) -> Result<PathBuf, IpcError> {
    let project = document.project.as_ref().ok_or(IpcError::Api {
        status: "PROJECT_PATH_MISSING".to_string(),
        message: "active schematic has no project path".to_string(),
    })?;
    Ok(PathBuf::from(&project.path).join(format!("{}.kicad_sch", project.name)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagination_is_bounded() {
        let page = paginate((0..500).collect::<Vec<_>>(), 10, 999, 200);
        assert_eq!(page.items.len(), 200);
        assert_eq!(page.next_cursor, Some(210));
        assert_eq!(page.total, 500);
    }

    #[test]
    fn revisions_are_stable_and_content_sensitive() {
        assert_eq!(revision(b"same"), revision(b"same"));
        assert_ne!(revision(b"same"), revision(b"different"));
    }

    #[test]
    fn capability_gates_kicad_11_features() {
        let v10 = VersionInfo {
            major: 10,
            minor: 0,
            patch: 6,
            full_version: "10.0.6".to_string(),
        };
        let v11 = VersionInfo {
            major: 11,
            minor: 0,
            patch: 0,
            full_version: "11.0.0".to_string(),
        };
        assert!(!capabilities_for(&v10, false).schematic_ipc);
        assert!(capabilities_for(&v11, true).schematic_ipc);
        assert!(capabilities_for(&v11, true).mutations_enabled);
    }

    #[test]
    fn rejects_per_item_mutation_failure_before_commit() {
        use crate::proto::v10::kiapi::common::commands::{ItemCreationResult, ItemStatus};

        let error = validate_create_response(CreateItemsResponse {
            header: None,
            status: ItemRequestStatus::IrsOk as i32,
            created_items: vec![ItemCreationResult {
                status: Some(ItemStatus {
                    code: ItemStatusCode::IscInvalidData as i32,
                    error_message: "invalid geometry".to_string(),
                }),
                item: None,
            }],
        })
        .unwrap_err();
        assert!(matches!(
            error,
            IpcError::Api { status, message }
                if status == "ISC_INVALID_DATA" && message == "invalid geometry"
        ));
    }

    #[cfg(unix)]
    #[test]
    fn cli_execution_is_killed_after_timeout() {
        let service = KiCadService {
            ipc: IpcClient::new(IpcConfig::default()),
            safety: SafetyConfig {
                write_enabled: false,
                project_root: None,
                max_page_size: 200,
                max_output_bytes: 1024,
                cli_timeout: Duration::from_millis(20),
            },
            latest_drc: Arc::new(Mutex::new(None)),
            mutation_lock: Arc::new(Mutex::new(())),
        };
        let error = service
            .run_check(PathBuf::from("/bin/sleep"), vec!["1".to_string()])
            .unwrap_err();
        assert!(matches!(
            error,
            IpcError::Api { status, .. } if status == "CLI_TIMEOUT"
        ));
    }

    #[test]
    fn write_requires_explicit_opt_in_and_root() {
        let service = KiCadService {
            ipc: IpcClient::new(IpcConfig::default()),
            safety: SafetyConfig {
                write_enabled: false,
                project_root: None,
                max_page_size: 200,
                max_output_bytes: 1024,
                cli_timeout: Duration::from_secs(10),
            },
            latest_drc: Arc::new(Mutex::new(None)),
            mutation_lock: Arc::new(Mutex::new(())),
        };
        assert!(service.require_write().is_err());
    }

    #[cfg(unix)]
    #[test]
    fn project_root_rejects_outside_paths_and_symlink_escape() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let inside_file = root.path().join("inside.kicad_pcb");
        let outside_file = outside.path().join("outside.kicad_pcb");
        fs::write(&inside_file, "inside").unwrap();
        fs::write(&outside_file, "outside").unwrap();
        let escape = root.path().join("escape.kicad_pcb");
        symlink(&outside_file, &escape).unwrap();
        let service = KiCadService {
            ipc: IpcClient::new(IpcConfig::default()),
            safety: SafetyConfig {
                write_enabled: true,
                project_root: Some(root.path().canonicalize().unwrap()),
                max_page_size: 200,
                max_output_bytes: 1024,
                cli_timeout: Duration::from_secs(10),
            },
            latest_drc: Arc::new(Mutex::new(None)),
            mutation_lock: Arc::new(Mutex::new(())),
        };
        assert!(service.ensure_allowed(&inside_file).is_ok());
        assert!(service.ensure_allowed(&outside_file).is_err());
        assert!(service.ensure_allowed(&escape).is_err());
        assert!(service.ensure_allowed_output(&escape).is_err());

        let final_output = root.path().join("artifact.step");
        let (canonical_output, staged_output) = service.staged_output(&final_output).unwrap();
        fs::write(&staged_output, b"artifact").unwrap();
        service
            .publish_staged_output(&staged_output, &canonical_output)
            .unwrap();
        assert_eq!(fs::read(&final_output).unwrap(), b"artifact");
        assert!(service.ensure_allowed_output(&final_output).is_err());

        let outside_document = DocumentSpecifier {
            r#type: DocumentType::DoctypePcb as i32,
            project: Some(crate::proto::v10::kiapi::common::types::ProjectSpecifier {
                name: "outside".to_string(),
                path: outside.path().display().to_string(),
            }),
            identifier: Some(
                crate::proto::v10::kiapi::common::types::document_specifier::Identifier::BoardFilename(
                    "outside.kicad_pcb".to_string(),
                ),
            ),
        };
        assert!(service.ensure_document_allowed(&outside_document).is_err());
    }
}
