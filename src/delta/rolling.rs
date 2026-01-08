/// Adler-32 Rolling Checksum implementation
/// 
/// Rsync uses a slightly modified Adler-32 where offsets are calculated differently 
/// for efficiency in rolling windows.
/// 
/// A = 1 + s[i] + ... + s[i+w-1]
/// B = w + w*s[i] + (w-1)*s[i+1] + ... + 1*s[i+w-1]
/// 
/// Checksum = (B << 16) | A
pub struct RollingChecksum {
    a: u32,
    b: u32,
    window_size: usize,
}

impl RollingChecksum {
    pub fn new() -> Self {
        Self { a: 0, b: 0, window_size: 0 }
    }

    /// Reset state
    pub fn reset(&mut self) {
        self.a = 0;
        self.b = 0;
        self.window_size = 0;
    }

    /// Initialize with a data block
    pub fn update(&mut self, data: &[u8]) {
        self.a = 0;
        self.b = 0;
        self.window_size = data.len();
        
        // Rsync's adler32 usually starts with a=0, b=0 based on tridge's paper,
        // but zlib adler32 starts with a=1. 
        // We follow rsync implementation logic:
        // s1 = 0; s2 = 0;
        // for i in 0..len: s1 += data[i]; s2 += s1;
        // But to make it "rolling" safely, let's just do the math.
        
        for (i, &byte) in data.iter().enumerate() {
            let val = byte as u32;
            self.a = self.a.wrapping_add(val);
            self.b = self.b.wrapping_add((data.len() - i) as u32 * val);
        }
    }

    /// Roll the window: remove `old_byte`, add `new_byte`
    /// 
    /// New A = Old A - old_byte + new_byte
    /// New B = Old B - (window_size * old_byte) + New A
    #[inline]
    pub fn roll(&mut self, old_byte: u8, new_byte: u8) {
        let old_val = old_byte as u32;
        let new_val = new_byte as u32;
        
        self.a = self.a.wrapping_sub(old_val).wrapping_add(new_val);
        self.b = self.b.wrapping_sub(self.window_size as u32 * old_val).wrapping_add(self.a);
    }

    pub fn digest(&self) -> u32 {
        ((self.b & 0xffff) << 16) | (self.a & 0xffff)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rolling() {
        let data = b"abcdefgh";
        
        // Initial: "abcd"
        let mut rc = RollingChecksum::new();
        rc.update(&data[0..4]);
        
        // Expected sum for "abcd":
        // a = 97+98+99+100 = 394
        // b = 4*97 + 3*98 + 2*99 + 1*100 = 388+294+198+100 = 980
        // digest = (980 << 16) | 394
        assert_eq!(rc.a, 394);
        assert_eq!(rc.b, 980);
        
        // Roll: remove 'a', add 'e' -> "bcde"
        rc.roll(b'a', b'e'); // 'a'=97, 'e'=101
        
        // Calculate manually for "bcde"
        // a = 98+99+100+101 = 398 (394 - 97 + 101) -> Correct
        // b = 4*98 + 3*99 + 2*100 + 1*101 = 392+297+200+101 = 990
        
        // Formula check:
        // New B = Old B - 4*old + New A 
        // 980 - 4*97 + 398 = 980 - 388 + 398 = 990 -> Correct
        
        assert_eq!(rc.a, 398);
        assert_eq!(rc.b, 990);
        
        // Verify against fresh calculation
        let mut rc2 = RollingChecksum::new();
        rc2.update(&data[1..5]); // "bcde"
        assert_eq!(rc.digest(), rc2.digest());
    }
}
