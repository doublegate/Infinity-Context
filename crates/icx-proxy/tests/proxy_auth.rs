//! v1.1.0 proxy authentication (ICX-AUD-012).

use icx_adapters::mmu::InfinityContextMmu;
use icx_proxy::WireProxyTransformer;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

fn request(addr: &str, raw: &str) -> String {
    let mut s = TcpStream::connect(addr).unwrap();
    s.write_all(raw.as_bytes()).unwrap();
    let mut out = String::new();
    let _ = s.read_to_string(&mut out);
    out
}

#[test]
fn every_route_requires_the_install_token() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let server = std::thread::spawn(move || {
        let mut mmu = InfinityContextMmu::default();
        WireProxyTransformer::serve_http_listener_authenticated(
            &listener,
            None,
            Some("none"),
            None,
            Some(6),
            Some("secret-token"),
            &mut mmu,
        )
    });
    let body = "data: ⟪!icx:deref(#icx:0000000000000001)⟫";
    let cases = [
        ("GET /health HTTP/1.1\r\n\r\n".to_owned(), "401"),
        (
            "GET /t/wrong-token/health HTTP/1.1\r\n\r\n".to_owned(),
            "401",
        ),
        (
            format!(
                "POST /sse-expand HTTP/1.1\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            ),
            "401",
        ),
        (
            "GET /t/secret-token/health HTTP/1.1\r\n\r\n".to_owned(),
            "200",
        ),
        (
            "GET /health HTTP/1.1\r\nx-icx-token: secret-token\r\n\r\n".to_owned(),
            "200",
        ),
        (
            "POST /v1/messages HTTP/1.1\r\nX-ICX-Token: nope\r\nContent-Length: 2\r\n\r\n{}"
                .to_owned(),
            "401",
        ),
    ];
    for (raw, want) in cases {
        let resp = request(&addr, &raw);
        let status = resp.split_whitespace().nth(1).unwrap_or("");
        assert_eq!(status, want, "request {raw:?} -> {resp}");
    }
    assert_eq!(server.join().unwrap().unwrap(), 6);
}

#[cfg(unix)]
#[test]
fn curl_header_dir_must_be_private_and_ours_else_a_per_user_temp_dir_is_used() {
    // PR #20 review (Antigravity): a group/world-accessible runtime dir is not used, and the
    // fallback is a private `<tmp>/icx-<uid>` rather than the shared temp dir itself.
    use std::os::unix::fs::PermissionsExt;
    let base = std::env::temp_dir().join(format!("icx_curl_dir_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (xdg, tmp) = (base.join("xdg"), base.join("tmp"));
    std::fs::create_dir_all(xdg.join("icx")).unwrap();
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::set_permissions(xdg.join("icx"), std::fs::Permissions::from_mode(0o755)).unwrap();
    // Our own widened runtime dir is repaired to 0700 and used.
    let picked = WireProxyTransformer::curl_scratch_dir_in(Some(xdg.clone()), &tmp);
    assert_eq!(picked, xdg.join("icx"));
    let mode = std::fs::metadata(&picked).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o700, "widened runtime dir was not repaired");
    // A symlinked runtime dir is never used; the fallback is a private per-user temp dir.
    std::fs::remove_dir_all(xdg.join("icx")).unwrap();
    std::os::unix::fs::symlink(&base, xdg.join("icx")).unwrap();
    let picked = WireProxyTransformer::curl_scratch_dir_in(Some(xdg.clone()), &tmp);
    assert_ne!(picked, tmp, "fell back to the shared temp dir");
    assert!(picked.starts_with(&tmp), "{}", picked.display());
    let mode = std::fs::metadata(&picked).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o700);
    let _ = std::fs::remove_dir_all(&base);
}
