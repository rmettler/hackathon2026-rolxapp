use super::*;
use core::convert::Infallible;

/// Serves exactly 32 known bytes; panics if more are requested.
struct TestRng {
    data: [u8; 32],
    pos: usize,
}

impl TestRng {
    fn new() -> Self {
        let mut data = [0u8; 32];
        for (i, b) in data.iter_mut().enumerate() {
            *b = (i + 1) as u8; // 0x01..=0x20
        }
        Self { data, pos: 0 }
    }
}

impl rand::TryRng for TestRng {
    type Error = Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Infallible> {
        panic!("try_next_u32 not expected in this test");
    }
    fn try_next_u64(&mut self) -> Result<u64, Infallible> {
        panic!("try_next_u64 not expected in this test");
    }
    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Infallible> {
        if self.pos + dst.len() > 32 {
            panic!(
                "more than 32 bytes requested (pos={}, len={})",
                self.pos,
                dst.len()
            );
        }
        dst.copy_from_slice(&self.data[self.pos..self.pos + dst.len()]);
        self.pos += dst.len();
        Ok(())
    }
}

impl rand::TryCryptoRng for TestRng {}

#[test]
fn s256_verifier_and_challenge() {
    let (verifier, challenge) = generate_s256::<32>(&mut TestRng::new());

    // 32 bytes -> base64url no-pad is 43 chars (RFC 7636 minimum verifier length).
    assert_eq!(
        verifier.as_str(),
        "AQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyA"
    );

    // Challenge must be the S256 of the verifier.
    assert_eq!(
        challenge.as_str(),
        "658WgAyQKf_KhWlXY9I8Os5xARz0DpNUrNgQIF4lD4c"
    );
}
