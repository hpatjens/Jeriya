//! Assets are written to files in a custom format that contains a header and the asset content.
//!
//! ## Overview
//!
//! The header contains a magic number, a version number and a file type. The magic number is a
//! 16-byte UUID that is used to identify the file as an asset file. The version number is used
//! to identify the version of the asset file format. This is not the version of the content
//! The file type is a string that is used to identify the type of the asset.
//!
//! Header:
//!
//! | Field            | Type   | Size (bytes) | Description                          |
//! |------------------|--------|--------------|--------------------------------------|
//! | Magic            | u8[16] | 16           | bf432095-05f2-43f6-bf3f-adf8967da67f |
//! | Version          | u32    | 4            | 1 or greater                         |
//! | File type length | u32    | 4            | Length of the file type string       |
//! | File type        | String | variable     | Type of the asset                    |
//!
//! The content of the asset file is written directly after the header. Consider that the
//! header has a variable length.
//!
//! ## Writing
//!
//! To write an asset file, use the [`AssetWrite`] struct. The [`AssetWrite::create`] method
//! is used to create a new asset file. The [`AssetWrite::write_content`] method is used to
//! provide a [`Write`] implementation for the content of the asset file. To extend or modify
//! the content of an existing asset file, use the [`AssetWrite::open`] method.
//!
//! ## Reading
//!
//! To read an asset file, use the [`AssetRead`] struct. The [`AssetRead::open`] method is used
//! to open an existing asset file. The [`AssetRead::read_content`] method is used to provide a
//! [`Read`] implementation for the content of the asset file.

use std::io::{self, Read, Seek, SeekFrom, Write};

use jeriya_shared::byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

/* UUID string: bf432095-05f2-43f6-bf3f-adf8967da67f */
pub const MAGIC: [u8; 16] = [
    0xbf, 0x43, 0x20, 0x95, 0x05, 0xf2, 0x43, 0xf6, 0xbf, 0x3f, 0xad, 0xf8, 0x96, 0x7d, 0xa6, 0x7f,
];

/// Header that is written at the beginning of an asset file.
#[derive(Debug)]
pub struct AssetHeader {
    pub magic: [u8; 16],
    pub version: u32,
    pub file_type: String,
}

impl AssetHeader {
    pub fn read<R: Read>(mut reader: R) -> io::Result<Self> {
        // Read the magic number
        let mut magic = [0u8; 16];
        reader
            .read_exact(&mut magic)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Failed to read magic number"))?;

        // Read the version number
        let version = reader
            .read_u32::<LittleEndian>()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Failed to read version number"))?;

        // Read the file type
        let file_type_len = reader
            .read_u32::<LittleEndian>()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Failed to read file type length"))?;
        let mut file_type_buf = vec![0u8; file_type_len as usize];
        reader
            .read_exact(&mut file_type_buf)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Failed to read file type"))?;
        let file_type =
            String::from_utf8(file_type_buf).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Failed to read file type"))?;

        Ok(Self { magic, version, file_type })
    }

    pub fn write<W: Write>(&self, mut writer: W) -> io::Result<()> {
        writer
            .write(&self.magic)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Failed to write magic number"))?;
        writer
            .write_u32::<LittleEndian>(self.version)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Failed to write version number"))?;
        writer
            .write_u32::<LittleEndian>(self.file_type.len() as u32)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Failed to write file type length"))?;
        writer
            .write(self.file_type.as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Failed to write file type"))?;
        Ok(())
    }

    /// Checks if the header is valid.
    pub fn check(&self) -> io::Result<()> {
        if self.magic != MAGIC {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid magic number"));
        }
        if self.version != 1 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid version number"));
        }
        Ok(())
    }
}

pub struct AssetWrite<W: Write> {
    writer: W,
    content_start_position: u64,
}

