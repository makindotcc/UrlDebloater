mod error;

use anyhow::Context;
use axum::extract::{Query, Request, State};
use axum::{
    error_handling::HandleErrorLayer,
    http::{header, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    BoxError, Router,
};
use axum_macros::debug_handler;
use error::{AppResult, UserError};
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tower::util::option_layer;
use tower::ServiceBuilder;
use tower_governor::key_extractor::SmartIpKeyExtractor;
use tower_governor::{governor::GovernorConfigBuilder, GovernorError, GovernorLayer};
use tower_http::trace::TraceLayer;
use tracing::debug;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;
use url::Url;
use urldebloater_urlwasher::clearurls;
use urldebloater_urlwasher::clearurls::CatalogLoader;
use urldebloater_urlwasher::clearurls::ClearUrls;
use urldebloater_urlwasher::clearurls::ReqwestCatalogLoader;
use urldebloater_urlwasher::redirect_resolver::DefaultRedirectResolver;
use urldebloater_urlwasher::redirect_resolver::WithoutMixerRedirectResolver;
use urldebloater_urlwasher::TwitterPolicy;
use urldebloater_urlwasher::WasherConfig;
use urldebloater_urlwasher::{UrlWasher, WashOptions};

const CLEARURLS_CACHE_DURATION: Duration = Duration::from_secs(60 * 60 * 24);

type AppUrlWasher = UrlWasher<DefaultRedirectResolver>;
type UrlWasherState = Arc<RwLock<AppUrlWasher>>;

#[derive(Clone)]
struct AppState {
    url_washer: UrlWasherState,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .pretty()
        .with_line_number(false)
        .with_file(false)
        .init();

    let url_washer = create_url_washer_from_cache().await;
    tokio::spawn(run_cleanurls_updater(url_washer.clone()));
    let app_state = AppState { url_washer };

    let bind_addr = std::env::var("LISTEN_ADDRESS").unwrap_or_else(|_| "127.0.0.1:7777".into());
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("Could not bind tcp listener");
    info!("Starting listening...");
    axum::serve(
        listener,
        app(app_state, true).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

async fn create_url_washer_from_cache() -> UrlWasherState {
    let catalog = clearurls::cache::load(
        &ReqwestCatalogLoader,
        "clearurls_cache.json".into(),
        CLEARURLS_CACHE_DURATION,
    )
    .await
    .unwrap_or_else(|(catalog, err)| {
        error!("Failed to load clearurls catalog from cache: {err:?}");
        catalog
    });
    Arc::new(RwLock::new(create_url_washer(catalog)))
}

fn create_url_washer(clearurls_catalog: clearurls::DataCatalog) -> AppUrlWasher {
    AppUrlWasher::new(
        DefaultRedirectResolver::WithoutMixer(WithoutMixerRedirectResolver::default()),
        WasherConfig {
            clear_urls: Some(ClearUrls::new(clearurls_catalog)),
            twitter_policy: TwitterPolicy::Original,
        },
    )
}

async fn run_cleanurls_updater(url_washer: UrlWasherState) {
    loop {
        tokio::time::sleep(CLEARURLS_CACHE_DURATION).await;
        debug!("Updating clearurls catalog...");

        match ReqwestCatalogLoader.load_catalog().await {
            Ok(new_catalog) => {
                let new_washer = create_url_washer(new_catalog);
                *url_washer.write().await = new_washer;
                info!("Successfully updated clearurls catalog");
            }
            Err(err) => {
                error!("Failed to update clearurls catalog: {err:?}");
            }
        }
    }
}

async fn allow_cors(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    response
}

fn app(state: AppState, rate_limit: bool) -> Router {
    Router::new()
        .route("/wash", get(wash))
        .layer(
            ServiceBuilder::new()
                .layer(middleware::from_fn(allow_cors))
                .layer(TraceLayer::new_for_http())
                .layer(HandleErrorLayer::new(handle_service_err))
                .timeout(Duration::from_secs(10))
                .layer(option_layer(if rate_limit {
                    Some(GovernorLayer {
                        config: Box::leak(Box::new(
                            GovernorConfigBuilder::default()
                                .per_second(5)
                                .burst_size(10)
                                .key_extractor(SmartIpKeyExtractor)
                                .finish()
                                .unwrap(),
                        )),
                    })
                } else {
                    None
                })),
        )
        .with_state(state)
}

#[derive(Deserialize)]
struct WashQuery {
    url: String,
}

#[debug_handler]
async fn wash(State(state): State<AppState>, Query(query): Query<WashQuery>) -> AppResult<String> {
    const MAX_URL_LENGTH: usize = 16384;
    if query.url.len() > MAX_URL_LENGTH {
        return Err(UserError::TooLongUrl.into());
    }
    let url = Url::parse(&query.url).map_err(|_| UserError::InvalidUrl)?;

    let washed = state
        .url_washer
        .read()
        .await
        .wash(&url, WashOptions::default())
        .await
        .context("wash url")?;
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
    use super::*;
    use axum::{
        body::Body,
        http::{header, Request},
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    use urldebloater_urlwasher::redirect_resolver::DefaultRedirectResolver;

    #[tokio::test]
    async fn cleans_url() {
        let data_catalog =
            serde_json::from_str(include_str!("../../../assets/cleanurls_rules.json")).unwrap();
        let url_washer: UrlWasher<DefaultRedirectResolver> = UrlWasher::new(
            DefaultRedirectResolver::None,
            WasherConfig {
                clear_urls: Some(ClearUrls::new(data_catalog)),
                twitter_policy: TwitterPolicy::Original,
            },
        );
        let test_state = AppState {
            url_washer: Arc::new(RwLock::new(url_washer)),
        };
        let app = app(test_state, false);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/wash?url=https://youtube.com/watch?v=d2348942389234%26t=123%26si=fdgfsdfg")
                    .header(header::ORIGIN, "https://example.com")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap(),
            "*"
        );

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body = String::from_utf8_lossy(&body);
        assert_eq!(body, "https://youtube.com/watch?v=d2348942389234&t=123");
    }
}
