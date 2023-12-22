use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use axum::extract::{Query, State};
use axum::{
    error_handling::HandleErrorLayer, http::StatusCode, response::IntoResponse, routing::get,
    BoxError, Router,
};
use axum_macros::debug_handler;
use error::{AppResult, UserError};
use serde::Deserialize;
use tower::ServiceBuilder;
use tower_governor::{governor::GovernorConfigBuilder, GovernorError, GovernorLayer};
use tower_http::trace::TraceLayer;
use tower_http::ServiceBuilderExt;
use tracing::error;
use tracing_subscriber::EnvFilter;
use url::Url;
use urlwasher::UrlWasher;

mod error;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .pretty()
        .with_line_number(false)
        .with_file(false)
        .init();

    let listener = tokio::net::TcpListener::bind("0.0.0.0:7777")
        .await
        .expect("Could not bind tcp listener");
    axum::serve(
        listener,
        app(true).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

fn app(rate_limit: bool) -> Router {
    let url_washer = UrlWasher::default();
    Router::new()
        .route("/wash", get(wash))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(HandleErrorLayer::new(handle_service_err))
                .timeout(Duration::from_secs(10))
                .option_layer(if rate_limit {
                    Some(GovernorLayer {
                        config: Box::leak(Box::new(
                            GovernorConfigBuilder::default()
                                .per_second(5)
                                .burst_size(10)
                                .finish()
                                .unwrap(),
                        )),
                    })
                } else {
                    None
                })
                .compression(),
        )
        .with_state(Arc::new(url_washer))
}

#[derive(Deserialize)]
struct WashQuery {
    url: String,
}

#[debug_handler]
async fn wash(
    State(washer): State<Arc<UrlWasher>>,
    Query(query): Query<WashQuery>,
) -> AppResult<String> {
    let url = Url::parse(&query.url).map_err(|_| UserError::InvalidUrl)?;
    let washed = washer.wash(&url).await.context("wash url")?;
    Ok(washed.unwrap_or(url).to_string())
}

async fn handle_service_err(err: BoxError) -> impl IntoResponse {
    if let Some(GovernorError::TooManyRequests { .. }) = err.downcast_ref::<GovernorError>() {
        (StatusCode::TOO_MANY_REQUESTS).into_response()
    } else {
        error!("Internal server error: {err:?}");
        (StatusCode::INTERNAL_SERVER_ERROR).into_response()
    }
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn cleans_url() {
        let app = app(false);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/wash?url=https://youtube.com/watch?v=d2348942389234%26t=123%26si=fdgfsdfg")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body = String::from_utf8_lossy(&body);
        assert_eq!(body, "https://youtube.com/watch?v=d2348942389234&t=123");
    }
}
