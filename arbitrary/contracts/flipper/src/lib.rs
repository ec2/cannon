//! Receives the key from the calldata, and flips that value from 0 to 1 or from 1 to 0.

#[no_mangle]
pub extern "C" fn entrypoint() {
    let calldata = arbitrary_sdk::calldata();
    let mut key = [0u8; 32];
    key.copy_from_slice(&calldata[0..32]);

    let mut result = arbitrary_sdk::get_storage(&key);

    if result[0] == 0 {
        result[0] = 1;
    } else {
        result[0] = 0;
    }

    arbitrary_sdk::set_storage(&key, &result);
}
