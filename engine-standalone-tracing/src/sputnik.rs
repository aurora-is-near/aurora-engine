use std::cell::RefCell;
use std::ptr::NonNull;
use std::rc::Rc;

// TODO: implement evm* tracing traits for `crate::trace::Logs`, and tie this `traced_call` function to the FFI.
#[allow(dead_code)]
/// Capture all events from SputnikVM emitted from within the given closure using the given listener.
pub fn traced_call<T, R, F>(listener: &mut T, f: F) -> R
where
    T: evm_gasometer::tracing::EventListener
        + evm_runtime::tracing::EventListener
        + evm::tracing::EventListener
        + 'static,
    F: FnOnce() -> R,
{
    let mut gas_listener = SharedMutableReference::new(listener);
    let mut runtime_listener = gas_listener.clone();
    let mut evm_listener = gas_listener.clone();

    evm_gasometer::tracing::using(&mut gas_listener, || {
        evm_runtime::tracing::using(&mut runtime_listener, || {
            evm::tracing::using(&mut evm_listener, f)
        })
    })
}

/// This structure is intentionally private to this module as it is memory unsafe (contains a raw pointer).
/// Its purpose here is to allow a single event handling object to be used as the listener for
/// all SputnikVM events. It is needed because the listener must be passed as an object with a `'static`
/// lifetime, hence a normal reference cannot be used and we resort to raw pointers. The usage of this
/// struct in this module is safe because the `SharedMutableReference` objects created do not outlive
/// the reference they are based on (see `pub fn traced_call`). Moreover, because the SputnikVM code
/// is single-threaded, we do not need to worry about race conditions.
struct SharedMutableReference<T> {
    pointer: Rc<RefCell<NonNull<T>>>,
}

impl<T> SharedMutableReference<T> {
    fn new(reference: &mut T) -> Self {
        let ptr = NonNull::new(reference as _).unwrap();
        Self {
            pointer: Rc::new(RefCell::new(ptr)),
        }
    }

    fn clone(&self) -> Self {
        Self {
            pointer: Rc::clone(&self.pointer),
        }
    }
}

impl<T: evm_gasometer::tracing::EventListener> evm_gasometer::tracing::EventListener
    for SharedMutableReference<T>
{
    fn event(&mut self, event: evm_gasometer::tracing::Event) {
        unsafe {
            self.pointer.borrow_mut().as_mut().event(event);
        }
    }
}

impl<T: evm_runtime::tracing::EventListener> evm_runtime::tracing::EventListener
    for SharedMutableReference<T>
{
    fn event(&mut self, event: evm_runtime::tracing::Event) {
        unsafe {
            self.pointer.borrow_mut().as_mut().event(event);
        }
    }
}

impl<T: evm::tracing::EventListener> evm::tracing::EventListener for SharedMutableReference<T> {
    fn event(&mut self, event: evm::tracing::Event) {
        unsafe {
            self.pointer.borrow_mut().as_mut().event(event);
        }
    }
}
