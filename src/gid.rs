use rdma_sys::*;
use serde::{Deserialize, Serialize};
/// A Global identifier for ibv.
///
/// This struct acts as a rust wrapper for `ffi::ibv_gid`. We use it instead of
/// `ffi::ibv_giv` because `ffi::ibv_gid` is actually an untagged union.
///
/// ```c
/// union ibv_gid {
///     uint8_t   raw[16];
///     struct {
///         __be64  subnet_prefix;
///         __be64  interface_id;
///     } global;
/// };
/// ```
///
/// It appears that `global` exists for convenience, but can be safely ignored.
/// For continuity, the methods `subnet_prefix` and `interface_id` are provided.
/// These methods read the array as big endian, regardless of native cpu
/// endianness.
#[derive(Default, Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Gid {
    raw: [u8; 16],
}

impl Gid {
    /// Expose a pointer to the underlying `ffi::ibv_gid` object for use in ffi
    /// calls.
    pub fn ffi(&mut self) -> *mut ibv_gid {
        self.raw.as_mut_ptr() as _
    }

    /// Expose the subnet_prefix component of the `Gid` as a u64. This is
    /// equivalent to accessing the `global.subnet_prefix` component of the
    /// `ffi::ibv_gid` union.
    #[allow(dead_code)]
    pub fn subnet_prefix(&self) -> u64 {
        u64::from_be_bytes(self.raw[..8].try_into().unwrap())
    }

    /// Expose the interface_id component of the `Gid` as a u64. This is
    /// equivalent to accessing the `global.interface_id` component of the
    /// `ffi::ibv_gid` union.
    #[allow(dead_code)]
    pub fn interface_id(&self) -> u64 {
        u64::from_be_bytes(self.raw[8..].try_into().unwrap())
    }
}

impl From<ibv_gid> for Gid {
    fn from(gid: ibv_gid) -> Self {
        Self {
            raw: unsafe { gid.raw },
        }
    }
}

impl From<Gid> for ibv_gid {
    fn from(mut gid: Gid) -> Self {
        unsafe { *gid.ffi() }
    }
}
