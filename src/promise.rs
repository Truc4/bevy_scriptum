use std::sync::{Arc, Mutex};

use crate::{Runtime, ScriptingError};

/// A struct that represents a function that will get called when the Promise is resolved.
pub(crate) struct PromiseCallback<C: Send, V: Send> {
    callback: V,
    following_promise: Arc<Mutex<PromiseInner<C, V>>>,
}

/// Internal representation of a Promise.
pub(crate) struct PromiseInner<C: Send, V: Send> {
    pub(crate) callbacks: Vec<PromiseCallback<C, V>>,
    #[allow(deprecated)]
    pub(crate) context: C,
}

/// A struct that represents a Promise.
#[derive(Clone)]
pub struct Promise<C: Send, V: Send> {
    pub(crate) inner: Arc<Mutex<PromiseInner<C, V>>>,
}

impl<C: Send, V: Send + Clone> PromiseInner<C, V> {
    /// Resolve the Promise. This will call all the callbacks that were added to the Promise.
    fn resolve<R>(&mut self, runtime: &mut R, val: R::Value) -> Result<(), ScriptingError>
    where
        R: Runtime<Value = V, CallContext = C>,
    {
        for callback in &self.callbacks {
            let next_val =
                runtime.call_fn_from_value(&callback.callback, &self.context, vec![val.clone()])?;

            callback
                .following_promise
                .lock()
                .expect("Failed to lock promise mutex")
                .resolve(runtime, next_val)?;
        }
        Ok(())
    }
}

impl<C: Clone + Send + 'static, V: Send + Clone> Promise<C, V> {
    /// Acquire [Mutex] for writing the promise and resolve it. Call will be forwarded to [PromiseInner::resolve].
    pub(crate) fn resolve<R>(
        &mut self,
        runtime: &mut R,
        val: R::Value,
    ) -> Result<(), ScriptingError>
    where
        R: Runtime<Value = V, CallContext = C>,
    {
        if let Ok(mut inner) = self.inner.lock() {
            inner.resolve(runtime, val)?;
        }
        Ok(())
    }

    /// Register a callback that will be called when the [Promise] is resolved.
    #[cfg(any(feature = "rhai", feature = "lua", feature = "ruby"))]
    pub(crate) fn then(&mut self, callback: V) -> Self {
        let mut inner = self
            .inner
            .lock()
            .expect("Failed to lock inner promise mutex");
        let following_inner = Arc::new(Mutex::new(PromiseInner {
            callbacks: vec![],
            context: inner.context.clone(),
        }));

        inner.callbacks.push(PromiseCallback {
            following_promise: following_inner.clone(),
            callback,
        });

        Promise {
            inner: following_inner,
        }
    }
}
