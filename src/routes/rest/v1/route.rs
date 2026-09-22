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

use super::handlers::{self, CatalogOperation};

pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/v1", axum_get(get))
}

#[ores_route(operation = handlers::get_catalog)]
pub async fn get(State(state): State<AppState>) -> Response {
    let request = OperationRequestData::new(RpcPayloadCodec::Json);
    request.insert_path::<CatalogOperation>(NoSection);
    request.insert_query::<CatalogOperation>(NoSection);
    request.insert_headers::<CatalogOperation>(NoSection);
    request.insert_body::<CatalogOperation>(NoSection);
    let base = OperationContext::http(state);
    let context = TypedOperationContext::<AppState, CatalogOperation>::new(base, request);
    match handlers::__ores_invoke_get_catalog(context).await {
        Ok(body) => Json(body).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
