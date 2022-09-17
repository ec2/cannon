use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;
use wasmi::{
    AsContext, AsContextMut, Caller, Engine, Extern, Func, Linker, Memory, MemoryType, Module,
    Store,
};

pub type Bytes32 = [u8; 32];

/// The implementation of the external API of the VM.
pub trait Ext {
    /// Returns the storage value at the given key.
    fn get(&self, key: &Bytes32) -> Bytes32;
    /// Sets the storage value at the given key.
    fn set(&mut self, key: &Bytes32, value: &Bytes32);
}

// get calls state trie
// set sets trie

struct VmStateInner {
    ext: Box<dyn Ext>,
    memory: Option<Memory>,
}

#[derive(Clone)]
struct VmState(Rc<RefCell<VmStateInner>>);

impl VmState {
    fn new(ext: Box<dyn Ext>) -> Self {
        VmState(Rc::new(RefCell::new(VmStateInner { ext, memory: None })))
    }

    /// A hack required for side-stepping the chicken-egg problem during the initialization of the
    /// store and the state.
    fn deferred_set_memory(&self, memory: Memory) {
        self.0.borrow_mut().memory = Some(memory);
    }

    /// Read 32 bytes from the contract memory at the given offset.
    ///
    /// Panics in case OOB.
    fn read_bytes32(&self, caller: impl AsContext<UserState = Self>, offset: u32) -> Bytes32 {
        let me = self.0.borrow();
        let memory = me.memory.as_ref().expect("memory is not initialized");
        let mut buf = [0u8; 32];
        memory.read(caller, offset as usize, &mut buf).unwrap();
        buf
    }

    /// Writes 32 bytes into the contract memory at the given offset.
    ///
    /// Panics in case OOB.
    fn write_bytes32(
        &self,
        caller: impl AsContextMut<UserState = Self>,
        offset: u32,
        bytes: &Bytes32,
    ) {
        let me = self.0.borrow_mut();
        let memory = me.memory.as_ref().expect("memory is not initialized");
        memory.write(caller, offset as usize, bytes).unwrap();
    }

    /// Reads a vector of bytes from the specified range and returns it.
    fn read(&self, caller: impl AsContextMut<UserState = Self>, offset: u32, len: u32) -> Vec<u8> {
        let me = self.0.borrow_mut();
        let memory = me.memory.as_ref().expect("memory is not initialized");
        let mut buf = vec![0u8; len as usize];
        memory.read(caller, offset as usize, &mut buf).unwrap();
        buf
    }

    fn ext(&self) -> Ref<'_, dyn Ext> {
        Ref::map(self.0.borrow(), |me| &*me.ext)
    }

    fn ext_mut(&self) -> RefMut<'_, dyn Ext> {
        RefMut::map(self.0.borrow_mut(), |me| &mut *me.ext)
    }
}

/// Creates an implementation of the linker, the thing that binds the API of this wasm runtime to
/// the implementations of the host functions.
fn populate_linker(
    mut context: impl AsContextMut<UserState = VmState>,
    memory: Memory,
) -> anyhow::Result<Linker<VmState>> {
    let env_get_storage = Func::wrap(
        &mut context,
        |mut caller: Caller<'_, VmState>, key_ptr: u32, out_ptr: u32| {
            let state = caller.host_data().clone();
            let key = state.read_bytes32(&caller, key_ptr);
            let value = state.ext().get(&key);
            state.write_bytes32(&mut caller, out_ptr, &value);
        },
    );

    let env_set_storage = Func::wrap(
        &mut context,
        |mut caller: Caller<'_, VmState>, key_ptr: u32, value_ptr: u32| {
            let state = caller.host_data().clone();
            let key = state.read_bytes32(&caller, key_ptr);
            let value = state.read_bytes32(&caller, value_ptr);
            state.ext_mut().set(&key, &value);
        },
    );

    let env_print = Func::wrap(
        &mut context,
        |mut caller: Caller<'_, VmState>, ptr: u32, len: u32| {
            let state = caller.host_data().clone();
            let bytes = state.read(&mut caller, ptr, len);
            let str = String::from_utf8_lossy(&bytes);
            let hex = hex::encode(&bytes);
            println!("print: {:?} (hex: {:?})", str, hex);
        },
    );

    let mut linker = Linker::new();
    linker.define("env", "memory", memory)?;
    linker.define("env", "get_storage", env_get_storage)?;
    linker.define("env", "set_storage", env_set_storage)?;
    linker.define("env", "print", env_print)?;
    Ok(linker)
}

