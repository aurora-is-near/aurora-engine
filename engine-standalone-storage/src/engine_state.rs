// TODO: replace with `Cow<'a, [u8]>`

#[derive(Debug)]
pub enum EngineStorageValue<'a> {
    Slice(&'a [u8]),
    Vec(Vec<u8>),
}

impl AsRef<[u8]> for EngineStorageValue<'_> {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Slice(slice) => slice,
            Self::Vec(bytes) => bytes,
        }
    }
}
