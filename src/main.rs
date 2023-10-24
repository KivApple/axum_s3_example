mod uploader;

use std::str::FromStr;
use axum::{Extension, Json};
use axum::extract::{DefaultBodyLimit, Multipart, Path};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::http::header::CACHE_CONTROL;
use axum::response::IntoResponse;
use uploader::UploadService;

#[derive(Debug, serde::Serialize)]
struct UploadResponse {
    pub url: String
}

async fn upload_file(
    Extension(upload_service): Extension<UploadService>,
    mut multipart: Multipart
) -> Result<impl IntoResponse, StatusCode> {
    while let Some(field) = multipart.next_field().await.map_err(|_|
        StatusCode::INTERNAL_SERVER_ERROR
    )? {
        if let Some("upload") = field.name() {
            let url = upload_service.upload(field).await?;
            return Ok(Json(UploadResponse { url }));
        }
    }
    Err(StatusCode::BAD_REQUEST)
}

async fn download_file(
    Path(path): Path<String>,
    Extension(upload_service): Extension<UploadService>
) -> Result<impl IntoResponse, StatusCode> {
    let body = upload_service.download(&path).await?;
    let headers = HeaderMap::from_iter([
        (CACHE_CONTROL, HeaderValue::from_str("max-age=31536000").unwrap()) // One year
    ]);
    Ok((headers, body))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    let upload_service = UploadService::new();
    let router = axum::Router::new()
        .route("/uploads", axum::routing::post(upload_file))
        .route("/uploads/*path", axum::routing::get(download_file))
        .layer(Extension(upload_service))
        .layer(DefaultBodyLimit::max(8 * 1024 * 1024));
    let address = std::env::var("HOST").expect("Expected HOST environment variable");
    let port = std::env::var("PORT").expect("Expected PORT environment variable")
        .parse::<u16>().expect("PORT environment variable must be an integer");
    log::info!("Listening on http://{}:{}/", address, port);
    axum::Server::bind(
        &std::net::SocketAddr::new(
            std::net::IpAddr::from_str(&address).unwrap(),
            port
        )
    ).serve(router.into_make_service()).await?;
    Ok(())
}
