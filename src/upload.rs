use miette::{IntoDiagnostic, Result, WrapErr, miette};
use reqwest::blocking::multipart::Form;
use reqwest::header::{CONNECTION, HeaderMap, HeaderValue, USER_AGENT};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub fn upload_gsp_file(gsp_path: &Path, upload_url: &str) -> Result<String> {
    let upload_path = resolve_upload_path(gsp_path)?;
    let form = Form::new()
        .file("file", &upload_path)
        .into_diagnostic()
        .wrap_err_with(|| {
            format!(
                "failed to prepare multipart upload for {}",
                upload_path.display()
            )
        })?;

    let response = build_upload_client()?
        .post(upload_url)
        .multipart(form)
        .send()
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to upload {}", upload_path.display()))?;
    let status = response.status();
    let body = response.text().into_diagnostic().wrap_err_with(|| {
        format!(
            "failed to read upload response for {}",
            upload_path.display()
        )
    })?;

    if !status.is_success() {
        let detail = body.trim();
        return Err(miette!(
            "failed to upload {} to {}: HTTP {}{}",
            upload_path.display(),
            upload_url,
            status,
            if detail.is_empty() {
                String::new()
            } else {
                format!(" - {detail}")
            }
        ));
    }

    Ok(body.trim().to_string())
}

fn resolve_upload_path(gsp_path: &Path) -> Result<PathBuf> {
    gsp_path.canonicalize().into_diagnostic().wrap_err_with(|| {
        format!(
            "failed to resolve full upload path for {}",
            gsp_path.display()
        )
    })
}

fn build_upload_client() -> Result<reqwest::blocking::Client> {
    let mut default_headers = HeaderMap::new();
    default_headers.insert(USER_AGENT, HeaderValue::from_static("curl/8.0.0"));
    default_headers.insert(CONNECTION, HeaderValue::from_static("close"));

    reqwest::blocking::Client::builder()
        .default_headers(default_headers)
        .http1_only()
        .timeout(Duration::from_secs(30))
        .build()
        .into_diagnostic()
        .wrap_err("failed to build upload client")
}

