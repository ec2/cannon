extern "C" {
    fn get_storage(key_ptr: usize, out_ptr: usize);
    fn set_storage(key_ptr: usize, value_ptr: usize);
}

#[no_mangle]
pub extern "C" fn entrypoint() {
    let key = [1u8; 32];
    let mut result = [0u8; 32];
    unsafe {
        get_storage(key.as_ptr() as usize, result.as_mut_ptr() as usize);
    }
    if result[0] == 0 {
        result[0] = 1;
    } else {
        result[0] = 0;
    }

    unsafe {
        set_storage(key.as_ptr() as usize, result.as_ptr() as usize);
    }
}
