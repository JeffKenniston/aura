#[repr(C)]
pub struct UnifiedDiffPayload {
    pub file_count: u32,
    pub total_additions: u32,
    pub total_deletions: u32,
    pub diff_data_len: usize,
    pub data_ptr: *const u8,
}

#[repr(C)]
pub struct AstGraphPayload {
    pub node_count: u32,
    pub edges_len: usize,
    pub edges_ptr: *const u8,
}
