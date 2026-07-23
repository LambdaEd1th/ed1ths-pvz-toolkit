use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

/// Shared immutable data whose UI equality is based on allocation identity.
///
/// Dioxus compares component props frequently. Comparing the payload behind an
/// `Arc` would make unrelated UI updates walk entire documents and text buffers.
pub(crate) struct IdentityArc<T>(Arc<T>);

impl<T> IdentityArc<T> {
    pub(crate) fn new(value: T) -> Self {
        Self(Arc::new(value))
    }

    pub(crate) fn arc(&self) -> Arc<T> {
        self.0.clone()
    }
}

impl<T> Clone for IdentityArc<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> From<Arc<T>> for IdentityArc<T> {
    fn from(value: Arc<T>) -> Self {
        Self(value)
    }
}

impl<T> Deref for IdentityArc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl<T> AsRef<T> for IdentityArc<T> {
    fn as_ref(&self) -> &T {
        self.0.as_ref()
    }
}

impl<T> PartialEq for IdentityArc<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl<T> Eq for IdentityArc<T> {}

impl<T: fmt::Debug> fmt::Debug for IdentityArc<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Payload;

    impl PartialEq for Payload {
        fn eq(&self, _other: &Self) -> bool {
            panic!("identity equality must not compare the payload");
        }
    }

    impl Eq for Payload {}

    #[test]
    fn equality_uses_arc_allocation_identity() {
        let shared = IdentityArc::new(Payload);
        assert_eq!(shared, shared.clone());
        assert_ne!(shared, IdentityArc::new(Payload));
    }
}
