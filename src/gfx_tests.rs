use super::*;
const LAYOUTS: [ControllerLayout; 4] = [
    ControllerLayout::Original,
    ControllerLayout::XboxSeries,
    ControllerLayout::DualShock4,
    ControllerLayout::DualSense,
];
fn changed_only(original: &[u8], patched: &[u8], ids: &[u16], added_images: usize) {
    let (a_header, a) = movie(original).unwrap();
    let (b_header, b) = movie(patched).unwrap();
    assert_eq!(&a_header[..4], &b_header[..4]);
    assert_eq!(&a_header[8..], &b_header[8..]);
    let retained: Vec<_> = b
        .iter()
        .filter(|tag| !(tag.code == 1009 && word(tag.body, 0).unwrap() >= 512))
        .collect();
    assert_eq!(b.len() - retained.len(), added_images);
    assert_eq!(a.len(), retained.len());
    let mut changed = Vec::new();
    for (before, after) in a.iter().zip(retained) {
        if before.raw != after.raw {
            assert_eq!(before.code, 39);
            assert_eq!(after.code, 39);
            let id = word(before.body, 0).unwrap();
            assert_eq!(word(after.body, 0).unwrap(), id);
            assert!(ids.contains(&id), "unexpected changed sprite {id}");
            changed.push(id);
        }
    }
    assert_eq!(changed, ids);
}
#[test]
fn source_fixtures_match_all_three_exact_gates() {
    for (kind, bytes) in [
        (Movie::Options, OFFICIAL_OPTIONS_GFX),
        (Movie::KeyConfig, OFFICIAL_KEY_GFX),
        (Movie::CommonOptions, OFFICIAL_COMMON_GFX),
    ] {
        assert!(kind.accepts(bytes, &sha256::digest(bytes)));
        movie(bytes).unwrap();
    }
}

