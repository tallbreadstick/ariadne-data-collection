use sha2::{Digest, Sha256};

/// One-way participant id: SHA-256(pepper || github_account_id).
/// The raw GitHub id must not be stored or logged after hashing.
pub fn hash_github_account_id(pepper: &str, github_account_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(pepper.as_bytes());
    hasher.update(github_account_id.as_bytes());
    hex::encode(hasher.finalize())
}
