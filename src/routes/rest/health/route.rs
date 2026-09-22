#![forbid(unsafe_code)]

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get as axum_get,
    Json, Router,
};
use ores_api_docs::{
    NoSection, OperationContext, OperationRequestData, RpcPayloadCodec, TypedOperationContext,
};
use ores_api_docs_operation_macros::ores_route;

use crate::AppState;

use super::handlers::{self, HealthOperation};

pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/health", axum_get(get))
}

#[ores_route(operation = handlers::get_health)]
pub async fn get(State(state): State<AppState>) -> Response {
    let request = OperationRequestData::new(RpcPayloadCodec::Json);
    request.insert_path::<HealthOperation>(NoSection);
    request.insert_query::<HealthOperation>(NoSection);
    request.insert_headers::<HealthOperation>(NoSection);
    request.insert_body::<HealthOperation>(NoSection);
    let base = OperationContext::http(state);
    let context = TypedOperationContext::<AppState, HealthOperation>::new(base, request);
    match handlers::__ores_invoke_get_health(context).await {
        Ok(body) => Json(body).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
