use tauri::webview::Cookie;

/// Renders cookies read with `cookies_for_url(<url of request_host>)` as a Netscape `cookies.txt`
/// (what yt-dlp and gallery-dl read).
/// Line: `[#HttpOnly_]domain  TRUE|FALSE  path  secure  expiry  name  value`, tab-separated;
/// session cookies get expiry 0.
///
/// `Cookie::domain()` strips the leading `.` WebView2 reports, so domain vs host-only is derived from
/// the request host instead: a cookie returned for that URL whose domain isn't the host itself can only
/// be a domain cookie for a parent (`.instagram.com`, flag TRUE); one equal to the host is written
/// host-only (flag FALSE). The dot matters: gallery-dl checks `cookie.domain == ".instagram.com"`.
pub fn to_netscape(cookies: &[Cookie<'_>], request_host: &str) -> String {
    let mut out = String::from("# Netscape HTTP Cookie File\n");
    for c in cookies {
        let bare = c.domain().map(|d| d.trim_start_matches('.')).unwrap_or(request_host);
        // A tab or line break would corrupt the file (or smuggle in a line); real cookies never have one.
        if [c.name(), c.value(), bare, c.path().unwrap_or("")].iter().any(|s| s.contains(['\t', '\n', '\r'])) {
            continue;
        }
        let host_only = bare.eq_ignore_ascii_case(request_host);
        let expiry = c.expires_datetime().map_or(0, |t| t.unix_timestamp().max(0));
        out.push_str(&format!(
            "{}{}{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            if c.http_only() == Some(true) { "#HttpOnly_" } else { "" },
            if host_only { "" } else { "." },
            bare,
            if host_only { "FALSE" } else { "TRUE" },
            c.path().unwrap_or("/"),
            if c.secure() == Some(true) { "TRUE" } else { "FALSE" },
            expiry,
            c.name(),
            c.value(),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cookie::time::OffsetDateTime;

    const HOST: &str = "www.instagram.com";

    #[test]
    fn netscape_format() {
        let cookies = [
            // `Cookie::domain()` is dot-stripped, as WebView2's `.instagram.com` comes back through wry.
            Cookie::build(("sessionid", "abc%3A1"))
                .domain("instagram.com")
                .path("/")
                .secure(true)
                .http_only(true)
                .expires(OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap())
                .build(),
            Cookie::build(("wd", "1"))
                .domain(HOST)
                .path("/")
                .secure(false)
                .http_only(false)
                .expires(cookie::Expiration::Session)
                .build(),
        ];
        let out = to_netscape(&cookies, HOST);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines,
            [
                "# Netscape HTTP Cookie File",
                "#HttpOnly_.instagram.com\tTRUE\t/\tTRUE\t1800000000\tsessionid\tabc%3A1",
                "www.instagram.com\tFALSE\t/\tFALSE\t0\twd\t1",
            ]
        );
        assert!(out.ends_with('\n'));
    }

    #[test]
    fn netscape_dot_comes_from_request_host() {
        let cookies = [
            Cookie::build(("a", "1")).domain(".instagram.com").build(), // dotted input: same result
            Cookie::build(("b", "2")).domain("WWW.Instagram.com").build(), // host compare ignores case
            Cookie::new("c", "3"),                                          // no domain: host-only
        ];
        assert_eq!(
            to_netscape(&cookies, HOST),
            "# Netscape HTTP Cookie File\n\
             .instagram.com\tTRUE\t/\tFALSE\t0\ta\t1\n\
             WWW.Instagram.com\tFALSE\t/\tFALSE\t0\tb\t2\n\
             www.instagram.com\tFALSE\t/\tFALSE\t0\tc\t3\n"
        );
    }

    #[test]
    fn netscape_skips_cookies_that_would_break_lines() {
        let cookies = [
            Cookie::build(("bad", "a\nb")).domain("x.com").path("/").build(),
            Cookie::build(("ok", "v")).domain("x.com").build(),
        ];
        let out = to_netscape(&cookies, "x.com");
        assert_eq!(out, "# Netscape HTTP Cookie File\nx.com\tFALSE\t/\tFALSE\t0\tok\tv\n");
    }
}
