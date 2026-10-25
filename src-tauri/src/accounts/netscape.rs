use tauri::webview::Cookie;

/// Renders cookies as a Netscape `cookies.txt` (what yt-dlp and gallery-dl read).
/// Line: `[#HttpOnly_]domain  TRUE|FALSE  path  secure  expiry  name  value`, tab-separated;
/// the flag is TRUE exactly when the domain starts with `.`, session cookies get expiry 0.
pub fn to_netscape(cookies: &[Cookie<'_>]) -> String {
    let mut out = String::from("# Netscape HTTP Cookie File\n");
    for c in cookies {
        // A tab or line break would corrupt the file (or smuggle in a line); real cookies never have one.
        if [c.name(), c.value(), c.domain().unwrap_or(""), c.path().unwrap_or("")]
            .iter()
            .any(|s| s.contains(['\t', '\n', '\r']))
        {
            continue;
        }
        let domain = raw_domain(c);
        let expiry = c.expires_datetime().map_or(0, |t| t.unix_timestamp().max(0));
        out.push_str(&format!(
            "{}{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            if c.http_only() == Some(true) { "#HttpOnly_" } else { "" },
            domain,
            if domain.starts_with('.') { "TRUE" } else { "FALSE" },
            c.path().unwrap_or("/"),
            if c.secure() == Some(true) { "TRUE" } else { "FALSE" },
            expiry,
            c.name(),
            c.value(),
        ));
    }
    out
}

/// The domain as WebView2 reported it, leading `.` included (`.instagram.com` = domain cookie,
/// `www.instagram.com` = host-only). `Cookie::domain()` strips that dot and the crate has no raw
/// getter, so it is read back from the derived `Debug` of a builder-made cookie
/// (`domain: Some(Concrete(".x"))`, which is how wry builds them). `netscape_format` guards this.
/// The dot matters: gallery-dl's login check compares `cookie.domain == ".instagram.com"` exactly.
fn raw_domain(c: &Cookie<'_>) -> String {
    let Some(d) = c.domain() else { return String::new() };
    let dotted = format!(".{d}");
    if format!("{c:?}").contains(&format!("domain: Some(Concrete({dotted:?}))")) { dotted } else { d.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cookie::time::OffsetDateTime;

    #[test]
    fn netscape_format() {
        let cookies = [
            Cookie::build(("sessionid", "abc%3A1"))
                .domain(".instagram.com")
                .path("/")
                .secure(true)
                .http_only(true)
                .expires(OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap())
                .build(),
            Cookie::build(("wd", "1"))
                .domain("www.instagram.com")
                .path("/")
                .secure(false)
                .http_only(false)
                .expires(cookie::Expiration::Session)
                .build(),
        ];
        let out = to_netscape(&cookies);
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
    fn netscape_skips_cookies_that_would_break_lines() {
        let cookies = [
            Cookie::build(("bad", "a\nb")).domain("x.com").path("/").build(),
            Cookie::build(("ok", "v")).domain("x.com").build(),
        ];
        let out = to_netscape(&cookies);
        assert_eq!(out, "# Netscape HTTP Cookie File\nx.com\tFALSE\t/\tFALSE\t0\tok\tv\n");
    }
}
