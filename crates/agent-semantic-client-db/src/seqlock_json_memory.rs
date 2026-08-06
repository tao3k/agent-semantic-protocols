use memmap2::{Mmap, MmapMut, MmapOptions};
use serde::{Serialize, de::DeserializeOwned};
use std::sync::atomic::{AtomicU64, Ordering};

const GENERATION_OFFSET: usize = 0;
const PAYLOAD_LENGTH_OFFSET: usize = 8;
const PAYLOAD_DIGEST_OFFSET: usize = 16;
const PAYLOAD_OFFSET: usize = 48;

fn generation(bytes: &[u8]) -> &AtomicU64 {
    // SAFETY: mappings are page aligned, GENERATION_OFFSET is aligned for u64,
    // and every mapping is retained for the lifetime of the returned atomic.
    unsafe { &*bytes.as_ptr().add(GENERATION_OFFSET).cast::<AtomicU64>() }
}

pub struct SeqlockJsonMemoryWriter {
    mapping: MmapMut,
    generation: u64,
}

impl SeqlockJsonMemoryWriter {
    pub fn next_committed_generation(&self) -> u64 {
        self.generation.saturating_add(2)
    }

    pub async fn create(path: &std::path::Path, capacity: usize) -> Result<Self, String> {
        if capacity <= PAYLOAD_OFFSET {
            return Err("seqlock JSON memory capacity must include a payload region".to_owned());
        }
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|error| format!("failed to create seqlock JSON memory parent: {error}"))?;
        }
        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(path)
            .await
            .map_err(|error| format!("failed to create seqlock JSON memory: {error}"))?;
        file.set_len(capacity as u64)
            .await
            .map_err(|error| format!("failed to size seqlock JSON memory: {error}"))?;
        #[cfg(unix)]
        file.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o600))
            .await
            .map_err(|error| format!("failed to protect seqlock JSON memory: {error}"))?;
        let file = file.into_std().await;
        // SAFETY: the elected writer exclusively initializes this fixed-size file
        // and retains the mapping for its full lifetime.
        let mapping = unsafe { MmapOptions::new().map_mut(&file) }
            .map_err(|error| format!("failed to map seqlock JSON memory writer: {error}"))?;
        Ok(Self {
            mapping,
            generation: 0,
        })
    }

    pub fn publish<T: Serialize>(&mut self, value: &T) -> Result<u64, String> {
        let payload = serde_json::to_vec(value)
            .map_err(|error| format!("failed to encode seqlock JSON payload: {error}"))?;
        self.publish_bytes(&payload)
    }

    pub fn publish_postcard<T: Serialize>(&mut self, value: &T) -> Result<u64, String> {
        let payload = postcard::to_allocvec(value)
            .map_err(|error| format!("failed to encode seqlock postcard payload: {error}"))?;
        self.publish_bytes(&payload)
    }

    fn publish_bytes(&mut self, payload: &[u8]) -> Result<u64, String> {
        if payload.len() > self.mapping.len().saturating_sub(PAYLOAD_OFFSET) {
            return Err(format!(
                "seqlock payload exceeds mapping: payloadBytes={} capacityBytes={}",
                payload.len(),
                self.mapping.len().saturating_sub(PAYLOAD_OFFSET)
            ));
        }
        let committed = self.generation.saturating_add(2);
        generation(&self.mapping).store(committed - 1, Ordering::Release);
        self.mapping[PAYLOAD_LENGTH_OFFSET..PAYLOAD_OFFSET].fill(0);
        self.mapping[PAYLOAD_LENGTH_OFFSET..PAYLOAD_LENGTH_OFFSET + 4]
            .copy_from_slice(&(payload.len() as u32).to_le_bytes());
        self.mapping[PAYLOAD_DIGEST_OFFSET..PAYLOAD_OFFSET]
            .copy_from_slice(blake3::hash(payload).as_bytes());
        self.mapping[PAYLOAD_OFFSET..PAYLOAD_OFFSET + payload.len()].copy_from_slice(&payload);
        generation(&self.mapping).store(committed, Ordering::Release);
        self.generation = committed;
        Ok(committed)
    }
}

pub struct SeqlockJsonMemoryReader {
    mapping: Mmap,
}

impl SeqlockJsonMemoryReader {
    pub fn stable_generation(&self) -> Option<u64> {
        let before = generation(&self.mapping).load(Ordering::Acquire);
        if before == 0 || before % 2 != 0 {
            return None;
        }
        let after = generation(&self.mapping).load(Ordering::Acquire);
        (before == after).then_some(after)
    }

