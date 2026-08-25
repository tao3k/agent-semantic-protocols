use std::fmt;
use std::str::FromStr;

use agent_semantic_content_identity::exact_selector_merkle::{
    ContentDigestV1, parse_content_digest_v1,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

const PREFIX: &str = "blake3-256:";

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Blake3ContentDigest {
    canonical: String,
    content: ContentDigestV1,
}

impl Blake3ContentDigest {
    pub fn parse(value: &str) -> Result<Self, String> {
        let raw = value
            .strip_prefix(PREFIX)
            .ok_or_else(|| format!("BLAKE3 content digest must start with `{PREFIX}`"))?;
        let content = parse_content_digest_v1(raw)?;
        Ok(Self {
            canonical: format!("{PREFIX}{}", content.as_str()),
            content,
        })
    }

    pub fn from_content_digest(content: ContentDigestV1) -> Self {
        Self {
            canonical: format!("{PREFIX}{}", content.as_str()),
            content,
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self::from_content_digest(
            agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1(bytes),
        )
    }

    pub fn as_str(&self) -> &str {
        &self.canonical
    }

    pub fn content_digest(&self) -> &ContentDigestV1 {
        &self.content
    }

    pub fn into_content_digest(self) -> ContentDigestV1 {
        self.content
    }
}

impl fmt::Display for Blake3ContentDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for Blake3ContentDigest {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl Serialize for Blake3ContentDigest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Blake3ContentDigest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::Blake3ContentDigest;

    const REAL_DIGEST: &str =
        "blake3-256:9313893f2985088dc3e5b14fdfd4877b0ed37c8dc8d5ef60bd6531ab5678abe1";

    #[test]
    fn precise_production_digest_roundtrips_through_display_and_serde() {
        let digest = Blake3ContentDigest::parse(REAL_DIGEST).expect("parse production digest");
        assert_eq!(digest.to_string(), REAL_DIGEST);
        let encoded = serde_json::to_string(&digest).expect("encode digest");
        let decoded: Blake3ContentDigest = serde_json::from_str(&encoded).expect("decode digest");
        assert_eq!(decoded, digest);
        assert_eq!(
            decoded.content_digest().as_str(),
            &REAL_DIGEST["blake3-256:".len()..]
        );
    }

    #[test]
    fn malformed_digest_shapes_are_rejected() {
        for invalid in [
            "9313893f2985088dc3e5b14fdfd4877b0ed37c8dc8d5ef60bd6531ab5678abe1",
            "blake3-256:9313893f",
            "blake3-256:9313893F2985088DC3E5B14FDFD4877B0ED37C8DC8D5EF60BD6531AB5678ABE1",
            "blake3-256:blake3-256:9313893f2985088dc3e5b14fdfd4877b0ed37c8dc8d5ef60bd6531ab5678abe1",
            "blake3-256:9313893g2985088dc3e5b14fdfd4877b0ed37c8dc8d5ef60bd6531ab5678abe1",
        ] {
            assert!(
                Blake3ContentDigest::parse(invalid).is_err(),
                "accepted {invalid}"
            );
        }
    }
}
