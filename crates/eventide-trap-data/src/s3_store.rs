//! Minimal S3 / RustFS client via rusty-s3 + reqwest (path-style).

use anyhow::{Context, Result};
use rusty_s3::{Bucket, Credentials, S3Action, UrlStyle};
use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;
use url::Url;

const PRESIGN_SECS: u64 = 600;

#[derive(Clone)]
pub struct S3Store {
    bucket: Bucket,
    creds: Credentials,
    http: reqwest::Client,
    bucket_name: String,
}

impl S3Store {
    pub async fn connect(
        endpoint: &str,
        access_key: &str,
        secret_key: &str,
        _region: &str,
        bucket: &str,
    ) -> Result<Arc<Self>> {
        if endpoint.trim().is_empty() {
            anyhow::bail!("s3_endpoint is required for MIB object storage");
        }
        if access_key.is_empty() || secret_key.is_empty() {
            anyhow::bail!("s3_access_key / s3_secret_key required");
        }
        let endpoint_url = Url::parse(endpoint).context("parse s3_endpoint")?;
        let bucket_name = bucket.to_string();
        let bucket_obj = Bucket::new(
            endpoint_url,
            UrlStyle::Path,
            Cow::Owned(bucket_name.clone()),
            Cow::Borrowed("us-east-1"),
        )
        .context("create s3 bucket handle")?;
        let creds = Credentials::new(access_key.to_string(), secret_key.to_string());
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .context("reqwest client")?;
        let store = Arc::new(Self {
            bucket: bucket_obj,
            creds,
            http,
            bucket_name,
        });
        store.ensure_bucket().await?;
        Ok(store)
    }

    pub fn bucket(&self) -> &str {
        &self.bucket_name
    }

    pub async fn ensure_bucket(&self) -> Result<()> {
        let action = self.bucket.create_bucket(&self.creds);
        let url = action.sign(Duration::from_secs(PRESIGN_SECS));
        let resp = self
            .http
            .put(url)
            .send()
            .await
            .context("create_bucket request")?;
        let status = resp.status();
        if status.is_success() || status.as_u16() == 409 {
            if status.is_success() {
                tracing::info!(bucket = %self.bucket_name, "created S3 bucket");
            }
            return Ok(());
        }
        let body = resp.text().await.unwrap_or_default();
        if body.contains("BucketAlreadyOwnedByYou")
            || body.contains("BucketAlreadyExists")
            || body.to_ascii_lowercase().contains("already")
        {
            return Ok(());
        }
        tracing::warn!(
            bucket = %self.bucket_name,
            %status,
            "create_bucket returned non-success; continuing (bucket may already exist): {body}"
        );
        Ok(())
    }

    pub async fn put_object(&self, key: &str, bytes: &[u8]) -> Result<()> {
        let action = self.bucket.put_object(Some(&self.creds), key);
        let url = action.sign(Duration::from_secs(PRESIGN_SECS));
        let resp = self
            .http
            .put(url)
            .header("content-length", bytes.len())
            .body(bytes.to_vec())
            .send()
            .await
            .with_context(|| format!("put s3://{}/{}", self.bucket_name, key))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("put s3://{}/{} -> {status}: {body}", self.bucket_name, key);
        }
        Ok(())
    }

    pub async fn get_object(&self, key: &str) -> Result<Vec<u8>> {
        let action = self.bucket.get_object(Some(&self.creds), key);
        let url = action.sign(Duration::from_secs(PRESIGN_SECS));
        let resp = self
            .http
            .get(url)
            .send()
            .await
            .with_context(|| format!("get s3://{}/{}", self.bucket_name, key))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("get s3://{}/{} -> {status}: {body}", self.bucket_name, key);
        }
        Ok(resp.bytes().await.context("read s3 body")?.to_vec())
    }

    pub async fn delete_object(&self, key: &str) -> Result<()> {
        let action = self.bucket.delete_object(Some(&self.creds), key);
        let url = action.sign(Duration::from_secs(PRESIGN_SECS));
        let resp = self
            .http
            .delete(url)
            .send()
            .await
            .with_context(|| format!("delete s3://{}/{}", self.bucket_name, key))?;
        if !resp.status().is_success() && resp.status().as_u16() != 404 {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "delete s3://{}/{} -> {status}: {body}",
                self.bucket_name,
                key
            );
        }
        Ok(())
    }
}

pub fn object_key(module_id: &str, filename: &str) -> String {
    format!("mibs/{module_id}/{filename}")
}