impl<W: Write + Seek> AssetWrite<W> {
    /// Creates a new asset file by initializing the header and providing a [`Write`] implementation for the content by calling `AssetWrite::write_content`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::io::{Cursor, Write, Read};
    /// use jeriya_content::asset_file::{AssetWrite, AssetHeader};
    ///
    /// let mut buf = Vec::new();
    /// let mut cursor = Cursor::new(&mut buf);
    /// let _writer = AssetWrite::create(&mut cursor, "MyFileType").unwrap();
    ///
    /// let mut cursor = Cursor::new(&mut buf);
    /// let header = AssetHeader::read(&mut cursor).unwrap();
    /// assert!(header.check().is_ok());
    /// ```
    pub fn create(mut writer: W, file_type: impl Into<String>) -> io::Result<Self> {
        let header = AssetHeader {
            magic: MAGIC,
            version: 1,
            file_type: file_type.into(),
        };
        header.write(&mut writer)?;
        let content_start_position = writer
            .seek(SeekFrom::Current(0))
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Failed to read content start position"))?;
        Ok(Self {
            writer,
            content_start_position,
        })
    }

    /// Provides a [`Write`] implementation for the content of the asset file.
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::io::{Cursor, Write, Read};
    /// use jeriya_content::asset_file::{AssetWrite, AssetHeader};
    ///
    /// let mut buf = Vec::new();
    /// let mut cursor = Cursor::new(&mut buf);
    /// let mut writer = AssetWrite::create(&mut cursor, "MyFileType").unwrap();
    /// writer.write_content().write(b"Hello, World!").unwrap();
    ///
    /// let mut cursor = Cursor::new(&mut buf);
    /// assert!(AssetHeader::read(&mut cursor).is_ok());
    /// let mut content = String::new();
    /// cursor.read_to_string(&mut content).unwrap();
    /// assert_eq!(content, "Hello, World!");
    /// ```
    pub fn write_content(&mut self) -> ContentWrite<W> {
        let content_start_position = self.content_start_position;
        ContentWrite {
            asset_write: self,
            content_start_position,
        }
    }
}

impl<S: Write + Read + Seek> AssetWrite<S> {
    /// Open an existing asset file by reading the header and providing a [`Write`] implementation for the content.
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::io::{Cursor, Write, Read};
    /// use jeriya_content::asset_file::{AssetWrite, AssetHeader};
    ///
    /// let mut buf = Vec::new();
    /// let mut cursor = Cursor::new(&mut buf);
    /// let mut writer = AssetWrite::create(&mut cursor, "MyFileType").unwrap();
    /// writer.write_content().write(b"Hello, world!").unwrap();
    ///
    /// let mut cursor = Cursor::new(&mut buf);
    /// let mut writer = AssetWrite::open(&mut cursor).unwrap();
    /// writer.write_content().write(b"Hello, Universe!").unwrap();
    ///
    /// let mut cursor = Cursor::new(&mut buf);
    /// assert!(AssetHeader::read(&mut cursor).is_ok());
    /// let mut content = String::new();
    /// cursor.read_to_string(&mut content).unwrap();
    /// assert_eq!(content, "Hello, Universe!");
    /// ```
    pub fn open(mut writer: S) -> io::Result<Self> {
        let header = AssetHeader::read(&mut writer)?;
        header.check()?;
        let content_start_position = writer
            .seek(SeekFrom::Current(0))
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Failed to read content start position"))?;
        Ok(Self {
            writer,
            content_start_position,
        })
    }
}

pub struct ContentWrite<'a, W: Write + Seek> {
    asset_write: &'a mut AssetWrite<W>,
    content_start_position: u64,
}

impl<'a, W: Write + Seek> Write for ContentWrite<'a, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.asset_write.writer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.asset_write.writer.flush()
    }
}

impl<'a, W: Write + Seek> Seek for ContentWrite<'a, W> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        seek(&mut self.asset_write.writer, pos, self.content_start_position)
    }
}

pub struct AssetRead<R: Read + Seek> {
    reader: R,
    content_start_position: u64,
}

