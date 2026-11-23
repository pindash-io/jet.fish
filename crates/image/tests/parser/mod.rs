// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use super::*;

/// A user-defined protocol that just echoes a fixed 1×1 image.
#[derive(Default)]
struct CustomParser;

impl ImageParser for CustomParser {
    fn parse(
        &mut self,
        payload: &[u8],
        final_byte: u8,
        ctx: ImageCtx,
    ) -> ParseOutcome {
        if final_byte != b'z' || payload != b"ping" {
            return ParseOutcome::Skip;
        }
        ParseOutcome::Handled(ParsedImage {
            image: Some(Image {
                kind: ImageKind::Kitty,
                id: 0,
                version: 0,
                anchor: USizeVec2::new(
                    ctx.cursor_col.saturating_sub(1),
                    ctx.screen_start,
                ),
                cell_size: UVec2::ZERO,
                size: UVec2::ONE,
                cell_offset: UVec2::ZERO,
                z_index: 0,
                aspect: UVec2::ONE,
                pixels: Arc::from(vec![255, 0, 0, 255]),
            }),
            reply: None,
            advance_cursor: false,
            clear_image: false,
            delete_id: None,
            delete_image_id: None,
        })
    }
}

#[test]
fn registry_consults_custom_parser_in_order() {
    let mut parsers = ImageParsers::default_parsers();
    parsers.push(CustomParser);
    let ctx = ImageCtx { cursor_col: 1, cursor_row: 1, screen_start: 0 };
    // Built-ins skip `z`; the custom parser handles it.
    let mut hit = false;
    for parser in parsers.iter_mut() {
        match parser.parse(b"ping", b'z', ctx) {
            ParseOutcome::Skip => continue,
            ParseOutcome::Consumed => unreachable!(),
            ParseOutcome::Handled(p) => {
                let img = p.image.expect("placed");
                assert_eq!(img.size, UVec2::ONE);
                assert_eq!(&img.pixels[..], &[255, 0, 0, 255]);
                hit = true;
                break;
            }
        }
    }
    assert!(hit, "custom parser was consulted");
}
