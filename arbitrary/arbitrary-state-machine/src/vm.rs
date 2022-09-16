use std::cell::{Ref, RefMut, RefCell};
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
    fn read_bytes(&self, caller: impl AsContext<UserState = Self>, offset: u32) -> Bytes32 {
        let me = self.0.borrow();
        let memory = me.memory.as_ref().expect("memory is not initialized");
        let mut buf = [0u8; 32];
        memory.read(caller, offset as usize, &mut buf).unwrap();
        buf
    }

    /// Writes 32 bytes into the contract memory at the given offset.
    ///
    /// Panics in case OOB.
    fn write_bytes(
        &self,
        caller: impl AsContextMut<UserState = Self>,
        offset: u32,
        bytes: &Bytes32,
    ) {
        let me = self.0.borrow_mut();
        let memory = me.memory.as_ref().expect("memory is not initialized");
        memory.write(caller, offset as usize, bytes).unwrap();
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
            let key = state.read_bytes(&caller, key_ptr);
            let value = state.ext().get(&key);
            state.write_bytes(&mut caller, out_ptr, &value);
        },
    );

    let env_set_storage = Func::wrap(
        &mut context,
        |mut caller: Caller<'_, VmState>, key_ptr: u32, value_ptr: u32| {
            let state = caller.host_data().clone();
            let key = state.read_bytes(&caller, key_ptr);
            let value = state.read_bytes(&caller, value_ptr);
            state.ext_mut().set(&key, &value);
        },
    );

    let mut linker = Linker::new();
    linker.define("env", "memory", memory)?;
    linker.define("env", "get_storage", env_get_storage)?;
    linker.define("env", "set_storage", env_set_storage)?;
    Ok(linker)
}

/// Executes the given wasm contract.
pub fn execute(ext: Box<dyn Ext>, wasm: &[u8]) -> anyhow::Result<()> {
    let engine = Engine::default();
    let module = Module::new(&engine, wasm)?;
    let state = VmState::new(ext);
    let mut store = Store::new(&engine, state.clone());

    // Allocate 16 wasm pages of memory for each contract. Each wasm page is 64 KiB.
    let memory = Memory::new(&mut store, MemoryType::new(16, Some(16)))
        .map_err(|e| anyhow::anyhow!("err: {}", e))?;
    state.deferred_set_memory(memory.clone());

    let mut linker = populate_linker(&mut store, memory)?;

    let instance = linker.instantiate(&mut store, &module)?.start(&mut store)?;

    let main = instance
        .get_export(&store, "entrypoint")
        .and_then(Extern::into_func)
        .ok_or_else(|| anyhow::anyhow!("could not find function \"entrypoint\""))?
        .typed::<(), (), _>(&mut store)?;

    main.call(&mut store, ())?;

    Ok(())
}

mod tests {
    use std::collections::HashMap;
    use super::*;

    struct TestExtInner {
        storage: HashMap<Bytes32, Bytes32>,
    }

    #[derive(Clone)]
    struct TestExt(Rc<RefCell<TestExtInner>>);

    impl TestExt {
        fn new() -> Self {
            TestExt(Rc::new(RefCell::new(TestExtInner { storage: HashMap::new() })))
        }
    }

    impl Ext for TestExt {
        fn get(&self, key: &Bytes32) -> Bytes32 {
            self.0.borrow().storage.get(key).cloned().unwrap_or_default()
        }

        fn set(&mut self, key: &Bytes32, value: &Bytes32) {
            self.0.borrow_mut().storage.insert(*key, *value);
        }
    }

    #[test]
    fn flipper_simple() {
        let wasm = include_bytes!(env!("CARGO_CDYLIB_FILE_FLIPPER"));
        let ext = TestExt::new();
        execute(Box::new(ext.clone()), wasm).unwrap();

        // Flipper supposed to set the storage at key 0x0101..0101 to 1.
        let value = ext.get(&[1u8; 32]);
        assert_eq!(value[0], 1);

        execute(Box::new(ext.clone()), wasm).unwrap();
        let value = ext.get(&[1u8; 32]);
        assert_eq!(value[0], 0);
    }
}
