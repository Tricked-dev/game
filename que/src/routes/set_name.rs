use axum::Json;
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine};
use lib_knuckle::{
    api_interfaces::UserUpdate, signature_from_string, verifying_key_from_string,
};

use crate::{pool_extractor::DatabaseConnection, UserCreateError};

pub async fn set_name(
    DatabaseConnection(conn): DatabaseConnection,
    Json(body): Json<UserUpdate>,
) -> Result<String, UserCreateError> {
    let signature = signature_from_string(&body.signature);
    let pub_key = verifying_key_from_string(&body.pub_key);
    if let (Some(signature), Some(pub_key)) = (signature, pub_key) {
        pub_key.verify_strict(body.name.as_bytes(), &signature)?;
        conn.query(
            /* language=postgresql */
            "UPDATE players SET name = $1 WHERE public_key = $2",
            &[&body.name, &STANDARD_NO_PAD.decode(&body.pub_key)?],
        )
        .await?;
        Ok("Ok".to_string())
    } else {
        Err(UserCreateError::InvalidSignature)
    }
}