/// Executes the given wasm contract.
pub fn execute(ext: Box<dyn Ext>, wasm: &[u8], calldata: Vec<u8>) -> anyhow::Result<()> {
    let engine = Engine::default();
    let module = Module::new(&engine, wasm)?;
    let state = VmState::new(ext);
    let mut store = Store::new(&engine, state.clone());

    // Allocate 16 wasm pages of memory for the contract. Each wasm page is 64 KiB. Allow up to 32
    // pages.
    let memory =
        Memory::new(&mut store, MemoryType::new(16, Some(32))).map_err(handle_memory_err)?;
    state.deferred_set_memory(memory.clone());

    // Save the calldata into the contract memory.
    //
    // This is a bit of a hack, since we don't want to bother with proper allocation and stuff, so
    // we just slap the calldata in the beginning of the memory. LLD lays out the memory so that
    // the 1 MiB stack is placed at the beginning.
    //
    // The layout of the thing we write is as follows:
    //
    // [4..36) - length of the calldata
    // [36..) - the calldata itself
    //
    // The reason why we don't place the length at the offset 0, is because LLVM and other compilers
    // have special treatment for it: it's basically UB.
    //
    // A big caveat: the calldata cannot be larger than 1 MiB and in practice it should be less than
    // that, since the stack can overwrite.
    memory
        .write(&mut store, 4, &calldata.len().to_le_bytes())
        .map_err(handle_memory_err)?;
    memory
        .write(&mut store, 36, &calldata)
        .map_err(handle_memory_err)?;

    let mut linker = populate_linker(&mut store, memory)?;

    let instance = linker.instantiate(&mut store, &module)?.start(&mut store)?;

    let main = instance
        .get_export(&store, "entrypoint")
        .and_then(Extern::into_func)
        .ok_or_else(|| anyhow::anyhow!("could not find function \"entrypoint\""))?
        .typed::<(), (), _>(&mut store)?;

    main.call(&mut store, ())?;

    return Ok(());

    fn handle_memory_err(err: wasmi::errors::MemoryError) -> anyhow::Error {
        anyhow::anyhow!("memory error: {}", err)
    }
}

mod tests {
    use super::*;
    use std::collections::HashMap;

    struct TestExtInner {
        storage: HashMap<Bytes32, Bytes32>,
    }

    #[derive(Clone)]
    struct TestExt(Rc<RefCell<TestExtInner>>);

    impl TestExt {
        fn new() -> Self {
            TestExt(Rc::new(RefCell::new(TestExtInner {
                storage: HashMap::new(),
            })))
        }
    }

    impl Ext for TestExt {
        fn get(&self, key: &Bytes32) -> Bytes32 {
            self.0
                .borrow()
                .storage
                .get(key)
                .cloned()
                .unwrap_or_default()
        }

        fn set(&mut self, key: &Bytes32, value: &Bytes32) {
            self.0.borrow_mut().storage.insert(*key, *value);
        }
    }

    #[test]
    fn flipper_simple() {
        let wasm = include_bytes!(env!("CARGO_CDYLIB_FILE_FLIPPER"));
        let ext = TestExt::new();
        execute(Box::new(ext.clone()), wasm, vec![1u8; 32]).unwrap();

        // Flipper supposed to set the storage at key 0x0101..0101 to 1.
        let value = ext.get(&[1u8; 32]);
        assert_eq!(value[0], 1);

        execute(Box::new(ext.clone()), wasm, vec![1u8; 32]).unwrap();
        let value = ext.get(&[1u8; 32]);
        assert_eq!(value[0], 0);
    }
}
