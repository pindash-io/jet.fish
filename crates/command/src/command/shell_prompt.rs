// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! OSC 133 shell-integration marker (`A`/`B`/`C`/`D`).

use compact_str::{CompactString, format_compact};
use smallvec::SmallVec;

use vtty::Event;

use super::codec::{EncodeSeq, Encoded};

/// A parsed OSC 133 shell-integration marker: the action byte (`A`/`B`/`C`/`D`)
/// plus the optional `aid=` prompt id and positional exit code (iTerm2's
/// `OSC 133;D;<code>` shape).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellPrompt {
    /// The action byte (`A`, `B`, `C`, `D`, …).
    pub action: u8,
    /// The prompt/command id (`aid=`), if any.
    pub aid: Option<CompactString>,
    /// The exit code (`D;<code>` positional), if any.
    pub exit_code: Option<i32>,
    /// The URL-decoded command line (`C;cmdline_url=<url>`), if any.
    pub cmdline: Option<String>,
}

impl EncodeSeq for ShellPrompt {
    fn encode(self) -> Encoded {
        let mut params = SmallVec::<Vec<u8>, 8>::new();
        params.push(b"133".to_vec());
        params.push(vec![self.action]);
        if let Some(aid) = &self.aid {
            params.push(format_compact!("aid={aid}").into());
        }
        if let Some(cmdline) = &self.cmdline {
            params.push(format!("cmdline_url={cmdline}").into());
        }
        if let Some(code) = self.exit_code {
            params.push(code.to_string().into());
        }
        let mut out = Encoded::new();
        Event::OscDispatch { params, bell_terminated: false }
            .write_bytes(&mut out);
        out
    }
}
