//! PDF download adapter.
//!
//! The webview cannot fetch most publisher PDFs directly (CORS), so PDFs are
//! downloaded here and handed to the frontend as raw bytes. Downloads are
//! cached in the app cache directory.

use std::io;
use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};
use std::time::Duration;

use paperflow_core::{Error, Result};
use reqwest::{StatusCode, Url};

use crate::search::describe_error;

pub const MAX_PDF_BYTES: usize = 32 * 1024 * 1024;
const MAX_REDIRECTS: usize = 5;

pub struct PdfFetcher {
    http: reqwest::Client,
    cache_dir: PathBuf,
    max_bytes: usize,
}

impl PdfFetcher {
    pub fn new(cache_dir: PathBuf) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("PaperFlow/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            // Large PDFs through a slow proxy can take well over a minute.
            .read_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(180))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= MAX_REDIRECTS {
                    return attempt.error(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "too many redirects",
                    ));
                }

                match validate_pdf_url(attempt.url().as_str()) {
                    Ok(_) => attempt.follow(),
                    Err(error) => {
                        attempt.error(io::Error::new(io::ErrorKind::PermissionDenied, error))
                    }
                }
            }))
            .build()
            .map_err(|e| Error::Network(format!("failed to build PDF client: {e}")))?;

        Ok(Self {
            http,
            cache_dir,
            max_bytes: MAX_PDF_BYTES,
        })
    }

    pub async fn fetch(&self, raw_url: &str) -> Result<Vec<u8>> {
        let url = validate_pdf_url(raw_url).map_err(Error::Config)?;
        let cache_path = self
            .cache_dir
            .join(format!("{}.pdf", cache_key(url.as_str())));

        if let Ok(bytes) = tokio::fs::read(&cache_path).await
            && bytes.len() <= self.max_bytes
            && looks_like_pdf(&bytes)
        {
            tracing::debug!(url = %redacted_url(&url), "PDF cache hit");
            return Ok(bytes);
        }

        ensure_public_host(&url).await.map_err(Error::Config)?;

        let response = self
            .http
            .get(url.clone())
            .header(
                "Accept",
                "application/pdf, application/octet-stream;q=0.9, */*;q=0.1",
            )
            .send()
            .await
            .map_err(|e| Error::Network(describe_error("PDF 下载失败", &e)))?;

        if !response.status().is_success() {
            return Err(download_status_error(response.status()));
        }

        // Redirects are followed by reqwest; validate the final host too.
        ensure_public_host(response.url())
            .await
            .map_err(Error::Config)?;

        if let Some(length) = response.content_length()
            && length as usize > self.max_bytes
        {
            return Err(too_large_error(self.max_bytes));
        }

        let mut response = response;
        let mut bytes = Vec::new();

        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| Error::Network(describe_error("读取 PDF 失败", &e)))?
        {
            if bytes.len() + chunk.len() > self.max_bytes {
                return Err(too_large_error(self.max_bytes));
            }
            bytes.extend_from_slice(&chunk);
        }

        if !looks_like_pdf(&bytes) {
            return Err(Error::Parse(
                "下载到的内容不是 PDF（可能是出版商登录页或订阅页）".into(),
            ));
        }

        if let Err(error) = self.write_cache(&cache_path, &bytes).await {
            tracing::warn!(%error, "failed to cache PDF");
        }

        Ok(bytes)
    }

    async fn write_cache(&self, path: &Path, bytes: &[u8]) -> Result<()> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| Error::Storage(format!("创建 PDF 缓存目录失败: {e}")))?;
        }

        let temp = path.with_extension("pdf.part");
        tokio::fs::write(&temp, bytes)
            .await
            .map_err(|e| Error::Storage(format!("写入 PDF 缓存失败: {e}")))?;

        #[cfg(windows)]
        if path.exists() {
            let _ = tokio::fs::remove_file(path).await;
        }

        tokio::fs::rename(&temp, path)
            .await
            .map_err(|e| Error::Storage(format!("保存 PDF 缓存失败: {e}")))
    }
}

fn too_large_error(max_bytes: usize) -> Error {
    Error::Parse(format!("PDF 超过 {} MiB 上限", max_bytes / (1024 * 1024)))
}

