//! 原生按钮预设与最小官方总览描述；不加载供体文件，不改变输入。
use crate::{config::ControllerLayout, sha256};
use std::ops::Range;
#[path = "overview.rs"]
mod overview;

#[cfg(all(test, feature = "game-fixtures"))]
pub const OFFICIAL_OPTIONS_GFX: &[u8] = include_bytes!(env!("ERCUI_OFFICIAL_GFX"));
#[cfg(all(test, feature = "game-fixtures"))]
pub const OFFICIAL_COMMON_GFX: &[u8] = include_bytes!(env!("ERCUI_COMMON_GFX"));
#[cfg(all(test, feature = "game-fixtures"))]
pub const OFFICIAL_KEY_GFX: &[u8] = include_bytes!(env!("ERCUI_KEY_GFX"));
#[cfg(all(test, feature = "game-fixtures"))]
pub const OFFICIAL_GFX_LENGTH: usize = Movie::Options.length();
#[cfg(all(test, feature = "game-fixtures"))]
pub const OFFICIAL_GFX_SHA256: [u8; 32] = Movie::Options.hash();
#[cfg(all(test, feature = "game-fixtures"))]
pub fn build_options_gfx(layout: ControllerLayout) -> Result<(Vec<u8>, ())> {
    patch(Movie::Options, OFFICIAL_OPTIONS_GFX, layout).map(|bytes| (bytes, ()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Movie {
    Options,
    KeyConfig,
    #[cfg(all(test, feature = "game-fixtures"))]
    CommonOptions,
}
impl Movie {
    pub const fn length(self) -> usize {
        match self {
            Self::Options => 44_007,
            Self::KeyConfig => 55_592,
            #[cfg(all(test, feature = "game-fixtures"))]
            Self::CommonOptions => 46_401,
        }
    }
    pub const fn hash(self) -> [u8; 32] {
        match self {
            Self::Options => {
                hex(b"170996C2376BB14675FE1BB308C3CE82C28BEC0DEAFBFC8E40BD8FEF0E99E4B4")
            }
            Self::KeyConfig => {
                hex(b"693D6B509C01A4758BE63A17C55D043C0133FA2684B3A53CF5F21875202FF137")
            }
            #[cfg(all(test, feature = "game-fixtures"))]
            Self::CommonOptions => {
                hex(b"4F80A029D8D6BDB7C6C893F13704E44C592C670C5ADFC8BA6B11C793FB5E0392")
            }
        }
    }
    pub fn accepts(self, bytes: &[u8], digest: &[u8; 32]) -> bool {
        bytes.len() == self.length() && digest == &self.hash()
    }
}
const fn hex(text: &[u8; 64]) -> [u8; 32] {
    const fn digit(v: u8) -> u8 {
        if v <= b'9' { v - b'0' } else { v - b'A' + 10 }
    }
    let mut result = [0; 32];
    let mut index = 0;
    while index < 32 {
        result[index] = digit(text[index * 2]) * 16 + digit(text[index * 2 + 1]);
        index += 1;
    }
    result
}

type Result<T> = std::result::Result<T, &'static str>;
#[derive(Clone, Copy)]
struct Tag<'a> {
    code: u16,
    raw: &'a [u8],
    body: &'a [u8],
}
fn word(bytes: &[u8], at: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        bytes
            .get(at..at + 2)
            .ok_or("short word")?
            .try_into()
            .unwrap(),
    ))
}
fn tags(bytes: &[u8]) -> Result<Vec<Tag<'_>>> {
    let mut result = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let start = at;
        let header = word(bytes, at)?;
        at += 2;
        let length = if header & 63 == 63 {
            let value = u32::from_le_bytes(
                bytes
                    .get(at..at + 4)
                    .ok_or("short long tag")?
                    .try_into()
                    .unwrap(),
            ) as usize;
            at += 4;
            value
        } else {
            (header & 63) as usize
        };
        let end = at.checked_add(length).ok_or("tag overflow")?;
        let body = bytes.get(at..end).ok_or("truncated tag")?;
        result.push(Tag {
            code: header >> 6,
            raw: &bytes[start..end],
            body,
        });
        at = end;
        if header >> 6 == 0 {
            if length != 0 || at != bytes.len() {
                return Err("nonterminal end tag");
            }
            return Ok(result);
        }
    }
    Err("missing end tag")
}
fn movie(bytes: &[u8]) -> Result<(&[u8], Vec<Tag<'_>>)> {
    if bytes.len() < 22
        || bytes.len() > 512 * 1024
        || &bytes[..4] != b"GFX\x0b"
        || u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize != bytes.len()
    {
        return Err("unsupported movie header");
    }
    let start = 8 + (5 + 4 * (bytes[8] as usize >> 3)).div_ceil(8) + 4;
    Ok((
        bytes.get(..start).ok_or("short movie header")?,
        tags(bytes.get(start..).ok_or("short movie")?)?,
    ))
}
fn encode(code: u16, body: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(body.len() + 6);
    bytes.extend_from_slice(&((code << 6) | 63).to_le_bytes());
    bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
    bytes.extend_from_slice(body);
    bytes
}
fn sprite<'a>(top: &[Tag<'a>], id: u16) -> Result<Tag<'a>> {
    let mut found = top
        .iter()
        .filter(|tag| tag.code == 39 && word(tag.body, 0) == Ok(id));
    let result = *found.next().ok_or("missing sprite")?;
    if found.next().is_some() {
        return Err("duplicate sprite");
    }
    Ok(result)
}

