use erased_serde::Serialize as ErasedSerialize;
use serde::Serialize;

pub enum InputItem {
    /// A serializable object stored as a trait object
    Object(Box<dyn ErasedSerialize>),
    /// Pre-serialized bytes (e.g., from bincode)
    Bytes(Vec<u8>),
}

pub struct InputErased {
    buf: Vec<InputItem>,
}

impl InputErased {
    /// Create an empty input buffer.
    pub fn new() -> Self {
        Self {
            buf: Default::default(),
        }
    }

    /// Write a serializable value as a trait object
    pub fn write<T: Serialize + 'static>(&mut self, value: T) {
        self.buf.push(InputItem::Object(Box::new(value)));
    }

    /// Write pre-serialized bytes directly
    pub fn write_bytes(&mut self, bytes: Vec<u8>) {
        self.buf.push(InputItem::Bytes(bytes));
    }

    /// Get the number of items stored
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Check if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Iterate over the items
    pub fn iter(&self) -> std::slice::Iter<InputItem> {
        self.buf.iter()
    }
}

// Optional: Implement methods to work with the enum
impl InputItem {
    /// Serialize this item to bytes using the specified serializer
    pub fn serialize_with<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            InputItem::Object(obj) => erased_serde::serialize(obj.as_ref(), serializer),
            InputItem::Bytes(bytes) => {
                // Serialize the bytes as a byte array
                bytes.serialize(serializer)
            }
        }
    }

    /// Get the item as bytes (serialize objects, return bytes directly)
    pub fn as_bytes(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        match self {
            InputItem::Object(obj) => {
                let mut buf = Vec::new();
                let mut serializer =
                    bincode::Serializer::new(&mut buf, bincode::DefaultOptions::new());
                erased_serde::serialize(obj.as_ref(), &mut serializer)?;
                Ok(buf)
            }
            InputItem::Bytes(bytes) => Ok(bytes.clone()),
        }
    }
}

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

#[cfg(test)]
mod input_erased_tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Person {
        name: String,
        age: u32,
    }

    #[test]
    fn test_write_object() {
        let mut input = InputErased::new();

        let person = Person {
            name: "Alice".to_string(),
            age: 30,
        };

        input.write(person);
        assert_eq!(input.len(), 1);

        match &input.buf[0] {
            InputItem::Object(_) => (), // Success
            InputItem::Bytes(_) => panic!("Expected Object, got Bytes"),
        }
    }

    #[test]
    fn test_write_bytes() {
        let mut input = InputErased::new();

        let bytes = vec![1, 2, 3, 4, 5];
        input.write_bytes(bytes.clone());

        assert_eq!(input.len(), 1);

        match &input.buf[0] {
            InputItem::Bytes(stored_bytes) => assert_eq!(stored_bytes, &bytes),
            InputItem::Object(_) => panic!("Expected Bytes, got Object"),
        }
    }

    #[test]
    fn test_write_serialized() {
        let mut input = InputErased::new();

        let person = Person {
            name: "Bob".to_string(),
            age: 25,
        };

        // User serializes themselves and writes bytes
        let serialized = bincode::serialize(&person).unwrap();
        input.write_bytes(serialized);

        assert_eq!(input.len(), 1);

        match &input.buf[0] {
            InputItem::Bytes(_) => (), // Success
            InputItem::Object(_) => panic!("Expected Bytes, got Object"),
        }
    }

    #[test]
    fn test_mixed_usage() {
        let mut input = InputErased::new();

        let person = Person {
            name: "Charlie".to_string(),
            age: 35,
        };

        // Mix different write methods
        input.write(42i32); // Object
        let serialized = bincode::serialize(&person).unwrap();
        input.write_bytes(serialized); // Bytes (serialized)
        input.write_bytes(vec![10, 20, 30]); // Bytes (raw)
        input.write("hello".to_string()); // Object

        assert_eq!(input.len(), 4);

        // Verify types
        match &input.buf[0] {
            InputItem::Object(_) => (),
            _ => panic!(),
        }
        match &input.buf[1] {
            InputItem::Bytes(_) => (),
            _ => panic!(),
        }
        match &input.buf[2] {
            InputItem::Bytes(_) => (),
            _ => panic!(),
        }
        match &input.buf[3] {
            InputItem::Object(_) => (),
            _ => panic!(),
        }
    }

    #[test]
    fn test_as_bytes() {
        let mut input = InputErased::new();

        // Add an object
        input.write(42i32);

        // Add raw bytes
        input.write_bytes(vec![1, 2, 3]);

        // Convert both to bytes
        let obj_bytes = input.buf[0].as_bytes().unwrap();
        let raw_bytes = input.buf[1].as_bytes().unwrap();

        // The object should be serialized to some bytes
        assert!(!obj_bytes.is_empty());

        // The raw bytes should be returned as-is
        assert_eq!(raw_bytes, vec![1, 2, 3]);
    }

    #[test]
    fn test_iteration() {
        let mut input = InputErased::new();

        input.write(1);
        input.write(2);
        input.write_bytes(vec![3, 4, 5]);

        let count = input.iter().count();
        assert_eq!(count, 3);
    }
}
