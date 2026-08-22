use std::cmp::Ordering;

const SORTED_RECORD_TABLE_MAGIC: &[u8; 16] = b"ASPSORTEDTABLE1\0";
const SORTED_RECORD_TABLE_HEADER_LEN: usize = 64;
const SORTED_RECORD_TABLE_ENTRY_LEN: usize = 64;

#[derive(Clone, Copy, Debug)]
struct SortedRecordEntry {
    key_offset: usize,
    key_len: usize,
    value_offset: usize,
    value_len: usize,
    record_digest: [u8; 32],
}

#[derive(Clone, Copy, Debug)]
pub struct ValidatedSortedRecordTable<'a> {
    bytes: &'a [u8],
    entry_count: usize,
    payload_offset: usize,
}

pub fn encode_sorted_record_table(mut records: Vec<(Vec<u8>, Vec<u8>)>) -> Result<Vec<u8>, String> {
    records.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    if records.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err("sorted record table contains a duplicate key".to_owned());
    }

    let directory_len = records
        .len()
        .checked_mul(SORTED_RECORD_TABLE_ENTRY_LEN)
        .ok_or_else(|| "sorted record table directory length overflow".to_owned())?;
    let payload_offset = SORTED_RECORD_TABLE_HEADER_LEN
        .checked_add(directory_len)
        .ok_or_else(|| "sorted record table payload offset overflow".to_owned())?;
    let mut directory = Vec::with_capacity(directory_len);
    let mut payload = Vec::new();

    for (key, value) in records {
        let key_offset = payload.len();
        payload.extend_from_slice(&key);
        let value_offset = payload.len();
        payload.extend_from_slice(&value);

        write_u64(&mut directory, checked_u64(key_offset, "key offset")?);
        write_u64(&mut directory, checked_u64(key.len(), "key length")?);
        write_u64(&mut directory, checked_u64(value_offset, "value offset")?);
        write_u64(&mut directory, checked_u64(value.len(), "value length")?);
        directory.extend_from_slice(record_digest(&key, &value).as_bytes());
    }

    let total_len = payload_offset
        .checked_add(payload.len())
        .ok_or_else(|| "sorted record table total length overflow".to_owned())?;
    let mut encoded = Vec::with_capacity(total_len);
    encoded.extend_from_slice(SORTED_RECORD_TABLE_MAGIC);
    write_u64(
        &mut encoded,
        checked_u64(
            directory_len / SORTED_RECORD_TABLE_ENTRY_LEN,
            "record count",
        )?,
    );
    encoded.extend_from_slice(blake3::hash(&directory).as_bytes());
    write_u64(&mut encoded, checked_u64(total_len, "total length")?);
    encoded.extend_from_slice(&directory);
    encoded.extend_from_slice(&payload);
    Ok(encoded)
}