#[test]
fn reused_pc_label_wrapper_and_parent_matrix_match_every_official_overview_preset() {
    let (_, pc) = movie(OFFICIAL_OPTIONS_GFX).unwrap();
    let (_, common) = movie(OFFICIAL_COMMON_GFX).unwrap();
    assert_eq!(
        refs(sprite(&common, 163).unwrap(), 163, &[(93, 89), (162, 162)]).unwrap(),
        sprite(&pc, 163).unwrap().raw
    );
    let parent = named(sprite(&pc, 165).unwrap(), b"Win64").unwrap();
    let matrix = &parent.body[placement(parent).unwrap().matrix.unwrap()];
    for name in [b"PS4".as_slice(), b"PS5", b"Scarlette", b"XboxOne"] {
        let preset = named(sprite(&common, 171).unwrap(), name).unwrap();
        assert_eq!(
            &preset.body[placement(preset).unwrap().matrix.unwrap()],
            matrix
        );
    }
}
#[test]
fn four_layouts_preserve_pc_menus_scripts_keyboard_and_display_preference_code() {
    for layout in LAYOUTS {
        let options = patch(Movie::Options, OFFICIAL_OPTIONS_GFX, layout).unwrap();
        let keys = patch(Movie::KeyConfig, OFFICIAL_KEY_GFX, layout).unwrap();
        if layout == ControllerLayout::Original {
            assert_eq!(options, OFFICIAL_OPTIONS_GFX);
            assert_eq!(keys, OFFICIAL_KEY_GFX);
        } else {
            changed_only(
                OFFICIAL_OPTIONS_GFX,
                &options,
                &[161, 164],
                if layout == ControllerLayout::DualShock4 {
                    0
                } else {
                    2
                },
            );
            // Every animation definition, keyboard subtree, ActionScript block, and root frame is
            // byte-identical: only the two controller branch-container definitions change.
            changed_only(OFFICIAL_KEY_GFX, &keys, &[110, 161], 0);
        }
        println!(
            "{} options={} {} keyconfig={} {}",
            layout.as_str(),
            options.len(),
            sha256::to_hex(&sha256::digest(&options)),
            keys.len(),
            sha256::to_hex(&sha256::digest(&keys))
        );
    }
}
#[test]
fn native_button_configuration_selects_the_whole_matching_preset_and_keeps_hide_targets_unique() {
    let (_, source) = movie(OFFICIAL_KEY_GFX).unwrap();
    for (layout, name, help, select) in [
        (ControllerLayout::XboxSeries, &b"XboxSeries"[..], 89, 131),
        (ControllerLayout::DualShock4, &b"PS4"[..], 99, 141),
        (ControllerLayout::DualSense, &b"PS5"[..], 109, 160),
    ] {
        let output = patch(Movie::KeyConfig, OFFICIAL_KEY_GFX, layout).unwrap();
        let (_, top) = movie(&output).unwrap();
        for (id, chosen, old) in [(110, help, 85), (161, select, 127)] {
            let node = sprite(&top, id).unwrap();
            let active = named(node, b"Win64").unwrap();
            assert_eq!(
                word(active.body, placement(active).unwrap().character.unwrap()).unwrap(),
                chosen
            );
            let hidden = named(node, name).unwrap();
            assert_eq!(
                word(hidden.body, placement(hidden).unwrap().character.unwrap()).unwrap(),
                old
            );
            // Inverting the aliases reconstructs the original byte-for-byte, not merely its image.
            assert_eq!(
                swap_names(node, name).unwrap(),
                sprite(&source, id).unwrap().raw
            );
            for branch in [
                b"XboxOne".as_slice(),
                b"Win64",
                b"XboxSeries",
                b"PS4",
                b"PS5",
            ] {
                named(node, branch).unwrap();
            }
        }
    }
}
#[test]
fn overview_recipe_reproduces_exact_official_controller_lines_and_label_transforms() {
    let (_, common) = movie(OFFICIAL_COMMON_GFX).unwrap();
    for (layout, root, line, big_id, line_id) in [
        (ControllerLayout::XboxSeries, 164, 161, 74, 73),
        (ControllerLayout::DualShock4, 170, 169, 68, 67),
        (ControllerLayout::DualSense, 168, 167, 70, 69),
    ] {
        let output = patch(Movie::Options, OFFICIAL_OPTIONS_GFX, layout).unwrap();
        let (_, top) = movie(&output).unwrap();
        let big = if layout == ControllerLayout::DualShock4 {
            4
        } else {
            512
        };
        let lines = if layout == ControllerLayout::DualShock4 {
            6
        } else {
            513
        };
        assert_eq!(
            image_name(image(&top, big).unwrap()).unwrap(),
            image_name(image(&common, big_id).unwrap()).unwrap()
        );
        assert_eq!(
            image(&top, big).unwrap().body[2..],
            image(&common, big_id).unwrap().body[2..]
        );
        assert_eq!(
            image(&top, lines).unwrap().body[2..],
            image(&common, line_id).unwrap().body[2..]
        );
        // Translate the IDs back: the full controller and line subtrees must equal the donor.
        assert_eq!(
            refs(
                sprite(&top, 164).unwrap(),
                root,
                &[(big, big_id), (161, line), (163, 163)]
            )
            .unwrap(),
            sprite(&common, root).unwrap().raw
        );
        assert_eq!(
            refs(sprite(&top, 161).unwrap(), line, &[(lines, line_id)]).unwrap(),
            sprite(&common, line).unwrap().raw
        );
    }
}
#[test]
fn both_tab_timelines_stay_byte_identical_and_native_frame_selection_resolves_the_right_images() {
    let (_, original) = movie(OFFICIAL_OPTIONS_GFX).unwrap();
    for (layout, suffix) in [
        (ControllerLayout::Original, ""),
        (ControllerLayout::XboxSeries, "_XBoxSeries"),
        (ControllerLayout::DualShock4, "_PS4"),
        (ControllerLayout::DualSense, "_PS5"),
    ] {
        let output = patch(Movie::Options, OFFICIAL_OPTIONS_GFX, layout).unwrap();
        let (_, top) = movie(&output).unwrap();
        let frames = crate::native_tabs::frames(layout);
        for id in [174, 183] {
            let source = sprite(&original, id).unwrap();
            let selected = sprite(&top, id).unwrap();
            assert_eq!(source.raw, selected.raw);
            for (frame, name) in frames
                .into_iter()
                .zip(["MENU_Tab_Option", "MENU_Tab_Manual"])
            {
                let frame = tab_frame(selected, usize::from(frame)).unwrap();
                let mut cid =
                    word(frame.body, placement(frame).unwrap().character.unwrap()).unwrap();
                if let Ok(nested) = sprite(&top, cid) {
                    let inner = tab_frame(nested, 1).unwrap();
                    cid = word(inner.body, placement(inner).unwrap().character.unwrap()).unwrap();
                }
                assert_eq!(
                    image_name(image(&top, cid).unwrap()).unwrap(),
                    format!("{name}{suffix}").as_bytes()
                );
            }
        }
    }
}
#[test]
fn foreign_resources_and_malformed_tags_fail_closed_without_mutating_inputs() {
    for bits in 17..32 {
        let mut short = vec![0; 22];
        short[..4].copy_from_slice(b"GFX\x0b");
        short[4..8].copy_from_slice(&22u32.to_le_bytes());
        short[8] = bits << 3;
        assert!(movie(&short).is_err());
    }
    for (kind, data) in [
        (Movie::Options, OFFICIAL_OPTIONS_GFX),
        (Movie::KeyConfig, OFFICIAL_KEY_GFX),
    ] {
        for at in [0, 4, 9, 100, data.len() - 1] {
            let mut foreign = data.to_vec();
            foreign[at] ^= 1;
            let saved = foreign.clone();
            assert!(patch(kind, &foreign, ControllerLayout::DualSense).is_err());
            assert_eq!(foreign, saved);
        }
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
#[ignore = "requires explicitly supplied read-only official samples; does not launch game"]
fn real_samples_and_optional_offline_export() {
    let samples = [
        ("ERCUI_OFFICIAL_GFX", Movie::Options, OFFICIAL_OPTIONS_GFX),
        (
            "ERCUI_COMMON_GFX",
            Movie::CommonOptions,
            OFFICIAL_COMMON_GFX,
        ),
        ("ERCUI_KEY_GFX", Movie::KeyConfig, OFFICIAL_KEY_GFX),
    ];
    for (variable, kind, fixture) in samples {
        let bytes = std::fs::read(std::env::var(variable).expect(variable)).unwrap();
        assert_eq!(bytes, fixture);
        assert!(kind.accepts(&bytes, &sha256::digest(&bytes)));
    }
    if let Ok(directory) = std::env::var("ERCUI_EXPORT_GFX")
        && !directory.is_empty()
    {
        let directory = std::path::Path::new(&directory);
        assert!(directory.is_dir(), "export directory must already exist");
        for layout in LAYOUTS {
            for (kind, data, name) in [
                (Movie::Options, OFFICIAL_OPTIONS_GFX, "options"),
                (Movie::KeyConfig, OFFICIAL_KEY_GFX, "keyconfig"),
            ] {
                let output = patch(kind, data, layout).unwrap();
                std::fs::write(
                    directory.join(format!("{}-{name}.gfx", layout.as_str())),
                    output,
                )
                .unwrap();
            }
        }
    }
}
