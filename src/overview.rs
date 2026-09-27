//! 用最小官方布局描述重建 PC 总览；不打开其他文件，不内嵌电影、图片或逐键高亮。
use super::*;
#[path = "overview_data.rs"]
mod data;

struct ImageSpec {
    name: &'static [u8],
    size: [u16; 2],
}
struct Preset {
    big: ImageSpec,
    lines: ImageSpec,
    big_scale: i32,
    big_position: [i32; 2],
    line_scale: i32,
    line_position: [i32; 2],
    line_color: Option<[i16; 8]>,
    labels: [[i32; 2]; 18],
}
const LABELS: [&[u8]; 18] = [
    b"L2", b"L1", b"L3", b"L3_T", b"LU", b"LL", b"LR", b"LD", b"Back", b"Start", b"R3_T", b"R3",
    b"RL", b"RU", b"RD", b"RR", b"R1", b"R2",
];

// MATRIX is a bit stream, with signed twips and 16.16 scale values. Never use floating point or
// guessed atlas coordinates. Minimal field widths reproduce the verified native matrix encoding.
fn matrix(scale: Option<i32>, position: [i32; 2]) -> Vec<u8> {
    fn width(value: i32) -> usize {
        if value == 0 {
            0
        } else {
            33 - (if value < 0 { !value } else { value } as u32).leading_zeros() as usize
        }
    }
    let mut bits = Vec::with_capacity(112);
    let mut push = |value: u32, count: usize| {
        for shift in (0..count).rev() {
            bits.push(((value >> shift) & 1) as u8);
        }
    };
    push(u32::from(scale.is_some()), 1);
    if let Some(scale) = scale {
        let count = width(scale);
        push(count as u32, 5);
        push(scale as u32, count);
        push(scale as u32, count);
    }
    push(0, 1); // No rotation in these official presets.
    let count = width(position[0]).max(width(position[1]));
    push(count as u32, 5);
    push(position[0] as u32, count);
    push(position[1] as u32, count);
    bits.chunks(8)
        .map(|chunk| {
            chunk
                .iter()
                .enumerate()
                .fold(0, |value, (bit, v)| value | (v << (7 - bit)))
        })
        .collect()
}

enum Tint {
    Unchanged,
    Set(Option<[i16; 8]>),
}
fn color(terms: &[i16; 8]) -> Vec<u8> {
    let count = terms
        .iter()
        .map(|v| {
            33 - (if *v < 0 {
                !i32::from(*v)
            } else {
                i32::from(*v)
            } as u32)
                .leading_zeros()
        })
        .max()
        .unwrap() as usize;
    let mut bits = vec![1u8, 1]; // Add terms and multiply terms.
    for shift in (0..4).rev() {
        bits.push(((count >> shift) & 1) as u8);
    }
    for value in terms {
        for shift in (0..count).rev() {
            bits.push(((*value as u32 >> shift) & 1) as u8);
        }
    }
    bits.chunks(8)
        .map(|chunk| {
            chunk
                .iter()
                .enumerate()
                .fold(0, |value, (bit, v)| value | (v << (7 - bit)))
        })
        .collect()
}
fn placed(
    tag: Tag<'_>,
    character: u16,
    scale: Option<i32>,
    position: [i32; 2],
    tint: Tint,
) -> Result<Vec<u8>> {
    let placement = placement(tag)?;
    let at = placement.character.ok_or("missing overview character")?;
    let range = placement.matrix.ok_or("missing overview matrix")?;
    if at + 2 > range.start {
        return Err("unexpected overview field order");
    }
    let mut body = tag.body.to_vec();
    body[at..at + 2].copy_from_slice(&character.to_le_bytes());
    if let Tint::Set(terms) = tint {
        let end = if body[0] & 8 != 0 {
            color_end(&body, range.end)?
        } else {
            range.end
        };
        body.splice(range.end..end, terms.as_ref().map_or_else(Vec::new, color));
        body[0] = (body[0] & !8) | (u8::from(terms.is_some()) << 3);
    }
    body.splice(range, matrix(scale, position));
    if tag.raw.len() - tag.body.len() == 2 && body.len() < 63 {
        let mut out = ((tag.code << 6) | body.len() as u16).to_le_bytes().to_vec();
        out.extend_from_slice(&body);
        Ok(out)
    } else {
        Ok(encode(tag.code, &body))
    }
}

impl ImageSpec {
    fn declare(&self, top: &[Tag<'_>], reserved: u16, additions: &mut Vec<Vec<u8>>) -> Result<u16> {
        let mut body = Vec::new();
        for value in [reserved, 0, 13, self.size[0], self.size[1]] {
            body.extend_from_slice(&value.to_le_bytes());
        }
        body.push(self.name.len() as u8);
        body.extend_from_slice(self.name);
        body.push((self.name.len() + 4) as u8);
        body.extend_from_slice(self.name);
        body.extend_from_slice(b".tga");
        import_image(
            top,
            Tag {
                code: 1009,
                raw: &[],
                body: &body,
            },
            reserved,
            additions,
        )
    }
}

pub(super) fn patch(header: &[u8], top: &[Tag<'_>], layout: ControllerLayout) -> Result<Vec<u8>> {
    let preset = match layout {
        ControllerLayout::DualSense => &data::DUALSENSE,
        ControllerLayout::DualShock4 => &data::DUALSHOCK4,
        ControllerLayout::XboxSeries => &data::XBOX_SERIES,
        ControllerLayout::Original => return Err("original requires no overview reconstruction"),
    };
    let mut additions = Vec::new();
    let big = preset.big.declare(top, 512, &mut additions)?;
    let lines = preset.lines.declare(top, 513, &mut additions)?;
    let mut edits = Vec::new();
    for (sid, expected_count) in [(161, 1), (164, 20)] {
        let original = sprite(top, sid)?;
        if word(original.body, 2)? != 1 {
            return Err("unexpected overview frame count");
        }
        let mut body = original.body[..4].to_vec();
        let mut count = 0;
        for tag in tags(&original.body[4..])? {
            if matches!(tag.code, 26 | 70) {
                let p = placement(tag)?;
                let (character, scale, position, depth, name): (_, _, _, _, Option<&[u8]>) =
                    if sid == 161 {
                        (lines, Some(preset.line_scale), [0, 0], 1, None)
                    } else {
                        match count {
                            0 => (big, Some(preset.big_scale), preset.big_position, 1, None),
                            1 => (161, None, preset.line_position, 2, None),
                            2..=19 => (
                                163,
                                None,
                                preset.labels[count - 2],
                                (count as u16 - 1) * 4,
                                Some(LABELS[count - 2]),
                            ),
                            _ => return Err("unexpected overview placement count"),
                        }
                    };
                if p.depth != depth || p.name.as_ref().map(|r| &tag.body[r.clone()]) != name {
                    return Err("unexpected overview node");
                }
                let tint = if sid == 164 && count == 1 {
                    Tint::Set(preset.line_color)
                } else {
                    Tint::Unchanged
                };
                body.extend_from_slice(&placed(tag, character, scale, position, tint)?);
                count += 1;
            } else if matches!(tag.code, 0 | 1) {
                body.extend_from_slice(tag.raw);
            } else {
                return Err("unexpected overview timeline tag");
            }
        }
        if count != expected_count {
            return Err("incomplete overview");
        }
        edits.push((sid, encode(39, &body)));
    }
    assemble(header, top, &edits, &additions)
}
