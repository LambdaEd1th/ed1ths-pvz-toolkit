use std::sync::Arc;

use crypt_data::{decrypt_wrapped, encrypt_with_limit};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TransformMode {
    Encrypt,
    Decrypt,
}

fn transform_owned(
    data: &[u8],
    mode: TransformMode,
    key: &[u8],
    limit: usize,
) -> Result<Vec<u8>, String> {
    match mode {
        TransformMode::Encrypt => encrypt_with_limit(data, key, limit),
        TransformMode::Decrypt => decrypt_wrapped(data, key, limit),
    }
    .map_err(|error| error.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn transform(
    data: Arc<[u8]>,
    mode: TransformMode,
    key: Vec<u8>,
    limit: usize,
) -> Result<Vec<u8>, String> {
    let (sender, receiver) = futures_channel::oneshot::channel();
    rayon::spawn(move || {
        let _ = sender.send(transform_owned(&data, mode, &key, limit));
    });
    receiver
        .await
        .map_err(|_| "Crypt-Data 处理任务意外终止".to_string())?
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn transform(
    data: Arc<[u8]>,
    mode: TransformMode,
    key: Vec<u8>,
    limit: usize,
) -> Result<Vec<u8>, String> {
    transform_owned(&data, mode, &key, limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypt_data::{DEFAULT_KEY, DEFAULT_LIMIT, MAGIC};

    #[test]
    fn raw_and_wrapped_documents_roundtrip() {
        let raw = vec![0x5a; 600];
        let wrapped = transform_owned(&raw, TransformMode::Encrypt, DEFAULT_KEY, DEFAULT_LIMIT)
            .expect("encrypt");
        assert!(wrapped.starts_with(&MAGIC));
        assert_eq!(
            transform_owned(&wrapped, TransformMode::Decrypt, DEFAULT_KEY, DEFAULT_LIMIT,)
                .expect("decrypt"),
            raw
        );
    }
}