impl<R: Read + Seek> AssetRead<R> {
    /// Creates a new asset file by reading the header and providing a [`Read`] implementation for the content by calling `AssetRead::content_read`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::io::{Cursor, Write, Read};
    /// use jeriya_content::asset_file::{AssetWrite, AssetRead, AssetHeader};
    ///
    /// let mut buf = Vec::new();
    /// let mut cursor = Cursor::new(&mut buf);
    /// let mut writer = AssetWrite::create(&mut cursor, "MyFileType").unwrap();
    /// writer.write_content().write(b"Hello, World!").unwrap();
    ///
    /// let mut cursor = Cursor::new(&mut buf);
    /// let mut reader = AssetRead::open(&mut cursor).unwrap();
    /// let mut content = String::new();
    /// reader.read_content().read_to_string(&mut content).unwrap();
    /// assert_eq!(content, "Hello, World!");
    /// ```
    pub fn open(mut reader: R) -> io::Result<Self> {
        // Read the header
        let header = AssetHeader::read(&mut reader)?;
        header.check()?;

        // Content will start at this position
        let content_start_position = reader
            .seek(SeekFrom::Current(0))
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Failed to read content start position"))?;

        Ok(Self {
            reader,
            content_start_position,
        })
    }

    /// Provides a [`Read`] implementation for the content of the asset file.
    pub fn read_content(&mut self) -> ContentRead<R> {
        let content_start_position = self.content_start_position;
        ContentRead {
            asset_read: self,
            content_start_position,
        }
    }
}

pub struct ContentRead<'a, R: Read + Seek> {
    asset_read: &'a mut AssetRead<R>,
    content_start_position: u64,
}

impl<'a, R: Read + Seek> Read for ContentRead<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.asset_read.reader.read(buf)
    }
}

impl<'a, R: Read + Seek> Seek for ContentRead<'a, R> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        seek(&mut self.asset_read.reader, pos, self.content_start_position)
    }
}

