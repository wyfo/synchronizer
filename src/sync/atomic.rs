pub use std::sync::atomic::{compiler_fence, fence, Ordering};

macro_rules! atomic_method {
    ($atomic:ident, $method:ident($($param:ident: $tp:ty),*) $(-> $ret:ty)?) => {
            #[doc = "[`"]
            #[doc = concat!("std::sync::atomic::", stringify!($atomic), "::", stringify!($method)) ]
            #[doc = "`] mock."]
            #[inline]
            #[track_caller]
            pub fn $method(&self, $($param: $tp),*) $(-> $ret)? {
                crate::synchronizer::synchronized(self.0.$method($($param),*))
            }
    };
}

macro_rules! atomic {
    ($atomic:ident$(<$arg:ident>)?, $inner:ty, $($method:ident($($param:ident: $tp:ty),*) $(-> $ret:ty)?),* $(,)?) => {
        #[doc = "[`"]
        #[doc = concat!("std::sync::atomic::", stringify!($atomic)) ]
        #[doc = "`] mock."]
        #[derive(Default, Debug)]
        #[repr(transparent)]
        pub struct $atomic$(<$arg>)?(std::sync::atomic::$atomic$(<$arg>)?);

        impl<T> From<T> for $atomic$(<$arg>)?
        where
            std::sync::atomic::$atomic$(<$arg>)?: From<T>,
        {
            #[inline]
            fn from(value: T) -> Self {
                Self(value.into())
            }
        }

        impl$(<$arg>)? $atomic$(<$arg>)? {
            #[doc = "[`"]
            #[doc = concat!("std::sync::atomic::", stringify!($atomic), "::new") ]
            #[doc = "`] mock."]
            #[inline]
            pub fn new(v: $inner) -> Self {
                Self(v.into())
            }

            #[doc = "[`"]
            #[doc = concat!("std::sync::atomic::", stringify!($atomic), "::get_mut") ]
            #[doc = "`] mock."]
            #[inline]
            pub fn get_mut(&mut self) -> &mut $inner {
                self.0.get_mut()
            }

            #[doc = "[`"]
            #[doc = concat!("std::sync::atomic::", stringify!($atomic), "::into_inner") ]
            #[doc = "`] mock."]
            #[inline]
            pub fn into_inner(self) -> $inner {
                self.0.into_inner()
            }

            atomic_method!($atomic, load(order: Ordering) -> $inner);
            atomic_method!($atomic, store(val: $inner, order: Ordering));
            atomic_method!($atomic, swap(val: $inner, order: Ordering) -> $inner);
            atomic_method!($atomic, compare_exchange(current: $inner, new: $inner, success: Ordering, failure: Ordering) -> Result<$inner, $inner>);
            atomic_method!($atomic, compare_exchange_weak(current: $inner, new: $inner, success: Ordering, failure: Ordering) -> Result<$inner, $inner>);
            atomic_method!($atomic, fetch_update(set_order: Ordering, fetch_order: Ordering, f: impl FnMut($inner) -> Option<$inner>) -> Result<$inner, $inner>);
            $(atomic_method!($atomic, $method($($param: $tp),*) $(-> $ret)?);)*
        }
    };
    ($atomic:ident$(<$arg:ident>)?, $inner:ty) => {
        atomic!($atomic$(<$arg>)?, $inner,);
    };
}
macro_rules! atomic_int {
    ($atomic:ident, $inner:ty) => {
        atomic!(
            $atomic, $inner,
            fetch_add(val: $inner, order: Ordering) -> $inner,
            fetch_and(val: $inner, order: Ordering) -> $inner,
            fetch_max(val: $inner, order: Ordering) -> $inner,
            fetch_min(val: $inner, order: Ordering) -> $inner,
            fetch_nand(val: $inner, order: Ordering) -> $inner,
            fetch_or(val: $inner, order: Ordering) -> $inner,
            fetch_sub(val: $inner, order: Ordering) -> $inner,
            fetch_xor(val: $inner, order: Ordering) -> $inner,
        );
    };
}

atomic!(
    AtomicBool, bool,
    fetch_and(val: bool, order: Ordering) -> bool,
    fetch_nand(val: bool, order: Ordering) -> bool,
    fetch_or(val: bool, order: Ordering) -> bool,
    fetch_xor(val: bool, order: Ordering) -> bool,
);
atomic!(AtomicPtr<T>, *mut T);
atomic_int!(AtomicU8, u8);
atomic_int!(AtomicU16, u16);
atomic_int!(AtomicU32, u32);
atomic_int!(AtomicU64, u64);
atomic_int!(AtomicUsize, usize);
atomic_int!(AtomicI8, i8);
atomic_int!(AtomicI16, i16);
atomic_int!(AtomicI32, i32);
atomic_int!(AtomicI64, i64);
atomic_int!(AtomicIsize, isize);
