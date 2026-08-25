//! Immutable resident Search generation segment codec.

use std::collections::BTreeSet;

pub const WORKSPACE_SEARCH_GENERATION_SEGMENT_SCHEMA_ID: &str =
    "agent.semantic-protocols.workspace-memory-generation-segment";
const SEGMENT_MAGIC: &[u8; 16] = b"ASPWSSEARCHIDXV2";
const SEGMENT_HEADER_LEN: usize = 72;
const SECTION_DESCRIPTOR_LEN: usize = 64;
const REQUIRED_SECTION_COUNT: usize = 8;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum SearchGenerationSectionKind {
    GenerationEvidence = 1,
    ProjectResolutions = 2,
    OwnerDirectory = 3,
    OwnerBytes = 4,
    LexicalIndex = 5,
    SelectorIndex = 6,
    GraphRelations = 7,
    MerkleOwnerIndex = 8,
}

impl SearchGenerationSectionKind {
    fn from_byte(value: u8) -> Result<Self, String> {
        match value {
            1 => Ok(Self::GenerationEvidence),
            2 => Ok(Self::ProjectResolutions),
            3 => Ok(Self::OwnerDirectory),
            4 => Ok(Self::OwnerBytes),
            5 => Ok(Self::LexicalIndex),
            6 => Ok(Self::SelectorIndex),
            7 => Ok(Self::GraphRelations),
            8 => Ok(Self::MerkleOwnerIndex),
            _ => Err(format!("unknown search generation section kind {value}")),
        }
    }