fn seek<S: Seek>(seekable: &mut S, pos: SeekFrom, content_start_position: u64) -> io::Result<u64> {
    match pos {
        SeekFrom::Start(offset) => seekable
            .seek(SeekFrom::Start(content_start_position + offset))
            .map(|pos| pos - content_start_position),
        SeekFrom::End(offset) => {
            let end_position = seekable.seek(SeekFrom::End(0))?;
            if end_position as i64 + offset < content_start_position as i64 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Seeking before the start of the content",
                ));
            }
            seekable.seek(SeekFrom::End(offset)).map(|pos| pos - content_start_position)
        }
        SeekFrom::Current(offset) => {
            let current_position = seekable.seek(SeekFrom::Current(0))?;
            if current_position as i64 + offset < content_start_position as i64 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Seeking before the start of the content",
                ));
            }
            seekable.seek(SeekFrom::Current(offset)).map(|pos| pos - content_start_position)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, SeekFrom};

    use super::*;

    fn assert_magic<R: Read>(mut reader: R) {
        let mut actual_magic = [0u8; 16];
        reader.read_exact(&mut actual_magic).unwrap();
        assert_eq!(actual_magic, MAGIC);
    }

    fn assert_version<R: Read>(mut reader: R, expected_version: u32) {
        let actual_version = reader.read_u32::<LittleEndian>().unwrap();
        assert_eq!(actual_version, expected_version);
    }

    fn assert_file_type<R: Read>(mut reader: R, expected_file_type: &str) {
        let file_type_len = reader.read_u32::<LittleEndian>().unwrap();
        let mut actual_file_type = vec![0u8; file_type_len as usize];
        reader.read_exact(&mut actual_file_type).unwrap();
        let actual_file_type = String::from_utf8(actual_file_type).unwrap();
        assert_eq!(actual_file_type, expected_file_type);
    }

    fn assert_header<R: Read>(mut reader: R, expected_file_type: &str) {
        assert_magic(&mut reader);
        assert_version(&mut reader, 1);
        assert_file_type(&mut reader, expected_file_type);
    }

    fn assert_string<R: Read>(mut reader: R, expected: &str) {
        let mut actual = String::new();
        reader.read_to_string(&mut actual).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    #[should_panic]
    fn seek_negative_from_end_into_header() {
        // Given
        let mut buf = Vec::new();
        let mut cursor = io::Cursor::new(&mut buf);
        let dummy_header = [0x01, 0x02, 0x03, 0x04];
        cursor.write(&dummy_header).unwrap();

        // When
        seek(&mut cursor, SeekFrom::End(-1), dummy_header.len() as u64).unwrap();
    }

    #[test]
    #[should_panic]
    fn seek_negative_from_current_into_header() {
        // Given
        let mut buf = Vec::new();
        let mut cursor = io::Cursor::new(&mut buf);
        let dummy_header = [0x01, 0x02, 0x03, 0x04];
        cursor.write(&dummy_header).unwrap();

        // When
        seek(&mut cursor, SeekFrom::Current(-1), dummy_header.len() as u64).unwrap();
    }

    mod asset_write {
        use super::*;

        #[test]
        fn create() {
            // Given
            let mut buf = Vec::new();
            let mut cursor = Cursor::new(&mut buf);
            let file_type = String::from("MyFileType");

            // When
            let mut writer = AssetWrite::create(&mut cursor, &file_type).unwrap();
            writer.write_content().write(b"Hello, world!").unwrap();

            // Then
            let mut reader = Cursor::new(&buf);
            assert_header(&mut reader, &file_type);
            assert_string(&mut reader, "Hello, world!");
        }

        #[test]
        fn open() {
            // Given
            let mut buf = Vec::new();
            let mut cursor = Cursor::new(&mut buf);
            let file_type = String::from("MyFileType");
            let mut writer = AssetWrite::create(&mut cursor, &file_type).unwrap();
            writer.write_content().write(b"Hello, world!").unwrap();

            // When
            let mut cursor = Cursor::new(&mut buf);
            let mut writer = AssetWrite::open(&mut cursor).unwrap();
            writer.write_content().write(b"Hello, Universe!").unwrap();

            // Then
            let mut reader = Cursor::new(&buf);
            assert_header(&mut reader, &file_type);
            assert_string(&mut reader, "Hello, Universe!");
        }

        #[test]
        fn seek_to_start() {
            // Given
            let mut buf = Vec::new();
            let mut cursor = Cursor::new(&mut buf);
            let file_type = String::from("MyFileType");

            // When
            let mut writer = AssetWrite::create(&mut cursor, &file_type).unwrap();
            let mut content_writer = writer.write_content();
            content_writer.write(b"Hello, world!").unwrap();
            content_writer.seek(SeekFrom::Start(0)).unwrap();
            content_writer.write(b"Hello, Universe!").unwrap();

            // Then
            let mut reader = Cursor::new(&buf);
            assert_header(&mut reader, &file_type);
            assert_string(&mut reader, "Hello, Universe!");
        }

        #[test]
        fn stream_position() {
            // Given
            let mut buf = Vec::new();
            let mut cursor = Cursor::new(&mut buf);
            let file_type = String::from("MyFileType");

            // When
            let mut writer = AssetWrite::create(&mut cursor, &file_type).unwrap();
            let mut content_writer = writer.write_content();
            let content = b"Hello, world!";
            content_writer.write(content).unwrap();
            let actual = content_writer.seek(SeekFrom::Current(0)).unwrap();

            // Then
            assert_eq!(actual, content.len() as u64);
        }
    }

    mod asset_read {
        use super::*;

        #[test]
        fn read() {
            // Given
            let mut buf = Vec::new();
            let mut cursor = Cursor::new(&mut buf);
            let file_type = String::from("MyFileType");
            let mut writer = AssetWrite::create(&mut cursor, file_type).unwrap();
            writer.write_content().write(b"Hello, world!").unwrap();

            // When
            let mut cursor = Cursor::new(&mut buf);
            let mut reader = AssetRead::open(&mut cursor).unwrap();
            let mut content_reader = reader.read_content();
            let mut actual = String::new();
            content_reader.read_to_string(&mut actual).unwrap();

            // Then
            assert_eq!(actual, "Hello, world!");
        }

        #[test]
        fn seek_to_start() {
            // Given
            let mut buf = Vec::new();
            let mut cursor = Cursor::new(&mut buf);
            let file_type = String::from("MyFileType");
            let mut writer = AssetWrite::create(&mut cursor, file_type).unwrap();
            writer.write_content().write(b"Hello, world!").unwrap();

            // When
            let mut cursor = Cursor::new(&mut buf);
            let mut reader = AssetRead::open(&mut cursor).unwrap();
            let mut content_reader = reader.read_content();
            content_reader.seek(SeekFrom::Start(0)).unwrap();
            let mut actual = String::new();
            content_reader.read_to_string(&mut actual).unwrap();

            // Then
            assert_eq!(actual, "Hello, world!");
        }
    }
}
