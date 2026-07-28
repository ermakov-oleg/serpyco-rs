use smallvec::SmallVec;

const ARRAY32: u8 = 0xdd;
const MAP32: u8 = 0xdf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContainerKind {
    Array,
    Map,
}

#[derive(Debug, Clone, Copy)]
struct Container {
    kind: ContainerKind,
    header_pos: usize,
    items: u32,
}

/// Streaming MessagePack writer.
///
/// Container lengths are not known when the shared writer API opens them, so
/// arrays/maps use the valid 32-bit header form and backpatch their item count
/// when closed.
#[derive(Debug)]
pub(crate) struct MsgpackWriter {
    buf: Vec<u8>,
    containers: SmallVec<[Container; 8]>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Checkpoint {
    buf_len: usize,
    containers_len: usize,
}

pub(crate) fn encode_map_key(key: &str) -> Box<[u8]> {
    let mut buf = Vec::with_capacity(key.len() + 5);
    write_str_to(&mut buf, key);
    buf.into_boxed_slice()
}

impl MsgpackWriter {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: Vec::with_capacity(capacity),
            containers: SmallVec::new(),
        }
    }

    #[inline]
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    #[inline(always)]
    pub(crate) fn position(&self) -> usize {
        self.buf.len()
    }

    #[inline(always)]
    pub(crate) fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            buf_len: self.buf.len(),
            containers_len: self.containers.len(),
        }
    }

    #[inline(always)]
    pub(crate) fn rollback(&mut self, cp: Checkpoint) {
        self.buf.truncate(cp.buf_len);
        self.containers.truncate(cp.containers_len);
    }

    #[inline(always)]
    pub(crate) fn tail_is_null(&self, from: usize) -> bool {
        self.buf[from..] == [0xc0]
    }

    #[inline]
    pub(crate) fn item_end(&mut self) {
        let container = self
            .containers
            .last_mut()
            .expect("item_end must be called inside a container");
        container.items = container
            .items
            .checked_add(1)
            .expect("MessagePack container cannot exceed u32::MAX items");
    }

    #[inline]
    pub(crate) fn write_null(&mut self) {
        self.buf.push(0xc0);
    }

    #[inline]
    pub(crate) fn write_bool(&mut self, value: bool) {
        self.buf.push(if value { 0xc3 } else { 0xc2 });
    }

    #[inline]
    pub(crate) fn write_i64(&mut self, value: i64) {
        if value >= 0 {
            write_u64_to(&mut self.buf, value as u64);
        } else if value >= -32 {
            self.buf.push(value as i8 as u8);
        } else if value >= i8::MIN as i64 {
            self.buf.extend_from_slice(&[0xd0, value as i8 as u8]);
        } else if value >= i16::MIN as i64 {
            self.buf.push(0xd1);
            self.buf.extend_from_slice(&(value as i16).to_be_bytes());
        } else if value >= i32::MIN as i64 {
            self.buf.push(0xd2);
            self.buf.extend_from_slice(&(value as i32).to_be_bytes());
        } else {
            self.buf.push(0xd3);
            self.buf.extend_from_slice(&value.to_be_bytes());
        }
    }

    #[inline]
    pub(crate) fn write_big_int(&mut self, value: &str) -> Result<(), &'static str> {
        match value.parse::<u64>() {
            Ok(value) => {
                write_u64_to(&mut self.buf, value);
                Ok(())
            }
            Err(_) => Err("integer is out of range for MessagePack"),
        }
    }

    #[inline]
    pub(crate) fn write_f64(&mut self, value: f64) -> Result<(), &'static str> {
        self.buf.push(0xcb);
        self.buf.extend_from_slice(&value.to_be_bytes());
        Ok(())
    }

    #[inline]
    pub(crate) fn write_str(&mut self, value: &str) {
        write_str_to(&mut self.buf, value);
    }

    #[inline]
    pub(crate) fn write_bytes(&mut self, value: &[u8]) {
        write_len_prefixed(
            &mut self.buf,
            value,
            0xc4,
            0xc5,
            0xc6,
            "MessagePack binary value is too large",
        );
    }

    #[inline]
    pub(crate) fn begin_map(&mut self) {
        self.begin_container(ContainerKind::Map, MAP32);
    }

    #[inline]
    pub(crate) fn map_key(&mut self, key: &str) {
        self.write_str(key);
    }

    #[inline]
    pub(crate) fn map_key_encoded(&mut self, encoded: &[u8]) {
        self.buf.extend_from_slice(encoded);
    }

    #[inline]
    pub(crate) fn end_map(&mut self) {
        self.end_container(ContainerKind::Map);
    }

    #[inline]
    pub(crate) fn begin_array(&mut self) {
        self.begin_container(ContainerKind::Array, ARRAY32);
    }

    #[inline]
    pub(crate) fn end_array(&mut self) {
        self.end_container(ContainerKind::Array);
    }

    #[inline]
    fn begin_container(&mut self, kind: ContainerKind, marker: u8) {
        let header_pos = self.buf.len();
        self.buf.extend_from_slice(&[marker, 0, 0, 0, 0]);
        self.containers.push(Container {
            kind,
            header_pos,
            items: 0,
        });
    }

    #[inline]
    fn end_container(&mut self, expected: ContainerKind) {
        let container = self
            .containers
            .pop()
            .expect("container end without matching begin");
        debug_assert_eq!(container.kind, expected);
        self.buf[container.header_pos + 1..container.header_pos + 5]
            .copy_from_slice(&container.items.to_be_bytes());
    }
}

#[inline]
fn write_u64_to(buf: &mut Vec<u8>, value: u64) {
    if value <= 0x7f {
        buf.push(value as u8);
    } else if value <= u8::MAX as u64 {
        buf.extend_from_slice(&[0xcc, value as u8]);
    } else if value <= u16::MAX as u64 {
        buf.push(0xcd);
        buf.extend_from_slice(&(value as u16).to_be_bytes());
    } else if value <= u32::MAX as u64 {
        buf.push(0xce);
        buf.extend_from_slice(&(value as u32).to_be_bytes());
    } else {
        buf.push(0xcf);
        buf.extend_from_slice(&value.to_be_bytes());
    }
}

#[inline]
fn write_str_to(buf: &mut Vec<u8>, value: &str) {
    let bytes = value.as_bytes();
    let len = bytes.len();
    if len <= 31 {
        buf.push(0xa0 | len as u8);
        buf.extend_from_slice(bytes);
    } else {
        write_len_prefixed(
            buf,
            bytes,
            0xd9,
            0xda,
            0xdb,
            "MessagePack string is too large",
        );
    }
}

#[inline]
fn write_len_prefixed(
    buf: &mut Vec<u8>,
    value: &[u8],
    marker8: u8,
    marker16: u8,
    marker32: u8,
    too_large: &'static str,
) {
    let len = value.len();
    if len <= u8::MAX as usize {
        buf.extend_from_slice(&[marker8, len as u8]);
    } else if len <= u16::MAX as usize {
        buf.push(marker16);
        buf.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        let len: u32 = len.try_into().expect(too_large);
        buf.push(marker32);
        buf.extend_from_slice(&len.to_be_bytes());
    }
    buf.extend_from_slice(value);
}
