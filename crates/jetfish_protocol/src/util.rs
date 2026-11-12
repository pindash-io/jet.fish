// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Byte helpers: hex encoding and a blocking write-to-fd loop.

/// Decode a hex-encoded byte string (DECRQSS/XTGETTCAP payloads). Invalid
/// input decodes to empty.
pub fn hex_decode(bytes: &[u8]) -> Vec<u8> {
    vtty::const_hex::decode(bytes).unwrap_or_default()
}

/// Encode `bytes` as lowercase hex pairs.
pub fn hex_encode(bytes: &[u8]) -> String {
    vtty::const_hex::encode(bytes)
}

/// Write `data` to `fd`, retrying until every byte is written.
///
/// rustix's `write` already retries `EINTR`; this loops on partial writes
/// and yields on a full (non-blocking) pipe.
pub fn write_fd_all(fd: std::os::fd::BorrowedFd<'_>, mut data: &[u8]) {
    while !data.is_empty() {
        match rustix::io::write(fd, data) {
            Ok(n) => data = &data[n..],
            Err(rustix::io::Errno::AGAIN) => {
                std::thread::yield_now();
                continue;
            }
            Err(_) => break,
        }
    }
}
