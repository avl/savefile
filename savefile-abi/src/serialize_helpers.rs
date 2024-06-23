use std::io::{Read, Write};
use savefile::{Deserializer, Serializer};

/// Trait for something that can provide serializers and deserializers.
/// For example a TcpStream, or a rustls stream.
/// This is typically not useful with a file, since it's unlikely someone ever wants
/// to deserialize and serialize from/to the same file at the same time.
pub trait SerializerAndDeserializer {
    /// The type of the underlying reader
    type TR: Read;
    /// The type of the underlying writer (may be same as reader)
    type TW: Write;
    /// Get a serializer, backed by the underlying stream
    fn get_serializer(&mut self) -> Serializer<Self::TW>;
    /// Get a deserializer, backed by the underlying stream.
    /// The ephemeral state will be empty.
    fn get_deserializer(&mut self) -> Deserializer<Self::TR>;

    /// Get access to the underlying writer. Use with caution!
    fn raw_writer(&mut self) -> &mut Self::TW;

    /// Get access to the underlying reader. Use with caution!
    fn raw_reader(&mut self) -> &mut Self::TR;

    /// Called on the client side when the client is being shut down
    fn notify_close(&mut self);
}


/// Object which contains enough information to create a serializer or deserializer.
/// This is useful when a single stream, implementing both Read and Write, is available,
/// and this stream cannot be split into two independent uni-directional streams.
/// An example is a rustls TLS-connection.
pub struct CombinedSerializerAndDeserializer<T: Read + Write> {
    reader_writer: T,
    serializer_file_version: u32,
    deserializer_file_version: u32,
}

/// Represents a SerializerAndDeserializer comprised of separate write and read streams.
pub struct SeparateSerializerAndDeserializer<TR: Read, TW: Write> {
    reader: TR,
    writer: TW,
    serializer_file_version: u32,
    deserializer_file_version: u32,
}
impl<'a, TR: Read, TW: Write> SeparateSerializerAndDeserializer<TR, TW> {
    /// Given a reader+writer, initialize a new SerializerAndDeserializer.
    /// The serializer_file_version and deserializer_file_version will typically
    /// have the same value, but this is not necessary.
    pub fn new(reader: TR, writer: TW, serializer_file_version: u32, deserializer_file_version: u32) -> SeparateSerializerAndDeserializer<TR, TW> {
        SeparateSerializerAndDeserializer {
            reader,
            writer,
            serializer_file_version,
            deserializer_file_version,
        }
    }
}
impl<TR: Read, TW: Write> SerializerAndDeserializer for SeparateSerializerAndDeserializer<TR, TW> {
    type TR = TR;
    type TW = TW;
    fn get_serializer(&mut self) -> Serializer<TW> {
        Serializer {
            writer: &mut self.writer,
            file_version: self.serializer_file_version,
        }
    }

    fn get_deserializer(&mut self) -> Deserializer<TR> {
        Deserializer {
            reader: &mut self.reader,
            file_version: self.deserializer_file_version,
            ephemeral_state: Default::default(),
        }
    }

    fn raw_writer(&mut self) -> &mut TW {
        &mut self.writer
    }

    fn raw_reader(&mut self) -> &mut Self::TR {
        &mut self.reader
    }

    fn notify_close(&mut self) {
        _ = self.writer.flush()
    }
}

impl<T: Read + Write> CombinedSerializerAndDeserializer<T> {
    /// Given a reader+writer, initialize a new SerializerAndDeserializer.
    /// The serializer_file_version and deserializer_file_version will typically
    /// have the same value, but this is not necessary.
    pub fn new(reader_writer: T, serializer_file_version: u32, deserializer_file_version: u32) -> CombinedSerializerAndDeserializer<T> {
        CombinedSerializerAndDeserializer {
            reader_writer,
            serializer_file_version,
            deserializer_file_version,
        }
    }
}
impl<T: Read + Write> SerializerAndDeserializer for CombinedSerializerAndDeserializer<T> {
    type TR = T;
    type TW = T;
    /// Construct a serializer. Note, the returned value is not meant to be long-lived.
    /// It will not be possible to call 'get_deserializer' until the returned value
    /// has been dropped.
    fn get_serializer(&mut self) -> Serializer<T> {
        Serializer {
            writer: &mut self.reader_writer,
            file_version: self.serializer_file_version,
        }
    }
    /// Construct a deserializer. Note, the returned value is not meant to be long-lived.
    /// It will not be possible to call 'get_serializer' until the returned value
    /// has been dropped.
    fn get_deserializer(&mut self) -> Deserializer<T> {
        Deserializer {
            reader: &mut self.reader_writer,
            file_version: self.deserializer_file_version,
            ephemeral_state: Default::default(),
        }
    }

    fn raw_writer(&mut self) -> &mut T {
        &mut self.reader_writer
    }

    fn raw_reader(&mut self) -> &mut Self::TR {
        &mut self.reader_writer
    }

    fn notify_close(&mut self) {
        _  = self.reader_writer.flush()
    }
}