    pub async fn open(path: &std::path::Path) -> Result<Self, String> {
        // Opening a fixed-size read-only mapping is the one cold locator step.
        // Routing it through Tokio's blocking pool costs more than the mapping
        // itself and violates the sub-millisecond admission budget. Publication
        // remains Tokio-owned; all reads after this point are memory-only.
        let file = std::fs::OpenOptions::new()
            .read(true)
            .open(path)
            .map_err(|error| format!("failed to open seqlock JSON memory: {error}"))?;
        // SAFETY: the elected writer owns sizing and publishes under the generation seqlock.
        let mapping = unsafe { MmapOptions::new().map(&file) }
            .map_err(|error| format!("failed to map seqlock JSON memory reader: {error}"))?;
        if mapping.len() <= PAYLOAD_OFFSET {
            return Err("seqlock JSON memory has an invalid size".to_owned());
        }
        Ok(Self { mapping })
    }

    pub fn read<T: DeserializeOwned>(&self) -> Result<(u64, T), String> {
        const MAX_CONCURRENT_PUBLICATION_RETRIES: usize = 64;
        for _ in 0..MAX_CONCURRENT_PUBLICATION_RETRIES {
            let before = generation(&self.mapping).load(Ordering::Acquire);
            if before == 0 || before % 2 != 0 {
                std::hint::spin_loop();
                continue;
            }
            let length = u32::from_le_bytes(
                self.mapping[PAYLOAD_LENGTH_OFFSET..PAYLOAD_LENGTH_OFFSET + 4]
                    .try_into()
                    .expect("fixed seqlock payload length field"),
            ) as usize;
            if length == 0 || PAYLOAD_OFFSET + length > self.mapping.len() {
                let after = generation(&self.mapping).load(Ordering::Acquire);
                if before != after || after % 2 != 0 {
                    std::hint::spin_loop();
                    continue;
                }
                return Err("seqlock JSON memory payload length is invalid".to_owned());
            }
            let payload = &self.mapping[PAYLOAD_OFFSET..PAYLOAD_OFFSET + length];
            let expected_digest: [u8; 32] = self.mapping[PAYLOAD_DIGEST_OFFSET..PAYLOAD_OFFSET]
                .try_into()
                .expect("fixed seqlock payload digest field");
            let actual_digest = *blake3::hash(payload).as_bytes();
            let decoded = serde_json::from_slice(payload);
            let after = generation(&self.mapping).load(Ordering::Acquire);
            if before != after || after % 2 != 0 {
                std::hint::spin_loop();
                continue;
            }
            if actual_digest != expected_digest {
                return Err("seqlock JSON memory payload digest mismatch".to_owned());
            }
            let value = decoded
                .map_err(|error| format!("failed to decode seqlock JSON payload: {error}"))?;
            return Ok((after, value));
        }
        Err("seqlock JSON publication did not stabilize within the read boundary".to_owned())
    }

    pub fn read_postcard<T: DeserializeOwned>(&self) -> Result<(u64, T), String> {
        const MAX_CONCURRENT_PUBLICATION_RETRIES: usize = 64;
        for _ in 0..MAX_CONCURRENT_PUBLICATION_RETRIES {
            let before = generation(&self.mapping).load(Ordering::Acquire);
            if before == 0 || before % 2 != 0 {
                std::hint::spin_loop();
                continue;
            }
            let length = u32::from_le_bytes(
                self.mapping[PAYLOAD_LENGTH_OFFSET..PAYLOAD_LENGTH_OFFSET + 4]
                    .try_into()
                    .expect("fixed seqlock payload length field"),
            ) as usize;
            if length == 0 || PAYLOAD_OFFSET + length > self.mapping.len() {
                let after = generation(&self.mapping).load(Ordering::Acquire);
                if before != after || after % 2 != 0 {
                    std::hint::spin_loop();
                    continue;
                }
                return Err("seqlock postcard memory payload length is invalid".to_owned());
            }
            let payload = &self.mapping[PAYLOAD_OFFSET..PAYLOAD_OFFSET + length];
            let expected_digest: [u8; 32] = self.mapping[PAYLOAD_DIGEST_OFFSET..PAYLOAD_OFFSET]
                .try_into()
                .expect("fixed seqlock payload digest field");
            let actual_digest = *blake3::hash(payload).as_bytes();
            let decoded = postcard::from_bytes(payload);
            let after = generation(&self.mapping).load(Ordering::Acquire);
            if before != after || after % 2 != 0 {
                std::hint::spin_loop();
                continue;
            }
            if actual_digest != expected_digest {
                return Err("seqlock postcard memory payload digest mismatch".to_owned());
            }
            let value = decoded
                .map_err(|error| format!("failed to decode seqlock postcard payload: {error}"))?;
            return Ok((after, value));
        }
        Err("seqlock postcard publication did not stabilize within the read boundary".to_owned())
    }
}
