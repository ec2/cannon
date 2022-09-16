#[no_mangle]
pub extern "C" fn entrypoint() {
    let mut result = arbitrary_sdk::get_storage(&[1u8; 32]);

    if result[0] == 0 {
        result[0] = 1;
    } else {
        result[0] = 0;
    }

    arbitrary_sdk::set_storage(&[1u8; 32], &result);
}
