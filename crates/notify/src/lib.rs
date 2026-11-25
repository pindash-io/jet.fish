// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Desktop-notification parsing (OSC 99 / OSC 777). Parse-only: the host owns
//! displaying a [`Notification`].

/// A parsed desktop-notification request.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Notification {
    /// The title.
    pub title: String,
    /// The body.
    pub body: String,
}

/// Parse an OSC 99 payload; metadata is colon-separated `key=value`. Returns
/// `None` for the `p=?` capability query.
#[must_use]
pub fn parse_osc99(params: &[&[u8]]) -> Option<Notification> {
    let prop = parse_metadata(params.get(2).copied().unwrap_or(b""));
    let prop = prop.as_deref();
    if prop == Some("?") {
        return None;
    }
    let text = lossy(params, 1);
    let mut notification = Notification::default();
    match prop {
        Some("body") => notification.body = text,
        _ => notification.title = text,
    }
    Some(notification)
}

/// Parse an OSC 777 (urxvt `notify`) payload
/// (`ESC ] 777 ; notify ; title ; body ST`).
#[must_use]
pub fn parse_osc777(params: &[&[u8]]) -> Option<Notification> {
    if params.get(1).copied() != Some(b"notify") {
        return None;
    }
    Some(Notification { title: lossy(params, 2), body: lossy(params, 3) })
}

/// UTF-8-lossy decode of the byte slice at `index` (`""` when absent).
#[inline]
fn lossy(params: &[&[u8]], index: usize) -> String {
    String::from_utf8_lossy(params.get(index).copied().unwrap_or(b""))
        .into_owned()
}

/// Extract the `p=` property from an OSC 99 metadata string.
fn parse_metadata(metadata: &[u8]) -> Option<String> {
    let s = String::from_utf8_lossy(metadata);
    for field in s.split(':') {
        if let Some(("p", value)) = field.split_once('=') {
            return Some(value.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osc99_title_and_body() {
        let n = parse_osc99(&[b"99", b"Crush", b"i=crush-1:p=title"]).unwrap();
        assert_eq!(n.title, "Crush");
        assert_eq!(n.body, "");

        let n = parse_osc99(&[b"99", b"hello", b"i=crush-1:p=body"]).unwrap();
        assert_eq!(n.title, "");
        assert_eq!(n.body, "hello");
    }

    #[test]
    fn osc99_query_is_not_a_notification() {
        // `p=?` is the capability probe, not a notify.
        assert_eq!(parse_osc99(&[b"99", b"", b"i=id:p=?"]), None);
    }

    #[test]
    fn osc777_notify() {
        let n =
            parse_osc777(&[b"777", b"notify", b"Crush", b"hello"]).unwrap();
        assert_eq!((n.title.as_str(), n.body.as_str()), ("Crush", "hello"));
        assert_eq!(parse_osc777(&[b"777", b"other"]), None);
    }
}
