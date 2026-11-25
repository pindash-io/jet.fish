// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

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
    let n = parse_osc777(&[b"777", b"notify", b"Crush", b"hello"]).unwrap();
    assert_eq!((n.title.as_str(), n.body.as_str()), ("Crush", "hello"));
    assert_eq!(parse_osc777(&[b"777", b"other"]), None);
}
