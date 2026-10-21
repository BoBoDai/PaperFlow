use super::*;

#[test]
fn rejects_unsafe_urls() {
    assert!(validate_pdf_url("").is_err());
    assert!(validate_pdf_url("http://arxiv.org/pdf/1706.03762").is_err());
    assert!(validate_pdf_url("https://user:pass@arxiv.org/a.pdf").is_err());
    assert!(validate_pdf_url("https://localhost/a.pdf").is_err());
    assert!(validate_pdf_url("https://printer.local/a.pdf").is_err());
    assert!(validate_pdf_url("https://intranet/a.pdf").is_err());
    assert!(validate_pdf_url("https://127.0.0.1/a.pdf").is_err());
    assert!(validate_pdf_url("https://10.0.0.1/a.pdf").is_err());
    assert!(validate_pdf_url("https://[::1]/a.pdf").is_err());
}

#[test]
fn accepts_public_pdf_urls() {
    assert!(validate_pdf_url("https://arxiv.org/pdf/1706.03762").is_ok());
    assert!(validate_pdf_url("https://example.com/paper.pdf").is_ok());
}

#[test]
fn classifies_public_and_private_ips() {
    for private in [
        "127.0.0.1",
        "10.1.2.3",
        "172.16.0.1",
        "192.168.1.1",
        "169.254.1.1",
        "100.64.0.1",
        "0.0.0.0",
        "224.0.0.1",
        "::1",
        "fc00::1",
        "fe80::1",
    ] {
        assert!(
            !is_public_ip(private.parse().unwrap()),
            "{private} should be private"
        );
    }

    for public in ["1.1.1.1", "8.8.8.8", "2001:4860:4860::8888"] {
        assert!(
            is_public_ip(public.parse().unwrap()),
            "{public} should be public"
        );
    }
}

#[test]
fn detects_pdf_headers_and_html() {
    assert!(looks_like_pdf(b"%PDF-1.7\n..."));
    assert!(looks_like_pdf(
        &[0; 100]
            .iter()
            .copied()
            .chain(*b"%PDF-1.4")
            .collect::<Vec<_>>()
    ));
    assert!(!looks_like_pdf(b"<html><body>login</body></html>"));
    assert!(!looks_like_pdf(b""));
}

#[test]
fn cache_key_is_stable_and_url_specific() {
    let first = cache_key("https://arxiv.org/pdf/1706.03762");
    assert_eq!(first, cache_key("https://arxiv.org/pdf/1706.03762"));
    assert_ne!(first, cache_key("https://arxiv.org/pdf/1512.03385"));
    assert_eq!(first.len(), 16);
}

#[tokio::test]
#[ignore = "downloads a real PDF from arXiv"]
async fn downloads_a_real_arxiv_pdf() {
    let dir = std::env::temp_dir().join(format!("paperflow-pdf-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    let fetcher = PdfFetcher::new(dir.clone()).expect("client should build");
    let bytes = fetcher
        .fetch("https://arxiv.org/pdf/1706.03762")
        .await
        .expect("download should succeed");

    assert!(looks_like_pdf(&bytes));
    assert!(bytes.len() > 1000);

    let _ = std::fs::remove_dir_all(dir);
}
