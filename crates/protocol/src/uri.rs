// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! URL-percent decoding and OSC 7 working-directory URIs.

/// Percent-decode a URL-encoded byte string (`%XX` → byte). Invalid escapes
/// (`%` not followed by two hex digits) are kept literally.
#[must_use]
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

/// The filesystem path of an OSC 7 `file://HOST/PATH` payload (drops the
/// scheme/hostname and percent-decodes; `None` for a non-`file` scheme).
#[must_use]
pub fn cwd_path(payload: &str) -> Option<std::path::PathBuf> {
    let rest = payload.strip_prefix("file://")?;
    let path = rest.find('/').map(|i| &rest[i..])?;
    Some(std::path::PathBuf::from(percent_decode(path.as_bytes())))
}

#[cfg(test)]
#[path = "../tests/uri/mod.rs"]
mod tests;
