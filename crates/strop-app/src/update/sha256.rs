use std::io::{self, Read};

const K: [u32; 64] = [
    0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
    0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
    0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
    0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
    0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
    0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
    0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
    0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2,
];

/// Streaming: fixed 8 KiB read buffer, 64-byte block compression as bytes
/// arrive. Memory use is constant no matter the input size — hashing a
/// 256 MiB artifact must never allocate a second copy of it.
pub fn reader(mut input: impl Read) -> io::Result<(String, u64)> {
    let mut h = [0x6a09e667u32,0xbb67ae85,0x3c6ef372,0xa54ff53a,
        0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19];
    let mut len: u64 = 0;
    let mut block = [0u8; 64];
    let mut filled = 0usize;
    let mut buf = [0u8; 8192];
    loop {
        let n = input.read(&mut buf)?;
        if n == 0 { break; }
        len += n as u64;
        let mut rest = &buf[..n];
        while !rest.is_empty() {
            let take = (64 - filled).min(rest.len());
            block[filled..filled + take].copy_from_slice(&rest[..take]);
            filled += take;
            rest = &rest[take..];
            if filled == 64 { compress(&mut h, &block); filled = 0; }
        }
    }
    block[filled] = 0x80;
    filled += 1;
    if filled > 56 {
        block[filled..].fill(0);
        compress(&mut h, &block);
        filled = 0;
    }
    block[filled..56].fill(0);
    block[56..].copy_from_slice(&(len * 8).to_be_bytes());
    compress(&mut h, &block);
    Ok((h.iter().map(|x| format!("{x:08x}")).collect(), len))
}

fn compress(h: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    for (i, bytes) in block.chunks_exact(4).enumerate() {
        w[i] = u32::from_be_bytes(bytes.try_into().unwrap());
    }
    for i in 16..64 {
        let s0 = w[i-15].rotate_right(7) ^ w[i-15].rotate_right(18) ^ (w[i-15] >> 3);
        let s1 = w[i-2].rotate_right(17) ^ w[i-2].rotate_right(19) ^ (w[i-2] >> 10);
        w[i] = w[i-16].wrapping_add(s0).wrapping_add(w[i-7]).wrapping_add(s1);
    }
    let [mut a,mut b,mut c,mut d,mut e,mut f,mut g,mut hh] = *h;
    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ (!e & g);
        let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        hh=g; g=f; f=e; e=d.wrapping_add(t1); d=c; c=b; b=a; a=t1.wrapping_add(t2);
    }
    for (v, x) in h.iter_mut().zip([a,b,c,d,e,f,g,hh]) { *v = v.wrapping_add(x); }
}

#[cfg(test)]
mod tests {
    #[test]
    fn known_vector() {
        let (hash, size) = super::reader(&b"abc"[..]).unwrap();
        assert_eq!(size, 3);
        assert_eq!(hash, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn padding_edges_and_multi_block_streams() {
        // Every padding shape: empty, length-fits (55/56), block boundary
        // (63/64/65), and an input spanning many read-buffer refills.
        let cases: &[(usize, &str)] = &[
            (0, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"),
            (55, "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318"),
            (56, "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a"),
            (63, "7d3e74a05d7db15bce4ad9ec0658ea98e3f06eeecf16b4c6fff2da457ddc2f34"),
            (64, "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb"),
            (65, "635361c48bb9eab14198e76ea8ab7f1a41685d6ad62aa9146d301d4f17eb0ae0"),
            (10000, "27dd1f61b867b6a0f6e9d8a41c43231de52107e53ae424de8f847b821db4b711"),
        ];
        for &(n, expected) in cases {
            let (hash, size) = super::reader(&vec![b'a'; n][..]).unwrap();
            assert_eq!(size, n as u64);
            assert_eq!(hash, expected, "length {n}");
        }
    }

    /// A reader that trickles one byte per read, so block assembly must
    /// survive arbitrary fragmentation.
    struct Trickle<'a>(&'a [u8]);
    impl std::io::Read for Trickle<'_> {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            let Some((first, rest)) = self.0.split_first() else { return Ok(0) };
            out[0] = *first;
            self.0 = rest;
            Ok(1)
        }
    }

    #[test]
    fn fragmented_reads_hash_identically() {
        let data = vec![b'a'; 65];
        let (hash, size) = super::reader(Trickle(&data)).unwrap();
        assert_eq!(size, 65);
        assert_eq!(hash, "635361c48bb9eab14198e76ea8ab7f1a41685d6ad62aa9146d301d4f17eb0ae0");
    }
}
