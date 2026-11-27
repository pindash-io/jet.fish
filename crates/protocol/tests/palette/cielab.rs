// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

fn assert_close(got: u32, want: u32) {
    for shift in (8..32).step_by(8) {
        let d = (((got >> shift) & 0xff) as i32
            - ((want >> shift) & 0xff) as i32)
            .abs();
        assert!(d <= 2, "got {got:06x} want {want:06x}");
    }
}

#[test]
fn lab_roundtrip_is_accurate() {
    for c in [
        0x000000FF, 0xFFFFFFFF, 0x100F0FFF, 0xFFFCF0FF, 0xAF3029FF,
        0x205EA6FF, 0x879A39FF, 0x3AA99FFF,
    ] {
        assert_close(lab_to_rgb(rgb_to_lab(c)), c);
    }
}

#[test]
fn cielab_is_a_color_space() {
    // The crate machinery should round-trip through the trait.
    let lab = OpaqueColor::<Srgb>::new([1.0, 0.0, 0.0]).convert::<Cielab>();
    let back = lab.convert::<Srgb>();
    assert!(back.difference(OpaqueColor::<Srgb>::new([1.0, 0.0, 0.0])) < 1e-3);
}
