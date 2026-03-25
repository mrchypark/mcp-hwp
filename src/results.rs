#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextResult {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryArtifact {
    pub bytes: Vec<u8>,
    pub bytes_len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileArtifact {
    pub path: String,
    pub bytes_len: u64,
}
