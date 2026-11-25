// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `Attrs` packed (`u64`) vs an equivalent unpacked struct: hash, equality,
//! and field access. The unpacked form is the 5-field mirror of the packed
//! bit layout (`flags`, `underline`, `underline_color`, `fg`, `bg`).
#![allow(missing_docs, reason = "criterion-generated bench entrypoints")]
#![allow(
    clippy::cast_possible_truncation,
    reason = "bench indices stay below 1024, which fits u16"
)]

use std::hash::{Hash, Hasher};
use std::mem::size_of;

use criterion::{Criterion, black_box, criterion_group, criterion_main};

use vtty::{ColorId, SgrFlags, UnderlineStyle};

use jetfish_protocol::new_hasher;
use jetfish_protocol::{Attrs, Style};

/// The unpacked mirror: same data, named fields (12 bytes vs `Attrs`' 8).
#[derive(Clone, Copy, PartialEq, Eq)]
struct Unpacked {
    fg: ColorId,
    bg: ColorId,
    flags: SgrFlags,
    underline: UnderlineStyle,
    underline_color: Option<ColorId>,
}

impl Hash for Unpacked {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // `SgrFlags` has no `Hash`, so hash its raw bits like the packed form.
        self.fg.hash(state);
        self.bg.hash(state);
        self.flags.bits().hash(state);
        self.underline.hash(state);
        self.underline_color.hash(state);
    }
}

const _: () = {
    assert!(size_of::<Attrs>() == 8, "Attrs must stay 8 bytes");
    assert!(size_of::<Unpacked>() == 12, "Unpacked must be 12 bytes");
};

const N: usize = 1024;

fn packed() -> Vec<Attrs> {
    (0..N)
        .map(|i| {
            Attrs::from_style(Style::new(
                ColorId(i as u16),
                ColorId((i + 1) as u16),
                SgrFlags::from_bits_retain(i as u16),
                UnderlineStyle::Single,
                Some(ColorId((i + 2) as u16)),
            ))
        })
        .collect()
}

fn unpacked() -> Vec<Unpacked> {
    (0..N)
        .map(|i| Unpacked {
            fg: ColorId(i as u16),
            bg: ColorId((i + 1) as u16),
            flags: SgrFlags::from_bits_retain(i as u16),
            underline: UnderlineStyle::Single,
            underline_color: Some(ColorId((i + 2) as u16)),
        })
        .collect()
}

fn hash_packed(attrs: &[Attrs]) -> u64 {
    let mut acc = 0_u64;
    for a in attrs {
        let mut h = new_hasher();
        a.hash(&mut h);
        acc = acc.wrapping_add(h.finish());
    }
    acc
}

fn hash_unpacked(attrs: &[Unpacked]) -> u64 {
    let mut acc = 0_u64;
    for a in attrs {
        let mut h = new_hasher();
        a.hash(&mut h);
        acc = acc.wrapping_add(h.finish());
    }
    acc
}

fn eq_packed(attrs: &[Attrs]) -> usize {
    attrs.windows(2).filter(|w| w[0] == w[1]).count()
}

fn eq_unpacked(attrs: &[Unpacked]) -> usize {
    attrs.windows(2).filter(|w| w[0] == w[1]).count()
}

fn access_packed(attrs: &[Attrs]) -> u64 {
    let mut acc = 0_u64;
    for a in attrs {
        acc = acc
            .wrapping_add(a.fg().0 as u64)
            .wrapping_add(a.bg().0 as u64)
            .wrapping_add(a.flags().bits() as u64)
            .wrapping_add(a.underline_style() as u8 as u64);
    }
    acc
}

fn access_unpacked(attrs: &[Unpacked]) -> u64 {
    let mut acc = 0_u64;
    for a in attrs {
        acc = acc
            .wrapping_add(a.fg.0 as u64)
            .wrapping_add(a.bg.0 as u64)
            .wrapping_add(a.flags.bits() as u64)
            .wrapping_add(a.underline as u8 as u64);
    }
    acc
}

fn bench_attrs(c: &mut Criterion) {
    let packed = packed();
    let unpacked = unpacked();

    let mut group = c.benchmark_group("attrs/hash");
    group.bench_function("packed-u64", |b| {
        b.iter(|| black_box(hash_packed(black_box(&packed))));
    });
    group.bench_function("unpacked-struct", |b| {
        b.iter(|| black_box(hash_unpacked(black_box(&unpacked))));
    });
    group.finish();

    let mut group = c.benchmark_group("attrs/eq");
    group.bench_function("packed-u64", |b| {
        b.iter(|| black_box(eq_packed(black_box(&packed))));
    });
    group.bench_function("unpacked-struct", |b| {
        b.iter(|| black_box(eq_unpacked(black_box(&unpacked))));
    });
    group.finish();

    let mut group = c.benchmark_group("attrs/access");
    group.bench_function("packed-u64", |b| {
        b.iter(|| black_box(access_packed(black_box(&packed))));
    });
    group.bench_function("unpacked-struct", |b| {
        b.iter(|| black_box(access_unpacked(black_box(&unpacked))));
    });
    group.finish();
}

criterion_group!(benches, bench_attrs);
criterion_main!(benches);
