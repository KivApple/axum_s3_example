use std::sync::{Arc, Mutex};
use std::path::Path;
use std::ffi::OsStr;
use axum::http::StatusCode;
use axum::extract::multipart::Field;
use axum::response::IntoResponse;
use axum::body::StreamBody;
use async_hash::{Sha256, Digest};
use async_compat::CompatExt;
use futures::TryStreamExt;
use uuid::Uuid;
use s3::{Bucket, Region};
use s3::creds::Credentials;
use s3::error::S3Error;

#[derive(Clone)]
pub struct UploadService {
	bucket: Arc<Bucket>
}

impl UploadService {
	pub fn new() -> Self {
		let bucket_name = std::env::var("UPLOAD_BUCKET_NAME")
			.expect("Expected UPLOAD_BUCKET_NAME environment variable");
		let region = Region::Custom {
			region: std::env::var("UPLOAD_BUCKET_REGION")
				.expect("Expected UPLOAD_BUCKET_REGION environment variable"),
			endpoint: std::env::var("UPLOAD_BUCKET_ENDPOINT")
				.expect("Expected UPLOAD_BUCKET_ENDPOINT environment variable")
		};
		let credentials = Credentials::new(
			Some(
				&std::env::var("UPLOAD_BUCKET_ACCESS_KEY")
					.expect("Expected UPLOAD_BUCKET_ACCESS_KEY environment variable")
			),
			Some(
				&std::env::var("UPLOAD_BUCKET_SECRET_KEY")
					.expect("Expected UPLOAD_BUCKET_SECRET_KEY environment variable")
			),
			None,
			None,
			None
		).unwrap();
		
		let bucket = Bucket::new(&bucket_name, region, credentials).unwrap()
			.with_path_style();
		
		Self {
			bucket: Arc::new(bucket)
		}
	}
	
	pub async fn upload<'a>(&self, field: Field<'a>) -> Result<String, StatusCode> {
		let orig_filename = field.file_name()
			.unwrap_or("file")
			.to_owned();
		let mimetype = field.content_type()
			.unwrap_or("application/octet-stream")
			.to_owned();
		let digest = Arc::new(Mutex::new(Sha256::new()));
		let mut reader = field
			.map_ok(|chunk| {
				if let Ok(mut digest) = digest.lock() {
					digest.update(&chunk);
				}
				chunk
			})
			.map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))
			.into_async_read()
			.compat();
		let tmp_filename = format!("tmp/{}.bin", Uuid::new_v4());
		self.bucket.put_object_stream_with_content_type(
			&mut reader,
			&tmp_filename,
			&mimetype
		)
			.await
			.map_err(|err| {
				log::error!("S3 upload error: {:?}", err);
				StatusCode::INTERNAL_SERVER_ERROR
			})?;
		drop(reader); // Release digest borrow
		let mut result = Err(StatusCode::INTERNAL_SERVER_ERROR);
		if let Some(digest) = Arc::into_inner(digest).and_then(|m| m.into_inner().ok()) {
			let digest = hex::encode(digest.finalize());
			let ext = Path::new(&orig_filename).extension().and_then(OsStr::to_str);
			let mut filename = if let Some(ext) = ext {
				format!("{}.{}", digest, ext)
			} else {
				digest
			};
			filename.make_ascii_lowercase();
			match self.bucket.copy_object_internal(&tmp_filename, &filename).await {
				Ok(_) => result = Ok(format!("/uploads/{}", &filename)),
				Err(err) => log::error!("S3 copy error: {:?}", err)
			}
		}
		if let Err(err) = self.bucket.delete_object(&tmp_filename).await {
			log::error!("S3 delete error: {:?}", err);
		}
		result
	}
	
	pub async fn download(
		&self,
		filename: &str
	) -> Result<impl IntoResponse, StatusCode> {
		let stream = self.bucket.get_object_stream(filename)
			.await
			.map_err(|err| match err {
				S3Error::HttpFailWithBody(status_code, body) => match status_code {
					404 => StatusCode::NOT_FOUND,
					_ => {
						log::error!(
							"S3 download HTTP error with code={} and body={:?}",
							status_code,
							body
						);
						StatusCode::INTERNAL_SERVER_ERROR
					}
				}
				err => {
					log::error!("S3 download error: {:?}", err);
					StatusCode::INTERNAL_SERVER_ERROR
				}
			})?;
		Ok(StreamBody::from(stream.bytes))
	}
}