#[cfg(test)]
mod tests {
    use super::{resolve_upload_path, upload_gsp_file};
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn uploads_source_gsp_as_multipart_form() {
        let temp_root = unique_test_dir("upload-source-gsp");
        fs::create_dir_all(&temp_root).expect("temporary directory should be creatable");
        let gsp_path = temp_root.join("sample.gsp");
        let gsp_body = b"sample gsp body";
        fs::write(&gsp_path, gsp_body).expect("fixture gsp should be writable");

        let (upload_url, server) = start_test_server(200, "ok");

        let response = upload_gsp_file(&gsp_path, &upload_url).expect("upload should succeed");
        assert_eq!(response, "ok");

        let request = server.join().expect("server should finish");
        assert!(
            request.starts_with("POST /upload.php HTTP/1.1\r\n"),
            "expected multipart POST request, got: {request}"
        );
        assert!(
            request
                .contains("Content-Disposition: form-data; name=\"file\"; filename=\"sample.gsp\""),
            "expected uploaded filename in multipart body, got: {request}"
        );
        assert!(
            request.contains("sample gsp body"),
            "expected raw gsp body in multipart upload, got: {request}"
        );

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn upload_reports_http_failures_with_context() {
        let temp_root = unique_test_dir("upload-http-failure");
        fs::create_dir_all(&temp_root).expect("temporary directory should be creatable");
        let gsp_path = temp_root.join("sample.gsp");
        fs::write(&gsp_path, b"sample gsp body").expect("fixture gsp should be writable");

        let (upload_url, server) = start_test_server(500, "upload failed");
        let error = upload_gsp_file(&gsp_path, &upload_url).expect_err("upload should fail");

        assert!(error.to_string().contains("failed to upload"));
        assert!(
            error
                .to_string()
                .contains("HTTP 500 Internal Server Error - upload failed")
        );

        let _ = server.join();
        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn upload_failure_reports_resolved_full_path() {
        let temp_root = unique_test_dir("upload-full-path-error");
        let nested_root = temp_root.join("nested");
        fs::create_dir_all(&nested_root).expect("temporary directory should be creatable");
        let gsp_path = temp_root.join("sample.gsp");
        fs::write(&gsp_path, b"sample gsp body").expect("fixture gsp should be writable");

        let non_canonical = nested_root.join("..").join("sample.gsp");
        let (upload_url, server) = start_test_server(500, "upload failed");
        let error = upload_gsp_file(&non_canonical, &upload_url).expect_err("upload should fail");

        assert!(error.to_string().contains(&gsp_path.display().to_string()));

        let _ = server.join();
        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn resolves_upload_path_to_full_canonical_path() {
        let temp_root = unique_test_dir("upload-canonical-path");
        let nested_root = temp_root.join("nested");
        fs::create_dir_all(&nested_root).expect("temporary directory should be creatable");
        let gsp_path = temp_root.join("sample.gsp");
        fs::write(&gsp_path, b"sample gsp body").expect("fixture gsp should be writable");

        let non_canonical = nested_root.join("..").join("sample.gsp");
        let resolved = resolve_upload_path(&non_canonical).expect("path should canonicalize");
        assert_eq!(resolved, gsp_path);

        let _ = fs::remove_dir_all(&temp_root);
    }

    fn start_test_server(
        status_code: u16,
        response_body: &'static str,
    ) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        let address = listener.local_addr().expect("listener address");
        let reason = match status_code {
            200 => "OK",
            500 => "Internal Server Error",
            _ => "Test Status",
        };
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("server should accept upload");
            let request = read_http_request(&mut stream);

            let response = format!(
                "HTTP/1.1 {status_code} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream
                .write_all(response.as_bytes())
                .expect("response should be writable");

            request
        });

        (format!("http://{address}/upload.php"), server)
    }

    fn unique_test_dir(prefix: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();
        std::env::temp_dir().join(format!("gsp-rs-{prefix}-{unique}"))
    }

    fn read_http_request(stream: &mut std::net::TcpStream) -> String {
        let mut buffer = Vec::new();
        let mut header_end = None;

        while header_end.is_none() {
            let mut chunk = [0u8; 1024];
            let bytes_read = stream.read(&mut chunk).expect("request should be readable");
            assert!(bytes_read > 0, "expected http request bytes");
            buffer.extend_from_slice(&chunk[..bytes_read]);
            header_end = find_header_end(&buffer);
        }

        let header_end = header_end.expect("header boundary should exist");
        let headers = String::from_utf8_lossy(&buffer[..header_end]).into_owned();
        let lowercase_headers = headers.to_ascii_lowercase();

        if let Some(content_length) = headers.lines().find_map(|line| {
            line.strip_prefix("Content-Length:")
                .or_else(|| line.strip_prefix("content-length:"))
                .map(|value| value.trim().parse::<usize>().expect("content length"))
        }) {
            let total_length = header_end + 4 + content_length;

            while buffer.len() < total_length {
                let mut chunk = [0u8; 1024];
                let bytes_read = stream.read(&mut chunk).expect("body should be readable");
                assert!(bytes_read > 0, "expected more multipart body bytes");
                buffer.extend_from_slice(&chunk[..bytes_read]);
            }

            return String::from_utf8_lossy(&buffer[..total_length]).into_owned();
        }

        assert!(
            lowercase_headers.contains("transfer-encoding: chunked"),
            "expected content-length or chunked transfer encoding, got: {headers}"
        );

        while !buffer.ends_with(b"\r\n0\r\n\r\n") {
            let mut chunk = [0u8; 1024];
            let bytes_read = stream
                .read(&mut chunk)
                .expect("chunked body should be readable");
            assert!(bytes_read > 0, "expected more chunked body bytes");
            buffer.extend_from_slice(&chunk[..bytes_read]);
        }

        String::from_utf8_lossy(&buffer).into_owned()
    }

    fn find_header_end(buffer: &[u8]) -> Option<usize> {
        buffer.windows(4).position(|window| window == b"\r\n\r\n")
    }
}
