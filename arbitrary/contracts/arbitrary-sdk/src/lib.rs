//! The SDK that allows you to write contracts in Rust.
//!
//! Basically provides a nice to use functions for interacting with the host.

mod ffi {
    //! The functions imported from the host/runtime.
    extern "C" {
        pub fn get_storage(key_ptr: usize, out_ptr: usize);
        pub fn set_storage(key_ptr: usize, value_ptr: usize);
    }
}

pub type Bytes32 = [u8; 32];

/// Reads the storage entry at the given key and returns it.
pub fn get_storage(key: &Bytes32) -> Bytes32 {
    let mut result = [0u8; 32];
    unsafe {
        ffi::get_storage(key.as_ptr() as usize, result.as_mut_ptr() as usize);
    }
    result
}

/// Sets the storage entry at the given key with the given value.
pub fn set_storage(key: &Bytes32, value: &Bytes32) {
    unsafe {
        ffi::set_storage(key.as_ptr() as usize, value.as_ptr() as usize);
    }
}
