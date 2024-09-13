use base64::{prelude::BASE64_STANDARD_NO_PAD, Engine};
use ed25519::Signature;
use ed25519_dalek::{SigningKey, VerifyingKey};

pub fn signing_key_from_string<T: AsRef<str>>(key: T) -> Option<SigningKey> {
    Some(SigningKey::from_bytes(
        &BASE64_STANDARD_NO_PAD
            .decode(key.as_ref())
            .ok()?
            .try_into()
            .ok()?,
    ))
}
pub fn signature_from_string(key: &str) -> Option<Signature> {
    Some(Signature::from_bytes(
        &BASE64_STANDARD_NO_PAD.decode(key).ok()?.try_into().ok()?,
    ))
}
pub fn verifying_key_from_string(key: &str) -> Option<VerifyingKey> {
    VerifyingKey::from_bytes(&BASE64_STANDARD_NO_PAD.decode(key).ok()?.try_into().ok()?)
        .ok()
}