impl<'a> ValidatedSortedRecordTable<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, String> {
        if bytes.len() < SORTED_RECORD_TABLE_HEADER_LEN {
            return Err("sorted record table is truncated before its header".to_owned());
        }
        if &bytes[..SORTED_RECORD_TABLE_MAGIC.len()] != SORTED_RECORD_TABLE_MAGIC {
            return Err("sorted record table magic is invalid".to_owned());
        }
        let entry_count = checked_usize(read_u64(bytes, 16)?, "record count")?;
        let expected_total_len = checked_usize(read_u64(bytes, 56)?, "total length")?;
        if bytes.len() != expected_total_len {
            return Err("sorted record table total length mismatch".to_owned());
        }
        let directory_len = entry_count
            .checked_mul(SORTED_RECORD_TABLE_ENTRY_LEN)
            .ok_or_else(|| "sorted record table directory length overflow".to_owned())?;
        let payload_offset = SORTED_RECORD_TABLE_HEADER_LEN
            .checked_add(directory_len)
            .ok_or_else(|| "sorted record table payload offset overflow".to_owned())?;
        if payload_offset > bytes.len() {
            return Err("sorted record table is truncated inside its directory".to_owned());
        }
        let directory_commitment = array_32(&bytes[24..56])?;
        if directory_commitment == [0; 32] {
            return Err("sorted record table directory commitment is empty".to_owned());
        }
        Ok(Self {
            bytes,
            entry_count,
            payload_offset,
        })
    }

    pub fn len(&self) -> usize {
        self.entry_count
    }

    pub fn is_empty(&self) -> bool {
        self.entry_count == 0
    }

    #[cfg(test)]
    fn get(&self, key: &[u8]) -> Option<&'a [u8]> {
        self.get_checked(key).ok().flatten()
    }

    pub fn get_checked(&self, key: &[u8]) -> Result<Option<&'a [u8]>, String> {
        let mut lower = 0usize;
        let mut upper = self.entry_count;
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            let entry = match self.entry(middle) {
                Ok(entry) => entry,
                Err(error) => return Err(error),
            };
            let candidate = match self.payload_slice(entry.key_offset, entry.key_len, "key") {
                Ok(candidate) => candidate,
                Err(error) => return Err(error),
            };
            match candidate.cmp(key) {
                Ordering::Less => lower = middle + 1,
                Ordering::Greater => upper = middle,
                Ordering::Equal => {
                    let value =
                        match self.payload_slice(entry.value_offset, entry.value_len, "value") {
                            Ok(value) => value,
                            Err(error) => return Err(error),
                        };
                    if record_digest(candidate, value).as_bytes() != &entry.record_digest {
                        return Err("sorted record table record digest mismatch".to_owned());
                    }
                    return Ok(Some(value));
                }
            }
        }
        Ok(None)
    }

    pub fn owned_records(&self) -> Result<Vec<(Vec<u8>, Vec<u8>)>, String> {
        let mut records = Vec::with_capacity(self.entry_count);
        for index in 0..self.entry_count {
            let entry = self.entry(index)?;
            let key = self.payload_slice(entry.key_offset, entry.key_len, "key")?;
            let value = self.payload_slice(entry.value_offset, entry.value_len, "value")?;
            if record_digest(key, value).as_bytes() != &entry.record_digest {
                return Err("sorted record table record digest mismatch".to_owned());
            }
            records.push((key.to_vec(), value.to_vec()));
        }
        Ok(records)
    }

    fn entry(&self, index: usize) -> Result<SortedRecordEntry, String> {
        let start = SORTED_RECORD_TABLE_HEADER_LEN
            .checked_add(
                index
                    .checked_mul(SORTED_RECORD_TABLE_ENTRY_LEN)
                    .ok_or_else(|| "sorted record table entry offset overflow".to_owned())?,
            )
            .ok_or_else(|| "sorted record table entry offset overflow".to_owned())?;
        let end = start
            .checked_add(SORTED_RECORD_TABLE_ENTRY_LEN)
            .ok_or_else(|| "sorted record table entry range overflow".to_owned())?;
        let bytes = self
            .bytes
            .get(start..end)
            .ok_or_else(|| "sorted record table entry is truncated".to_owned())?;
        Ok(SortedRecordEntry {
            key_offset: checked_usize(read_u64(bytes, 0)?, "key offset")?,
            key_len: checked_usize(read_u64(bytes, 8)?, "key length")?,
            value_offset: checked_usize(read_u64(bytes, 16)?, "value offset")?,
            value_len: checked_usize(read_u64(bytes, 24)?, "value length")?,
            record_digest: array_32(&bytes[32..64])?,
        })
    }

    fn payload_slice(
        &self,
        relative_offset: usize,
        len: usize,
        field: &str,
    ) -> Result<&'a [u8], String> {
        let start = self
            .payload_offset
            .checked_add(relative_offset)
            .ok_or_else(|| format!("sorted record table {field} offset overflow"))?;
        let end = start
            .checked_add(len)
            .ok_or_else(|| format!("sorted record table {field} range overflow"))?;
        self.bytes
            .get(start..end)
            .ok_or_else(|| format!("sorted record table {field} range is out of bounds"))
    }
}

fn record_digest(key: &[u8], value: &[u8]) -> blake3::Hash {
    let mut hasher = blake3::Hasher::new();
    hasher.update(key);
    hasher.update(value);
    hasher.finalize()
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
    u64::try_from(value).map_err(|_| format!("sorted record table {field} exceeds u64"))
}

fn checked_usize(value: u64, field: &str) -> Result<usize, String> {
    usize::try_from(value).map_err(|_| format!("sorted record table {field} exceeds usize"))
}

fn array_32(bytes: &[u8]) -> Result<[u8; 32], String> {
    bytes
        .try_into()
        .map_err(|_| "digest field has an invalid width".to_owned())
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_search_index_table.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_search_index_table_integrity.rs"]
mod integrity_tests;

#[cfg(test)]
#[path = "../../tests/unit/runtime_server_search_index_cold_open.rs"]
mod performance_tests;
