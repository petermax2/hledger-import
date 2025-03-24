use std::hash::{DefaultHasher, Hash, Hasher};

pub fn transaction_hash<T: Hash>(prefix: &str, transaction: &T) -> String {
    let mut hasher = DefaultHasher::new();
    transaction.hash(&mut hasher);
    let hash = hasher.finish();
    format!("{prefix}_{hash}")
}
