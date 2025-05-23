use serde::Serialize;

/// Represents a builder for input data to be passed to a ZKVM guest program.
/// Values are serialized sequentially into an internal byte buffer.
#[derive(Debug, Default)]
pub struct Input {
    buf: Vec<u8>,
    ranges: Vec<(usize, usize)>,
}

impl Input {
    /// Create an empty input buffer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a value, serializing it with `bincode`.
    pub fn write<T: Serialize>(&mut self, value: &T) -> Result<(), bincode::Error> {
        let start = self.buf.len();
        bincode::serialize_into(&mut self.buf, value)?;
        let end = self.buf.len();
        self.ranges.push((start, end - start));
        Ok(())
    }

    pub fn write_slice(&mut self, slice: &[u8]) {
        let start = self.buf.len();
        self.buf.extend_from_slice(slice);
        let end = self.buf.len();
        self.ranges.push((start, end - start));
    }

    /// Number of elements written.
    pub fn len(&self) -> usize {
        self.ranges.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    /// Entire concatenated payload as one slice.
    pub fn bytes(&self) -> &[u8] {
        &self.buf
    }

    /// Iterator over individual chunks (the originally written objects).
    pub fn chunked_iter(&self) -> impl ExactSizeIterator<Item = &[u8]> + '_ {
        self.ranges.iter().map(|&(s, len)| &self.buf[s..s + len])
    }

    /// Byte‑wise iterator (rarely needed).
    pub fn iter(&self) -> std::slice::Iter<'_, u8> {
        self.buf.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_empty() {
        let input = Input::new();
        assert!(input.is_empty());
        assert_eq!(input.len(), 0);
        assert!(input.bytes().is_empty());
        assert_eq!(input.chunked_iter().count(), 0);
    }

    #[test]
    fn input_write_and_read() {
        let mut input = Input::new();
        let a: u32 = 42;
        let b: &str = "hello";

        input.write(&a).unwrap();
        input.write(&b).unwrap();

        // length bookkeeping
        assert_eq!(input.len(), 2);
        assert!(!input.is_empty());

        // chunk iteration and deserialization
        let chunks: Vec<&[u8]> = input.chunked_iter().collect();
        assert_eq!(chunks.len(), 2);
        let a_back: u32 = bincode::deserialize(chunks[0]).unwrap();
        assert_eq!(a_back, a);
        let b_back: String = bincode::deserialize(chunks[1]).unwrap();
        assert_eq!(b_back, b);

        // contiguous bytes match manual serialization
        let mut expected = Vec::<u8>::new();
        bincode::serialize_into(&mut expected, &a).unwrap();
        bincode::serialize_into(&mut expected, &b).unwrap();
        assert_eq!(input.bytes(), expected.as_slice());

        // iter() covers same length
        assert_eq!(input.iter().count(), expected.len());
    }

    #[test]
    fn input_write_slice() {
        let mut input = Input::new();

        let slice1 = [1, 2, 3, 4];
        let slice2 = [5, 6, 7, 8, 9];

        input.write_slice(&slice1);
        input.write_slice(&slice2);

        assert_eq!(input.len(), 2);
        assert!(!input.is_empty());

        // Check chunked iteration
        let chunks: Vec<&[u8]> = input.chunked_iter().collect();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], &slice1);
        assert_eq!(chunks[1], &slice2);

        // Check contiguous bytes
        let mut expected = Vec::<u8>::new();
        expected.extend_from_slice(&slice1);
        expected.extend_from_slice(&slice2);
        assert_eq!(input.bytes(), expected.as_slice());

        assert_eq!(input.iter().count(), slice1.len() + slice2.len());
    }
}
