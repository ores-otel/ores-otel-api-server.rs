#![forbid(unsafe_code)]

use ores_api_docs::{NoSection, OperationSpec, RpcPayloadCodec, TypedOperationContext};
use ores_api_docs_operation_macros::ores_operation;
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HealthBody {
    pub ok: bool,
    pub service: String,
}

pub struct HealthOperation;

impl OperationSpec for HealthOperation {
    type Path = NoSection;
    type Query = NoSection;
    type RequestHeaders = NoSection;
    type RequestBody = NoSection;
    type ResponseBody = HealthBody;
    type ResponseHeaders = NoSection;
    type ResponseTrailers = NoSection;
    type Error = ();

    const KEY: &'static str = "ores.otel.health.get";
    const CODECS: &'static [RpcPayloadCodec] = &[RpcPayloadCodec::Json];
    const DEFAULT_CODEC: RpcPayloadCodec = RpcPayloadCodec::Json;
}

pub fn body() -> HealthBody {
    HealthBody {
        ok: true,
        service: "ores-otel-api-server".to_owned(),
    }
}

#[ores_operation(
    spec = HealthOperation,
    key = "ores.otel.health.get",
    codecs("json"),
    default_codec = "json",
    audiences("server"),
    scope = "regular"
)]
pub async fn get_health(
    _ctx: TypedOperationContext<AppState, HealthOperation>,
) -> Result<HealthBody, ()> {
    Ok(body())
}
