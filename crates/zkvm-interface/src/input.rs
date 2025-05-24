use erased_serde::Serialize as ErasedSerialize;
use serde::Serialize;
/// Represents a builder for input data to be passed to a ZKVM guest program.
/// Values are serialized sequentially into an internal byte buffer.
#[derive(Debug, Default)]
pub struct Input {
    buf: Vec<u8>,
    ranges: Vec<(usize, usize)>,
}

pub struct InputErased {
    buf: Vec<Box<dyn ErasedSerialize>>,
}

impl InputErased {
    /// Create an empty input buffer.
    pub fn new() -> Self {
        Self {
            buf: Default::default(),
        }
    }

    pub fn write<T: Serialize + 'static>(&mut self, value: T) -> Result<(), bincode::Error> {
        self.buf.push(Box::new(value));
        Ok(())
    }
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

#[cfg(test)]
mod input_erased_tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Person {
        name: String,
        age: u32,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Product {
        id: u64,
        name: String,
        price: f64,
    }

    #[test]
    fn test_new_creates_empty_buffer() {
        let input = InputErased::new();
        assert_eq!(input.buf.len(), 0);
    }

    #[test]
    fn test_write_primitive_types() {
        let mut input = InputErased::new();

        assert!(input.write(42i32).is_ok());
        assert!(input.write(3.14f64).is_ok());
        assert!(input.write(true).is_ok());
        assert!(input.write("hello".to_string()).is_ok());

        assert_eq!(input.buf.len(), 4);
    }

    #[test]
    fn test_write_custom_structs() {
        let mut input = InputErased::new();

        let person = Person {
            name: "Alice".to_string(),
            age: 30,
        };

        let product = Product {
            id: 123,
            name: "Widget".to_string(),
            price: 9.99,
        };

        assert!(input.write(person).is_ok());
        assert!(input.write(product).is_ok());

        assert_eq!(input.buf.len(), 2);
    }

    #[test]
    fn test_write_collections() {
        let mut input = InputErased::new();

        let vec_data = vec![1, 2, 3, 4, 5];
        let array_data = [10, 20, 30];

        assert!(input.write(vec_data).is_ok());
        assert!(input.write(array_data).is_ok());

        assert_eq!(input.buf.len(), 2);
    }

    #[test]
    fn test_write_mixed_types() {
        let mut input = InputErased::new();

        // Write different types to the same buffer
        assert!(input.write(42).is_ok());
        assert!(input.write("test".to_string()).is_ok());
        assert!(input.write(vec![1, 2, 3]).is_ok());
        assert!(
            input
                .write(Person {
                    name: "Bob".to_string(),
                    age: 25,
                })
                .is_ok()
        );

        assert_eq!(input.buf.len(), 4);
    }

    #[test]
    fn test_serialization_with_erased_serde() {
        let mut input = InputErased::new();

        input.write(42i32).unwrap();
        input.write("hello".to_string()).unwrap();

        // Test that we can serialize the stored items to a buffer
        for item in &input.buf {
            let mut buf = Vec::new();
            let mut serializer = serde_json::Serializer::new(&mut buf);
            let json_result = erased_serde::serialize(item.as_ref(), &mut serializer);
            // Just testing that serialization works without error
            assert!(json_result.is_ok());
        }
    }

    #[test]
    fn test_write_returns_ok() {
        let mut input = InputErased::new();

        // All these should return Ok(())
        let results = vec![
            input.write(1),
            input.write("test".to_string()),
            input.write(vec![1, 2, 3]),
        ];

        for result in results {
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), ());
        }
    }

    #[test]
    fn test_multiple_writes_increase_buffer_size() {
        let mut input = InputErased::new();

        assert_eq!(input.buf.len(), 0);

        input.write(1).unwrap();
        assert_eq!(input.buf.len(), 1);

        input.write(2).unwrap();
        assert_eq!(input.buf.len(), 2);

        input.write(3).unwrap();
        assert_eq!(input.buf.len(), 3);
    }

    // Helper function to demonstrate actual serialization to bytes
    // (since the current implementation doesn't expose this)
    fn serialize_buffer_to_json(
        input: &InputErased,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut results = Vec::new();

        for item in &input.buf {
            let mut buf = Vec::new();
            let mut serializer = serde_json::Serializer::new(&mut buf);
            erased_serde::serialize(item.as_ref(), &mut serializer)?;
            results.push(String::from_utf8(buf)?);
        }

        Ok(results)
    }

    #[test]
    fn test_actual_serialization_output() {
        let mut input = InputErased::new();

        input.write(42).unwrap();
        input.write("hello".to_string()).unwrap();
        input.write(vec![1, 2, 3]).unwrap();

        let serialized = serialize_buffer_to_json(&input).unwrap();

        assert_eq!(serialized.len(), 3);
        assert_eq!(serialized[0], "42");
        assert_eq!(serialized[1], "\"hello\"");
        assert_eq!(serialized[2], "[1,2,3]");
    }
}
