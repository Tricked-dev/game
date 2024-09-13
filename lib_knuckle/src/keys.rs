use ed25519_dalek::{SigningKey, VerifyingKey};

pub enum Keys {
    VerifyOnly {
        my_keys: VerifyingKey,
        other_keys: VerifyingKey,
    },
    Sign {
        my_keys: SigningKey,
        other_keys: VerifyingKey,
    },
}

impl Keys {
    pub fn my_sign(&mut self) -> Option<&mut SigningKey> {
        match self {
            Keys::VerifyOnly { .. } => None,
            Keys::Sign { my_keys, .. } => Some(my_keys),
        }
    }
    pub fn my_verify(&self) -> &VerifyingKey {
        match self {
            Keys::VerifyOnly { my_keys, .. } => my_keys,
            Keys::Sign { my_keys, .. } => my_keys.as_ref(),
        }
    }
    pub fn other_verify(&self) -> &VerifyingKey {
        match self {
            Keys::VerifyOnly { other_keys, .. } => other_keys,
            Keys::Sign { other_keys, .. } => other_keys,
        }
    }
}
