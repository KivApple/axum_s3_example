# Axum integration with S3-compatible storage example

Uses axum and [rust-s3](https://crates.io/crates/rust-s3) crates.

Implements two endpoints:

- POST `/uploads` - Upload a file from "`upload`" field 
  of `multipart/form-data` request body.
- GET `/uploads/:name` - Download a previously uploaded file.

Configured using following environment variables:

- `HOST` - an address for binding of listening socket
- `PORT` - a port to listen
- `UPLOAD_BUCKET_NAME`
- `UPLOAD_BUCKET_REGION`
- `UPLOAD_BUCKET_ACCESS_KEY`
- `UPLOAD_BUCKET_SECRET_KEY`
- `UPLOAD_BUCKET_ENDPOINT`

More info: https://eternal-search.com/axum-s3
