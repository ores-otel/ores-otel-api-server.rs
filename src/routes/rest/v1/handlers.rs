#![forbid(unsafe_code)]

use ores_api_docs::{NoSection, OperationSpec, RpcPayloadCodec, TypedOperationContext};
use ores_api_docs_operation_macros::ores_operation;
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Catalog {
    pub resource: String,
}

pub struct CatalogOperation;

impl OperationSpec for CatalogOperation {
    type Path = NoSection;
    type Query = NoSection;
    type RequestHeaders = NoSection;
    type RequestBody = NoSection;
    type ResponseBody = Catalog;
    type ResponseHeaders = NoSection;
    type ResponseTrailers = NoSection;
    type Error = ();

    const KEY: &'static str = "ores.otel.catalog.get";
    const CODECS: &'static [RpcPayloadCodec] = &[RpcPayloadCodec::Json];
    const DEFAULT_CODEC: RpcPayloadCodec = RpcPayloadCodec::Json;
}

pub fn catalog() -> Catalog {
    Catalog {
        resource: "TelemetryRecord".to_owned(),
    }
}

#[ores_operation(
    spec = CatalogOperation,
    key = "ores.otel.catalog.get",
    codecs("json"),
    default_codec = "json",
    audiences("server"),
    scope = "regular"
)]
pub async fn get_catalog(
    _ctx: TypedOperationContext<AppState, CatalogOperation>,
) -> Result<Catalog, ()> {
    Ok(catalog())
}