struct Bits<'a> {
    bytes: &'a [u8],
    bit: usize,
}
impl Bits<'_> {
    fn take(&mut self, count: usize) -> Result<usize> {
        if count > 32 || self.bit + count > self.bytes.len() * 8 {
            return Err("short bit field");
        }
        let mut result = 0;
        for _ in 0..count {
            result =
                (result << 1) | usize::from((self.bytes[self.bit / 8] >> (7 - self.bit % 8)) & 1);
            self.bit += 1;
        }
        Ok(result)
    }
    fn skip(&mut self, count: usize) -> Result<()> {
        if self.bit + count > self.bytes.len() * 8 {
            return Err("short bit field");
        }
        self.bit += count;
        Ok(())
    }
}
fn matrix_end(bytes: &[u8], at: usize) -> Result<usize> {
    let mut bits = Bits { bytes, bit: at * 8 };
    for _ in 0..2 {
        if bits.take(1)? != 0 {
            let count = bits.take(5)?;
            bits.skip(count * 2)?;
        }
    }
    let count = bits.take(5)?;
    bits.skip(count * 2)?;
    Ok(bits.bit.div_ceil(8))
}
fn color_end(bytes: &[u8], at: usize) -> Result<usize> {
    let mut bits = Bits { bytes, bit: at * 8 };
    let add = bits.take(1)?;
    let multiply = bits.take(1)?;
    let count = bits.take(4)?;
    bits.skip((add + multiply) * 4 * count)?;
    Ok(bits.bit.div_ceil(8))
}
fn string_end(bytes: &[u8], at: usize) -> Result<usize> {
    Ok(at
        + bytes
            .get(at..)
            .ok_or("short string")?
            .iter()
            .position(|v| *v == 0)
            .ok_or("unterminated string")?)
}
#[derive(Debug)]
struct Placement {
    character: Option<usize>,
    matrix: Option<Range<usize>>,
    name: Option<Range<usize>>,
    depth: u16,
}
fn placement(tag: Tag<'_>) -> Result<Placement> {
    if !matches!(tag.code, 26 | 70) {
        return Err("not a placement");
    }
    let flags = *tag.body.first().ok_or("short placement")?;
    let mut at = if tag.code == 70 { 2 } else { 1 };
    let depth = word(tag.body, at)?;
    at += 2;
    // GFX uses only HasClassName here, not SWF's HasImage && HasCharacter rule.
    if tag.code == 70 && tag.body[1] & 8 != 0 {
        at = string_end(tag.body, at)? + 1;
    }
    let character = if flags & 2 != 0 {
        word(tag.body, at)?;
        let start = at;
        at += 2;
        Some(start)
    } else {
        None
    };
    let matrix = if flags & 4 != 0 {
        let start = at;
        at = matrix_end(tag.body, at)?;
        Some(start..at)
    } else {
        None
    };
    if flags & 8 != 0 {
        at = color_end(tag.body, at)?;
    }
    if flags & 16 != 0 {
        word(tag.body, at)?;
        at += 2;
    }
    let name = if flags & 32 != 0 {
        Some(at..string_end(tag.body, at)?)
    } else {
        None
    };
    Ok(Placement {
        character,
        matrix,
        name,
        depth,
    })
}
#[cfg(all(test, feature = "game-fixtures"))]
fn refs(sprite: Tag<'_>, new_id: u16, ids: &[(u16, u16)]) -> Result<Vec<u8>> {
    let mut body = sprite.body.get(..4).ok_or("short sprite")?.to_vec();
    body[..2].copy_from_slice(&new_id.to_le_bytes());
    for tag in tags(&sprite.body[4..])? {
        if matches!(tag.code, 26 | 70) {
            let p = placement(tag)?;
            let at = p.character.ok_or("expected character")?;
            let id = word(tag.body, at)?;
            let target = ids
                .iter()
                .find(|(from, _)| *from == id)
                .ok_or("unexpected dependency")?
                .1;
            let mut changed = tag.body.to_vec();
            changed[at..at + 2].copy_from_slice(&target.to_le_bytes());
            // Preserve the original tag encoding, including short/long headers.
            let prefix = tag.raw.len() - tag.body.len();
            body.extend_from_slice(&tag.raw[..prefix]);
            body.extend_from_slice(&changed);
        } else if matches!(tag.code, 0 | 1) {
            body.extend_from_slice(tag.raw);
        } else {
            return Err("unexpected controller timeline tag");
        }
    }
    Ok(encode(39, &body))
}
fn named<'a>(sprite: Tag<'a>, name: &[u8]) -> Result<Tag<'a>> {
    let mut found = None;
    for tag in tags(&sprite.body[4..])? {
        if matches!(tag.code, 26 | 70)
            && let Some(range) = placement(tag)?.name
            && &tag.body[range] == name
            && found.replace(tag).is_some()
        {
            return Err("duplicate instance");
        }
    }
    found.ok_or("missing instance")
}
fn swap_names(sprite: Tag<'_>, preset: &[u8]) -> Result<Vec<u8>> {
    named(sprite, b"Win64")?;
    named(sprite, preset)?;
    let mut body = sprite.body[..4].to_vec();
    for tag in tags(&sprite.body[4..])? {
        if matches!(tag.code, 26 | 70)
            && let Some(range) = placement(tag)?.name
        {
            let name = &tag.body[range.clone()];
            let replacement = if name == b"Win64" {
                Some(preset)
            } else if name == preset {
                Some(&b"Win64"[..])
            } else {
                None
            };
            if let Some(replacement) = replacement {
                let mut data = tag.body.to_vec();
                data.splice(range, replacement.iter().copied());
                body.extend_from_slice(&encode(tag.code, &data));
                continue;
            }
        }
        body.extend_from_slice(tag.raw);
    }
    Ok(encode(39, &body))
}

