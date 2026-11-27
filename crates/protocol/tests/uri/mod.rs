// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::cwd_path;

#[test]
fn cwd_path_strips_scheme_and_host() {
    assert_eq!(cwd_path("file://host/path"), Some("/path".into()));
    assert_eq!(
        cwd_path("file://localhost/home/alice"),
        Some("/home/alice".into())
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