    fn required() -> [Self; REQUIRED_SECTION_COUNT] {
        [
            Self::GenerationEvidence,
            Self::ProjectResolutions,
            Self::OwnerDirectory,
            Self::OwnerBytes,
            Self::LexicalIndex,
            Self::SelectorIndex,
            Self::GraphRelations,
            Self::MerkleOwnerIndex,
        ]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SearchGenerationSectionRepresentation {
    SortedOffsetTable = 1,
    Utf8Json = 2,
    OpaqueBytes = 3,
}

impl SearchGenerationSectionRepresentation {
    fn from_byte(value: u8) -> Result<Self, String> {
        match value {
            1 => Ok(Self::SortedOffsetTable),
            2 => Ok(Self::Utf8Json),
            3 => Ok(Self::OpaqueBytes),
            _ => Err(format!(
                "unknown search generation section representation {value}"
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SearchGenerationSection {
    pub kind: SearchGenerationSectionKind,
    pub representation: u8,
    pub record_count: u64,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
struct ValidatedSectionDescriptor {
    kind: SearchGenerationSectionKind,
    representation: SearchGenerationSectionRepresentation,
    offset: usize,
    len: usize,
    record_count: u64,
}

#[derive(Clone, Debug)]
pub struct ValidatedSearchGenerationSegment<'a> {
    bytes: &'a [u8],
    epoch: u64,
    sections: [ValidatedSectionDescriptor; REQUIRED_SECTION_COUNT],
}

pub fn encode_search_generation_segment(
    epoch: u64,
    sections: Vec<SearchGenerationSection>,
) -> Result<Vec<u8>, String> {
    if epoch == 0 {
        return Err("search generation epoch must be positive".to_owned());
    }
    let sections = canonical_search_generation_sections(sections)?;
    let (directory, total_len) = encode_search_generation_directory(&sections)?;
    finish_search_generation_segment(epoch, sections, directory, total_len)
}

fn canonical_search_generation_sections(
    mut sections: Vec<SearchGenerationSection>,
) -> Result<Vec<SearchGenerationSection>, String> {
    sections.sort_unstable_by_key(|section| section.kind);
    let actual: Vec<_> = sections.iter().map(|section| section.kind).collect();
    if actual != SearchGenerationSectionKind::required() {
        return Err("search generation must contain every v1 section exactly once".to_owned());
    }
    Ok(sections)
}

fn encode_search_generation_directory(
    sections: &[SearchGenerationSection],
) -> Result<(Vec<u8>, usize), String> {
    let directory_len = REQUIRED_SECTION_COUNT
        .checked_mul(SECTION_DESCRIPTOR_LEN)
        .ok_or_else(|| "search generation directory length overflow".to_owned())?;
    let payload_offset = SEGMENT_HEADER_LEN
        .checked_add(directory_len)
        .ok_or_else(|| "search generation payload offset overflow".to_owned())?;
    let payload_len = sections.iter().try_fold(0usize, |total, section| {
        total
            .checked_add(section.bytes.len())
            .ok_or_else(|| "search generation payload length overflow".to_owned())
    })?;
    let total_len = payload_offset
        .checked_add(payload_len)
        .ok_or_else(|| "search generation total length overflow".to_owned())?;

    let (directory, _) = sections.iter().try_fold(
        (Vec::with_capacity(directory_len), 0usize),
        |(mut directory, relative_offset), section| {
            directory.push(section.kind as u8);
            SearchGenerationSectionRepresentation::from_byte(section.representation)?;
            directory.push(section.representation);
            directory.extend_from_slice(&[0; 6]);
            write_u64(
                &mut directory,
                checked_u64(
                    payload_offset
                        .checked_add(relative_offset)
                        .ok_or_else(|| "search generation section offset overflow".to_owned())?,
                    "section offset",
                )?,
            );
            write_u64(
                &mut directory,
                checked_u64(section.bytes.len(), "section length")?,
            );
            write_u64(&mut directory, section.record_count);
            directory.extend_from_slice(
                section_commitment(section.representation, &section.bytes)?.as_bytes(),
            );
            let relative_offset = relative_offset
                .checked_add(section.bytes.len())
                .ok_or_else(|| "search generation section range overflow".to_owned())?;
            Ok::<_, String>((directory, relative_offset))
        },
    )?;
    Ok((directory, total_len))
}

fn finish_search_generation_segment(
    epoch: u64,
    sections: Vec<SearchGenerationSection>,
    directory: Vec<u8>,
    total_len: usize,
) -> Result<Vec<u8>, String> {
    let mut encoded = Vec::with_capacity(total_len);
    encoded.extend_from_slice(SEGMENT_MAGIC);
    write_u64(&mut encoded, epoch);
    encoded.extend_from_slice(blake3::hash(&directory).as_bytes());
    write_u64(
        &mut encoded,
        checked_u64(REQUIRED_SECTION_COUNT, "section count")?,
    );
    write_u64(&mut encoded, checked_u64(total_len, "total length")?);
    encoded.extend_from_slice(&directory);
    for section in sections {
        encoded.extend_from_slice(&section.bytes);
    }
    Ok(encoded)
}

impl<'a> ValidatedSearchGenerationSegment<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, String> {
        Self::parse_inner(bytes)
    }

    fn parse_inner(bytes: &'a [u8]) -> Result<Self, String> {
        if bytes.len() < SEGMENT_HEADER_LEN {
            return Err("search generation segment is truncated before its header".to_owned());
        }
        if &bytes[..SEGMENT_MAGIC.len()] != SEGMENT_MAGIC {
            return Err(format!(
                "{WORKSPACE_SEARCH_GENERATION_SEGMENT_SCHEMA_ID} magic is invalid"
            ));
        }
        let epoch = read_u64(bytes, 16)?;
        if epoch == 0 {
            return Err("search generation epoch must be positive".to_owned());
        }
        let expected_directory_digest = array_32(&bytes[24..56])?;
        let section_count = checked_usize(read_u64(bytes, 56)?, "section count")?;
        if section_count != REQUIRED_SECTION_COUNT {
            return Err(format!(
                "search generation requires {REQUIRED_SECTION_COUNT} sections; actual={section_count}"
            ));
        }
        let expected_total_len = checked_usize(read_u64(bytes, 64)?, "total length")?;
        if bytes.len() != expected_total_len {
            return Err("search generation segment total length mismatch".to_owned());
        }
        let directory_end = SEGMENT_HEADER_LEN
            .checked_add(
                section_count
                    .checked_mul(SECTION_DESCRIPTOR_LEN)
                    .ok_or_else(|| "search generation directory length overflow".to_owned())?,
            )
            .ok_or_else(|| "search generation directory range overflow".to_owned())?;
        let directory = bytes
            .get(SEGMENT_HEADER_LEN..directory_end)
            .ok_or_else(|| {
                "search generation segment is truncated inside its directory".to_owned()
            })?;
        if blake3::hash(directory).as_bytes() != &expected_directory_digest {
            return Err("search generation directory digest mismatch".to_owned());
        }

        let mut seen = BTreeSet::new();
        let mut descriptors = Vec::with_capacity(REQUIRED_SECTION_COUNT);
        let mut previous_end = directory_end;
        for index in 0..section_count {
            let start = index * SECTION_DESCRIPTOR_LEN;
            let descriptor = &directory[start..start + SECTION_DESCRIPTOR_LEN];
            if descriptor[2..8] != [0; 6] {
                return Err("search generation section reserved bytes must be zero".to_owned());
            }
            let kind = SearchGenerationSectionKind::from_byte(descriptor[0])?;
            if !seen.insert(kind) {
                return Err("search generation contains a duplicate section".to_owned());
            }
            let representation = SearchGenerationSectionRepresentation::from_byte(descriptor[1])?;
            let offset = checked_usize(read_u64(descriptor, 8)?, "section offset")?;
            let len = checked_usize(read_u64(descriptor, 16)?, "section length")?;
            let end = offset
                .checked_add(len)
                .ok_or_else(|| "search generation section range overflow".to_owned())?;
            if offset != previous_end || end > bytes.len() {
                return Err("search generation section ranges are not canonical".to_owned());
            }
            previous_end = end;
            let digest = array_32(&descriptor[32..64])?;
            if representation == SearchGenerationSectionRepresentation::Utf8Json {
                let section_bytes = bytes
                    .get(offset..end)
                    .ok_or_else(|| "search generation section range is out of bounds".to_owned())?;
                if blake3::hash(section_bytes).as_bytes() != &digest {
                    return Err("search generation section digest mismatch".to_owned());
                }
            } else if representation == SearchGenerationSectionRepresentation::SortedOffsetTable {
                let section = bytes
                    .get(offset..end)
                    .ok_or_else(|| "sorted-offset-table section is out of bounds".to_owned())?;
                let header = &section[..section.len().min(64)];
                if blake3::hash(header).as_bytes() != &digest {
                    return Err("sorted-offset-table header commitment mismatch".to_owned());
                }
            }
            descriptors.push(ValidatedSectionDescriptor {
                kind,
                representation,
                offset,
                len,
                record_count: read_u64(descriptor, 24)?,
            });
        }
        if previous_end != bytes.len()
            || seen
                != SearchGenerationSectionKind::required()
                    .into_iter()
                    .collect()
        {
            return Err("search generation section set or terminal range is invalid".to_owned());
        }
        let sections: [ValidatedSectionDescriptor; REQUIRED_SECTION_COUNT] = descriptors
            .try_into()
            .map_err(|_| "search generation section directory width is invalid".to_owned())?;
        Ok(Self {
            bytes,
            epoch,
            sections,
        })
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn section(&self, kind: SearchGenerationSectionKind) -> (&'a [u8], u8, u64) {
        let descriptor = self
            .sections
            .iter()
            .find(|section| section.kind == kind)
            .expect("validated v1 search generation contains every required section");
        let bytes = self
            .bytes
            .get(descriptor.offset..descriptor.offset + descriptor.len)
            .expect("validated v1 search generation section range is in bounds");
        (
            bytes,
            descriptor.representation as u8,
            descriptor.record_count,
        )
    }

    pub fn section_range(&self, kind: SearchGenerationSectionKind) -> std::ops::Range<usize> {
        let descriptor = self
            .sections
            .iter()
            .find(|section| section.kind == kind)
            .expect("validated v1 search generation contains every required section");
        descriptor.offset..descriptor.offset + descriptor.len
    }
}

fn section_commitment(representation: u8, bytes: &[u8]) -> Result<blake3::Hash, String> {
    match SearchGenerationSectionRepresentation::from_byte(representation)? {
        SearchGenerationSectionRepresentation::SortedOffsetTable => {
            let header = &bytes[..bytes.len().min(64)];
            Ok(blake3::hash(header))
        }
        SearchGenerationSectionRepresentation::Utf8Json => Ok(blake3::hash(bytes)),
        SearchGenerationSectionRepresentation::OpaqueBytes => Ok(blake3::hash(
            &u64::try_from(bytes.len())
                .map_err(|_| "opaque section length exceeds u64".to_owned())?
                .to_le_bytes(),
        )),
    }
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, String> {
    let end = offset
        .checked_add(8)
        .ok_or_else(|| "u64 field offset overflow".to_owned())?;
    let field = bytes
        .get(offset..end)
        .ok_or_else(|| "u64 field is truncated".to_owned())?;
    Ok(u64::from_le_bytes(field.try_into().map_err(|_| {
        "u64 field has an invalid width".to_owned()
    })?))
}

fn write_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn checked_u64(value: usize, field: &str) -> Result<u64, String> {
    u64::try_from(value).map_err(|_| format!("search generation {field} exceeds u64"))
}

fn checked_usize(value: u64, field: &str) -> Result<usize, String> {
    usize::try_from(value).map_err(|_| format!("search generation {field} exceeds usize"))
}

fn array_32(bytes: &[u8]) -> Result<[u8; 32], String> {
    bytes
        .try_into()
        .map_err(|_| "digest field has an invalid width".to_owned())
}

#[cfg(test)]
#[path = "../tests/unit/search_generation_segment.rs"]
mod tests;
