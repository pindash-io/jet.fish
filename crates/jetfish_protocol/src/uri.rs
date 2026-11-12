// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! URL-percent decoding and OSC 7 working-directory URIs.

/// Percent-decode a URL-encoded byte string (`%XX` → byte). Invalid escapes
/// (`%` not followed by two hex digits) are kept literally.
pub fn percent_decode(bytes: &[u8]) -> String {
    const fn hex(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    }
    let mut out = Vec::with_capacity(bytes.len());
    // Split on `%`: a valid `%XX` prefix decodes; an invalid `%` is literal.
    let mut parts = bytes.split(|&b| b == b'%');
    if let Some(first) = parts.next() {
        out.extend(first);
    }
    for part in parts {
        let decoded = match part {
            [h, l, ..] => match (hex(*h), hex(*l)) {
                (Some(h), Some(l)) => Some(h << 4 | l),
                _ => None,
            },
            _ => None,
        };
        match decoded {
            Some(byte) => {
                out.push(byte);
                out.extend(&part[2..]);
            }
            None => {
                out.push(b'%');
                out.extend(part);
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The filesystem path carried by an OSC 7 `file://HOST/PATH` payload.
/// Drops the `file://` scheme and the hostname authority, then
/// percent-decodes the path (`file:///my%20dir` → `/my dir`). Returns
/// `None` for a non-`file` scheme or a payload without a path.
pub fn cwd_path(payload: &str) -> Option<std::path::PathBuf> {
    let rest = payload.strip_prefix("file://")?;
    let path = rest.find('/').map(|i| &rest[i..])?;
    Some(std::path::PathBuf::from(percent_decode(path.as_bytes())))
}

#[cfg(test)]
mod tests {
    use super::cwd_path;

    #[test]
    fn cwd_path_strips_scheme_and_host() {
        assert_eq!(cwd_path("file://host/path"), Some("/path".into()));
        assert_eq!(
            cwd_path("file://localhost/Users/ein"),
            Some("/Users/ein".into())
        );
        // Empty host: three slashes.
        assert_eq!(cwd_path("file:///home/user"), Some("/home/user".into()));
    }

    #[test]
    fn cwd_path_percent_decodes() {
        assert_eq!(cwd_path("file:///my%20dir"), Some("/my dir".into()));
    }

    #[test]
    fn cwd_path_rejects_non_file_and_pathless() {
        assert_eq!(cwd_path("http://host/path"), None);
        assert_eq!(cwd_path("file://"), None);
        assert_eq!(cwd_path("file://host"), None);
    }
}
