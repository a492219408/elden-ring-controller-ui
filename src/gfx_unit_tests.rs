// SPDX-License-Identifier: GPL-3.0-only
//! 不依赖游戏资源的解析器与完整性门禁测试，公开 CI 始终执行。
use super::*;

#[test]
fn every_layout_rejects_foreign_content_without_modifying_it() {
    for kind in [Movie::Options, Movie::KeyConfig] {
        for layout in [
            ControllerLayout::Original,
            ControllerLayout::XboxSeries,
            ControllerLayout::DualShock4,
            ControllerLayout::DualSense,
        ] {
            let bytes = vec![0xA5; kind.length()];
            let saved = bytes.clone();
            assert!(patch(kind, &bytes, layout).is_err());
            assert_eq!(bytes, saved);
        }
    }
}

#[test]
fn malformed_movie_headers_and_tag_boundaries_fail_closed() {
    for length in 0..22 {
        assert!(movie(&vec![0; length]).is_err());
    }
    for bytes in [
        &b""[..],
        &[255],
        &[255, 255],
        &[255, 255, 255, 255, 255, 255],
        &[0, 0, 1],
    ] {
        assert!(tags(bytes).is_err());
    }
    let mut data = encode(10, &[1, 2, 3]);
    assert!(tags(&data).is_err());
    data.extend_from_slice(&[0, 0]);
    let parsed = tags(&data).unwrap();
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].body, &[1, 2, 3]);
    data.push(0);
    assert!(tags(&data).is_err());
}

#[test]
fn bitfields_and_unterminated_names_are_bounded() {
    let mut bits = Bits {
        bytes: &[0b1010_0101],
        bit: 0,
    };
    assert_eq!(bits.take(4), Ok(10));
    assert_eq!(bits.take(4), Ok(5));
    assert!(bits.take(1).is_err());
    assert!(bits.take(33).is_err());
    assert!(string_end(b"Win64", 0).is_err());
    assert_eq!(string_end(b"Win64\0", 0), Ok(5));
    for count in 0..18 {
        assert!(
            placement(Tag {
                code: 26,
                raw: &[],
                body: &vec![0xff; count]
            })
            .is_err()
        );
    }
}

#[test]
fn duplicate_or_missing_sprite_identifiers_are_rejected() {
    let raw = encode(39, &[7, 0, 1, 0, 0, 0]);
    let mut bytes = raw.clone();
    bytes.extend_from_slice(&raw);
    bytes.extend_from_slice(&[0, 0]);
    let top = tags(&bytes).unwrap();
    assert!(sprite(&top, 7).is_err());
    assert!(sprite(&top, 8).is_err());
}

#[test]
fn source_gates_require_both_exact_length_and_full_digest() {
    for kind in [Movie::Options, Movie::KeyConfig] {
        assert!(!kind.accepts(&vec![0; kind.length() - 1], &kind.hash()));
        assert!(!kind.accepts(&vec![0; kind.length() + 1], &kind.hash()));
        assert!(!kind.accepts(&vec![0; kind.length()], &[0; 32]));
    }
}
