// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

use vtty::TermFlags;

use super::*;

#[test]
fn dcs_codec() {
    // DECRQSS (`DCS $ q <setting> ST`).
    let dcs = Dcs::new(SmallVec::new(), 0, vec![b'$'], b'q', b"r".to_vec());
    assert_eq!(dcs.clone().encode().as_ref(), b"\x1bP$qr\x1b\\");
    assert_eq!(
        dcs.decode(),
        Some(Command::Query(Query::Setting(b"r".to_vec())))
    );

    // XTGETTCAP (`DCS + q <hex> ST`).
    let dcs =
        Dcs::new(SmallVec::new(), 0, vec![b'+'], b'q', b"524742".to_vec());
    assert_eq!(dcs.clone().encode().as_ref(), b"\x1bP+q524742\x1b\\");
    assert_eq!(
        dcs.decode(),
        Some(Command::Query(Query::Termcap(b"RGB".to_vec())))
    );

    // Legacy iTerm2 sync begin (`DCS = 1 s`).
    let dcs = Dcs::new(SmallVec::from([1]), 0, vec![b'='], b's', Vec::new());
    assert_eq!(dcs.clone().encode().as_ref(), b"\x1bP=1s\x1b\\");
    assert_eq!(
        dcs.decode(),
        Some(Command::SetMode { mode: TermFlags::SYNC_OUTPUT, on: true })
    );
}