#[cfg(all(test, feature = "game-fixtures"))]
fn image<'a>(top: &[Tag<'a>], id: u16) -> Result<Tag<'a>> {
    top.iter()
        .copied()
        .find(|tag| tag.code == 1009 && word(tag.body, 0) == Ok(id))
        .ok_or("missing external image")
}
fn image_name(tag: Tag<'_>) -> Result<&[u8]> {
    let size = usize::from(*tag.body.get(10).ok_or("short image")?);
    tag.body.get(11..11 + size).ok_or("short image name")
}
fn import_image(
    top: &[Tag<'_>],
    source: Tag<'_>,
    reserved: u16,
    additions: &mut Vec<Vec<u8>>,
) -> Result<u16> {
    let name = image_name(source)?;
    for tag in top.iter().copied().filter(|tag| tag.code == 1009) {
        if image_name(tag)? == name {
            if tag.body[2..] != source.body[2..] {
                return Err("incompatible existing image");
            }
            return word(tag.body, 0);
        }
    }
    // The input is fingerprinted. Additionally reject collisions in character definitions/imports.
    for tag in top {
        if matches!(tag.code, 2 | 22 | 32 | 37 | 39 | 1009) && word(tag.body, 0)? == reserved {
            return Err("reserved ID collision");
        }
        if tag.code == 71 {
            let mut at = string_end(tag.body, 0)? + 3;
            let count = word(tag.body, at)?;
            at += 2;
            for _ in 0..count {
                if word(tag.body, at)? == reserved {
                    return Err("import ID collision");
                }
                at = string_end(tag.body, at + 2)? + 1;
            }
        }
    }
    let mut data = source.body.to_vec();
    data[..2].copy_from_slice(&reserved.to_le_bytes());
    additions.push(encode(1009, &data));
    Ok(reserved)
}
#[cfg(all(test, feature = "game-fixtures"))]
fn tab_frame(sprite: Tag<'_>, frame: usize) -> Result<Tag<'_>> {
    let mut current = 1;
    let mut found = None;
    for tag in tags(&sprite.body[4..])? {
        if tag.code == 1 {
            current += 1;
        } else if current == frame && matches!(tag.code, 26 | 70) {
            if found.replace(tag).is_some() {
                return Err("multiple tab placements");
            }
            let p = placement(tag)?;
            if p.depth != 1
                || p.character.is_none()
                || p.matrix.is_none()
                || p.name.is_some()
                || tag.body[0] & 1 != 0
            {
                return Err("unexpected tab placement");
            }
        }
    }
    found.ok_or("missing tab frame")
}
fn assemble(
    header: &[u8],
    top: &[Tag<'_>],
    edits: &[(u16, Vec<u8>)],
    additions: &[Vec<u8>],
) -> Result<Vec<u8>> {
    let mut output = header.to_vec();
    let mut inserted = false;
    let mut changed = 0;
    for tag in top {
        if tag.code == 39 {
            if !inserted {
                for extra in additions {
                    output.extend_from_slice(extra);
                }
                inserted = true;
            }
            if let Some((_, data)) = edits.iter().find(|(id, _)| word(tag.body, 0) == Ok(*id)) {
                output.extend_from_slice(data);
                changed += 1;
                continue;
            }
        }
        output.extend_from_slice(tag.raw);
    }
    if changed != edits.len() || !inserted {
        return Err("wrong edit count");
    }
    let length = output.len() as u32;
    output[4..8].copy_from_slice(&length.to_le_bytes());
    movie(&output)?;
    Ok(output)
}

pub fn patch(kind: Movie, source: &[u8], layout: ControllerLayout) -> Result<Vec<u8>> {
    let output = build(kind, source, layout)?;
    let (length, hash) = expected_output(kind, layout)?;
    if output.len() != length || sha256::digest(&output) != hash {
        return Err("generated movie fingerprint mismatch");
    }
    Ok(output)
}

fn expected_output(kind: Movie, layout: ControllerLayout) -> Result<(usize, [u8; 32])> {
    let result = match (kind, layout) {
        #[cfg(all(test, feature = "game-fixtures"))]
        (Movie::CommonOptions, _) => return Err("donor is not a replacement target"),
        (_, ControllerLayout::Original) => (kind.length(), kind.hash()),
        (Movie::Options, ControllerLayout::XboxSeries) => (
            44_117,
            hex(b"BC454046764E080BE75B7A2D1CEF4B0698650D620FD254126D0EBEDF2244297F"),
        ),
        (Movie::Options, ControllerLayout::DualShock4) => (
            44_006,
            hex(b"DE57F12C6F75BFBF3E5EE800103876015A8960C983409ED562BBE42B4A766548"),
        ),
        (Movie::Options, ControllerLayout::DualSense) => (
            44_082,
            hex(b"3EA063FCBC23DD9D6ECDFF6807E0A3C5889751376FB3A268E20C6978C24ACB0F"),
        ),
        (Movie::KeyConfig, ControllerLayout::XboxSeries) => (
            55_592,
            hex(b"DD25884BAB3440ECBCF42402CCC8BDCDDBEBD89D32D47EC6842878B78FF457A3"),
        ),
        (Movie::KeyConfig, ControllerLayout::DualShock4) => (
            55_592,
            hex(b"E4D245F0E3C85C7117FD06F14F4A6D896DDB450A91D4F9A36A1151C57094BAB2"),
        ),
        (Movie::KeyConfig, ControllerLayout::DualSense) => (
            55_592,
            hex(b"1E3C5E775980FDE2DE3078600CD63434B52BAF7553D6BDA307D75A19F573E8A4"),
        ),
    };
    Ok(result)
}

fn build(kind: Movie, source: &[u8], layout: ControllerLayout) -> Result<Vec<u8>> {
    if !kind.accepts(source, &sha256::digest(source)) {
        return Err("source fingerprint mismatch");
    }
    if layout == ControllerLayout::Original {
        return Ok(source.to_vec());
    }
    let (header, top) = movie(source)?;
    let preset: &[u8] = match layout {
        ControllerLayout::DualSense => b"PS5",
        ControllerLayout::DualShock4 => b"PS4",
        ControllerLayout::XboxSeries => b"XboxSeries",
        ControllerLayout::Original => unreachable!(),
    };
    if kind == Movie::KeyConfig {
        return assemble(
            header,
            &top,
            &[
                (110, swap_names(sprite(&top, 110)?, preset)?),
                (161, swap_names(sprite(&top, 161)?, preset)?),
            ],
            &[],
        );
    }
    if kind != Movie::Options {
        return Err("donor is not a replacement target");
    }
    overview::patch(header, &top, layout)
}

#[cfg(all(test, feature = "game-fixtures"))]
#[path = "gfx_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "gfx_unit_tests.rs"]
mod unit_tests;