fn download_status_error(status: StatusCode) -> Error {
    match status.as_u16() {
        401 | 403 => Error::Network("该 PDF 需要订阅或登录，无法直接下载（HTTP 403）".into()),
        404 => Error::Network("PDF 链接不存在（HTTP 404）".into()),
        429 => Error::Network("PDF 下载被限流，请稍后重试（HTTP 429）".into()),
        500..=599 => Error::Network(format!("PDF 服务器错误（HTTP {status}）")),
        _ => Error::Network(format!("PDF 下载失败（HTTP {status}）")),
    }
}

pub(crate) fn validate_pdf_url(raw: &str) -> std::result::Result<Url, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("PDF 链接为空".into());
    }

    let url = Url::parse(raw).map_err(|e| format!("PDF 链接无效: {e}"))?;

    if url.scheme() != "https" {
        return Err("只允许打开 HTTPS PDF 链接".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("PDF 链接不能包含用户名或密码".into());
    }

    let host = url
        .host_str()
        .ok_or_else(|| "PDF 链接缺少主机名".to_string())?;
    validate_host(host)?;

    Ok(url)
}

fn validate_host(raw_host: &str) -> std::result::Result<(), String> {
    let host = raw_host
        .trim()
        .trim_matches(['[', ']'])
        .to_ascii_lowercase();

    if host.is_empty() {
        return Err("PDF 链接缺少主机名".into());
    }
    if host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
        || host.ends_with(".home.arpa")
    {
        return Err("不允许访问本地或内网地址".into());
    }
    if !host.contains('.') {
        return Err("不允许使用单标签主机名".into());
    }
    if let Ok(ip) = host.parse::<IpAddr>()
        && !is_public_ip(ip)
    {
        return Err("不允许访问私网 IP".into());
    }

    Ok(())
}

async fn ensure_public_host(url: &Url) -> std::result::Result<(), String> {
    let host = url
        .host_str()
        .ok_or_else(|| "PDF 链接缺少主机名".to_string())?;
    validate_host(host)?;

    if host.trim_matches(['[', ']']).parse::<IpAddr>().is_ok() {
        return Ok(());
    }

    let port = url.port_or_known_default().unwrap_or(443);
    let addresses: Vec<_> = tokio::net::lookup_host((host, port))
        .await
        .map_err(|e| format!("无法解析 PDF 主机: {e}"))?
        .collect();

    if addresses.is_empty() {
        return Err("PDF 主机没有可用地址".into());
    }
    if addresses.iter().any(|address| !is_public_ip(address.ip())) {
        return Err("PDF 主机解析到私网地址".into());
    }

    Ok(())
}

pub(crate) fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(address) => is_public_v4(address),
        IpAddr::V6(address) => {
            if let Some(mapped) = address.to_ipv4_mapped() {
                return is_public_v4(mapped);
            }

            let segments = address.segments();
            if address.is_unspecified() || address.is_loopback() {
                return false;
            }
            if segments[0] & 0xfe00 == 0xfc00 {
                return false; // fc00::/7 unique local
            }
            if segments[0] & 0xffc0 == 0xfe80 {
                return false; // fe80::/10 link local
            }
            if segments[0] & 0xff00 == 0xff00 {
                return false; // ff00::/8 multicast
            }
            if segments[0] == 0x2001 && segments[1] == 0x0db8 {
                return false; // documentation range
            }

            true
        }
    }
}

fn is_public_v4(address: Ipv4Addr) -> bool {
    let octets = address.octets();

    !(octets[0] == 0
        || octets[0] == 10
        || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        || octets[0] == 127
        || (octets[0] == 169 && octets[1] == 254)
        || (octets[0] == 172 && (16..=31).contains(&octets[1]))
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 2)
        || (octets[0] == 192 && octets[1] == 168)
        || (octets[0] == 198 && (octets[1] == 18 || octets[1] == 19))
        || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
        || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113)
        || octets[0] >= 224)
}

pub(crate) fn looks_like_pdf(bytes: &[u8]) -> bool {
    bytes.windows(5).take(1024).any(|window| window == b"%PDF-")
}

pub(crate) fn cache_key(url: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in url.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn redacted_url(url: &Url) -> String {
    format!(
        "{}://{}{}",
        url.scheme(),
        url.host_str().unwrap_or_default(),
        url.path()
    )
}

#[cfg(test)]
mod tests;
