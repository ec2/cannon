//! The SDK that allows you to write contracts in Rust.
//!
//! Basically provides a nice to use functions for interacting with the host.

mod ffi {
    //! The functions imported from the host/runtime.
    extern "C" {
        pub fn get_storage(key_ptr: usize, out_ptr: usize);
        pub fn set_storage(key_ptr: usize, value_ptr: usize);
        pub fn print(ptr: usize, len: usize);
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

/// Returns the calldata passed to the contract.
pub fn calldata() -> Vec<u8> {
    unsafe {
        // [4..36) - length of the calldata
        // [36..) - the calldata itself
        let len = *(4 as usize as *const u32);
        let calldata_ptr = (36 as usize) as *const u8;
        std::slice::from_raw_parts(calldata_ptr, len as usize).to_vec()
    }
}

/// Prints the given string.
pub fn print(s: &str) {
    unsafe {
        ffi::print(s.as_ptr() as usize, s.len());
    }
}

/// Prints the given bytes slice.
pub fn print_bytes(b: &[u8]) {
    unsafe {
        ffi::print(b.as_ptr() as usize, b.len());
    }
}
