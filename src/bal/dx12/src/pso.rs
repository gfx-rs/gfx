// Fixed size of the root signature.
// Limited by D3D12.
const ROOT_SIGNATURE_SIZE: usize = 64;

/// Strongly-typed root signature element
///
/// Could be removed for an unsafer variant to occupy less memory
#[derive(Debug, Copy, Clone)]
pub enum RootElement {
    /// Root constant in the signature
    Constant(u32),
    /// Descriptor table, storing table offset for the current descriptor heap
    TableSrvCbvUav(u32),
    /// Descriptor table, storing table offset for the current descriptor heap
    TableSampler(u32),
    /// Undefined value, implementation specific
    Undefined,
}

/// Virtual data storage for the current root signature memory.
#[derive(Clone)]
pub struct UserData {
    pub data: [RootElement; ROOT_SIGNATURE_SIZE],
    pub dirty_mask: u64,
}

impl UserData {
    pub fn new() -> Self {
        UserData {
            data: [RootElement::Undefined; ROOT_SIGNATURE_SIZE],
            dirty_mask: 0,
        }
    }

    /// Update root constant values. Changes are marked as dirty.
    pub fn set_constants(&mut self, offset: usize, data: &[u32]) {
        assert!(offset + data.len() <= ROOT_SIGNATURE_SIZE);
        // Each root constant occupies one DWORD
        for (i, val) in data.iter().enumerate() {
            self.data[offset + i] = RootElement::Constant(*val);
            self.dirty_mask |= 1u64 << (offset + i);
        }
    }

    /// Update descriptor table. Changes are marked as dirty.
    pub fn set_srv_cbv_uav_table(&mut self, offset: usize, table_start: u32) {
        assert!(offset < ROOT_SIGNATURE_SIZE);
        // A descriptor table occupies one DWORD
        self.data[offset] = RootElement::TableSrvCbvUav(table_start);
        self.dirty_mask |= 1u64 << offset;
    }

    /// Update descriptor table. Changes are marked as dirty.
    pub fn set_sampler_table(&mut self, offset: usize, table_start: u32) {
        assert!(offset < ROOT_SIGNATURE_SIZE);
        // A descriptor table occupies one DWORD
        self.data[offset] = RootElement::TableSampler(table_start);
        self.dirty_mask |= 1u64 << offset;
    }

    /// Clear dirty flag.
    pub fn clear_dirty(&mut self, i: usize) {
        self.dirty_mask &= !(1 << i);
    }

    /// Mark all entries as dirty.
    pub fn dirty_all(&mut self) {
        self.dirty_mask = !0;
    }
}
