//! Local streaming SHA-256 through Windows CNG. Keep executable validation and
//! high-resolution texture hashing fast in development builds as well as release.
use hudhook::windows::Win32::Security::Cryptography::*;

pub struct Sha256 {
    algorithm: BCRYPT_ALG_HANDLE,
    hash: BCRYPT_HASH_HANDLE,
}
impl Sha256 {
    pub fn new() -> Result<Self, String> {
        let mut value = Self {
            algorithm: BCRYPT_ALG_HANDLE::default(),
            hash: BCRYPT_HASH_HANDLE::default(),
        };
        // CNG allocates the hash object when its buffer is None; the hash guard
        // destroys it before closing the algorithm provider. No keys are used.
        unsafe {
            BCryptOpenAlgorithmProvider(
                &mut value.algorithm,
                BCRYPT_SHA256_ALGORITHM,
                None,
                BCRYPT_OPEN_ALGORITHM_PROVIDER_FLAGS(0),
            )
        }
        .ok()
        .map_err(|e| format!("open SHA-256 provider: {e}"))?;
        unsafe { BCryptCreateHash(value.algorithm, &mut value.hash, None, None, 0) }
            .ok()
            .map_err(|e| format!("create SHA-256 hash: {e}"))?;
        Ok(value)
    }
    pub fn update(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > u32::MAX as usize {
            return Err("SHA-256 input chunk exceeds ULONG".into());
        }
        unsafe { BCryptHashData(self.hash, bytes, 0) }
            .ok()
            .map_err(|e| format!("hash SHA-256 input: {e}"))
    }
    pub fn finish(self) -> Result<[u8; 32], String> {
        let mut output = [0u8; 32];
        unsafe { BCryptFinishHash(self.hash, &mut output, 0) }
            .ok()
            .map_err(|e| format!("finish SHA-256 hash: {e}"))?;
        Ok(output)
    }
}
impl Drop for Sha256 {
    fn drop(&mut self) {
        unsafe {
            if !self.hash.0.is_null() {
                let _ = BCryptDestroyHash(self.hash);
            }
            if !self.algorithm.0.is_null() {
                let _ = BCryptCloseAlgorithmProvider(self.algorithm, 0);
            }
        }
    }
}
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|v| format!("{v:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sha256_matches_standard_vectors_and_streamed_multiblock_input() {
        let mut hash = Sha256::new().unwrap();
        hash.update(b"a").unwrap();
        hash.update(b"bc").unwrap();
        assert_eq!(
            hex(&hash.finish().unwrap()),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        use sha2::Digest;
        let data = vec![0x61u8; 65537];
        let reference = sha2::Sha256::digest(&data);
        let mut hash = Sha256::new().unwrap();
        for chunk in data.chunks(1000) {
            hash.update(chunk).unwrap();
        }
        assert_eq!(hash.finish().unwrap().as_slice(), reference.as_slice());
    }
}
