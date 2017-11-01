#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate core;

mod error;
mod heap;

use core::char;
use core::cmp::Ordering;
use core::marker::PhantomData;
use core::mem;
use core::num::{FpCategory, Wrapping};
use core::ops::{Range, RangeFrom, RangeFull, RangeTo};
use core::str;
#[cfg(feature = "std")]
use std::ffi::{CStr, OsStr};
#[cfg(feature = "std")]
use std::net::Shutdown;
#[cfg(all(feature = "std", unix))]
use std::os::unix::ffi::OsStrExt;
#[cfg(feature = "std")]
use std::panic::AssertUnwindSafe;
#[cfg(feature = "std")]
use std::path::Path;
#[cfg(feature = "std")]
use std::string::ParseError;

pub use error::Error;
pub use heap::{Heap, decode};

pub trait Exhume<'input> {
    unsafe fn exhume(
        this: *mut Self,
        heap: &mut Heap<'input>,
    ) -> Result<(), Error>;
}

macro_rules! noop_impl {
    ($($ty:ty,)+) => {
        $(impl<'input> Exhume<'input> for $ty {
            unsafe fn exhume(
                _this: *mut Self,
                _heap: &mut Heap<'input>,
            ) -> Result<(), Error> {
                Ok(())
            }
        })+
    };
}

noop_impl!(
    (),
    RangeFull,
    u8,
    u16,
    u32,
    u64,
    usize,
    i8,
    i16,
    i32,
    i64,
    isize,
);

macro_rules! parameterised_newtype_impl {
    ($($(#[$attr:meta])* $ty:ident,)+) => {
        $($(#[$attr])*
        impl<'input, T> Exhume<'input> for $ty<T>
        where
            T: Exhume<'input>,
        {
            unsafe fn exhume(
                this: *mut Self,
                heap: &mut Heap<'input>,
            ) -> Result<(), Error> {
                #[allow(dead_code)]
                fn assert_shape<T>($ty(_): $ty<T>) {}
                T::exhume(&mut (*this).0 as *mut T, heap)
            }
        })+
    };
}

parameterised_newtype_impl!(
    #[cfg(feature = "std")] AssertUnwindSafe,
    Wrapping,
);

impl<'input> Exhume<'input> for bool {
    unsafe fn exhume(
        this: *mut Self,
        _heap: &mut Heap<'input>,
    ) -> Result<(), Error> {
        let _ = mem::transmute::<Self, u8>;
        let byte = *(this as *const u8);
        if byte == true as u8 || byte == false as u8 {
            Ok(())
        } else {
            Err(error::basic())
        }
    }
}

impl<'input> Exhume<'input> for f32 {
    unsafe fn exhume(
        this: *mut Self,
        _heap: &mut Heap<'input>,
    ) -> Result<(), Error> {
        let _ = mem::transmute::<Self, u32>;
        let bits = *(this as *const u32);
        if bits & 0x1FF << 22 == 0x1FF << 22 && bits & 0x3FFFFF != 0 {
            // Signaling NaNs are errors.
            return Err(error::basic());
        }
        Ok(())
    }
}

impl<'input> Exhume<'input> for f64 {
    unsafe fn exhume(
        this: *mut Self,
        _heap: &mut Heap<'input>,
    ) -> Result<(), Error> {
        let _ = mem::transmute::<Self, u64>;
        let bits = *(this as *const u64);
        if bits & 0xFFF << 51 == 0xFFF << 51 && bits & 0xFFFFFFFFFFFFF != 0 {
            // Signaling NaNs are errors.
            return Err(error::basic());
        }
        Ok(())
    }
}

impl<'input> Exhume<'input> for char {
    unsafe fn exhume(
        this: *mut Self,
        _heap: &mut Heap<'input>,
    ) -> Result<(), Error> {
        let _ = mem::transmute::<Self, u32>;
        char::from_u32(*(this as *mut u32)).ok_or(error::basic())?;
        Ok(())
    }
}

impl<'input> Exhume<'input> for &'input str {
    unsafe fn exhume(
        this: *mut Self,
        heap: &mut Heap<'input>,
    ) -> Result<(), Error> {
        let _ = mem::transmute::<Self, &[u8]>;
        let ptr = this as *mut &[u8];
        <&[u8]>::exhume(ptr, heap)?;
        str::from_utf8(*ptr).ok().ok_or(error::basic())?;
        Ok(())
    }
}

#[cfg(feature = "std")]
impl<'input> Exhume<'input> for &'input CStr {
    unsafe fn exhume(
        this: *mut Self,
        heap: &mut Heap<'input>,
    ) -> Result<(), Error> {
        let _ = mem::transmute::<Self, &[u8]>;
        let ptr = this as *mut &[u8];
        <&[u8]>::exhume(ptr, heap)?;
        CStr::from_bytes_with_nul(*ptr).ok().ok_or(error::basic())?;
        Ok(())
    }
}

#[cfg(all(feature = "std", unix))]
impl<'input> Exhume<'input> for &'input OsStr {
    unsafe fn exhume(
        this: *mut Self,
        heap: &mut Heap<'input>,
    ) -> Result<(), Error> {
        let _ = mem::transmute::<Self, &[u8]>;
        let ptr = this as *mut &[u8];
        <&[u8]>::exhume(ptr, heap)?;
        let _ = OsStr::from_bytes(*ptr);
        Ok(())
    }
}

#[cfg(feature = "std")]
impl<'input> Exhume<'input> for &'input Path {
    #[cfg(unix)]
    unsafe fn exhume(
        this: *mut Self,
        heap: &mut Heap<'input>,
    ) -> Result<(), Error> {
        let _ = mem::transmute::<Self, &OsStr>;
        let ptr = this as *mut OsStr;
        OsStr::exhume(ptr, heap)?;
        let _ = Path::new(*ptr);
        Ok(())
    }

    #[cfg(not(unix))]
    unsafe fn exhume(
        this: *mut Self,
        heap: &mut Heap<'input>,
    ) -> Result<(), Error> {
        let _ = mem::transmute::<Self, &str>;
        let ptr = this as *mut str;
        <&str>::exhume(ptr, heap)?;
        let _ = Path::new(*ptr);
        Ok(())
    }
}

impl<'input, T> Exhume<'input> for PhantomData<T> {
    unsafe fn exhume(
        _this: *mut Self,
        _heap: &mut Heap<'input>,
    ) -> Result<(), Error> {
        Ok(())
    }
}

macro_rules! c_enum_impl {
    ($(
        $(#[$attr:meta])*
        enum $ty:ident: $repr:ident { $($name:ident,)+ }
    )+) => {
        $($(#[$attr])* impl<'input> Exhume<'input> for $ty {
            #[allow(non_upper_case_globals)]
            unsafe fn exhume(
                this: *mut Self,
                _heap: &mut Heap<'input>,
            ) -> Result<(), Error> {
                let _ = mem::transmute::<Self, $repr>;
                let ptr = this as *mut $repr;
                #[allow(dead_code)]
                fn assert_shape<T>(value: $ty) {
                    match value {
                        $($ty::$name => {},)+
                    }
                }
                $(const $name: $repr = $ty::$name as $repr;)+
                match *ptr {
                    $($name => Ok(()),)+
                    _ => Err(error::basic())
                }
            }
        })+
    }
}

c_enum_impl! {
    enum Ordering: u8 {
        Less,
        Equal,
        Greater,
    }

    enum FpCategory: u8 {
        Nan,
        Infinite,
        Zero,
        Subnormal,
        Normal,
    }

    #[cfg(feature = "std")]
    enum Shutdown: u8 {
        Read,
        Write,
        Both,
    }
}

#[cfg(feature = "std")]
impl<'input> Exhume<'input> for ParseError {
    unsafe fn exhume(
        _this: *mut Self,
        _heap: &mut Heap<'input>,
    ) -> Result<(), Error> {
        #[allow(dead_code)]
        fn assert_shape(value: ParseError) {
            match value {}
        }
        Err(error::basic())
    }
}

macro_rules! range_impl {
    ($($ty:ident { $($name:ident),* })+) => {
        $(impl<'input, T> Exhume<'input> for $ty<T>
        where
            T: Exhume<'input>,
        {
            unsafe fn exhume(
                this: *mut Self,
                heap: &mut Heap<'input>,
            ) -> Result<(), Error> {
                #[allow(dead_code)]
                fn assert_shape<T>($ty { $($name: _),* }: $ty<T>) {}
                $(T::exhume(&mut (*this).$name as *mut T, heap)?;)*
                Ok(())
            }
        })+
    }
}

range_impl! {
    Range { start, end }
    RangeFrom { start }
    RangeTo { end }
}

macro_rules! array_impl {
    ($($len:expr,)+) => {
        $(impl<'input, T> Exhume<'input> for [T; $len]
        where
            T: Exhume<'input>,
        {
            unsafe fn exhume(
                this: *mut Self,
                heap: &mut Heap<'input>,
            ) -> Result<(), Error> {
                let ptr = this as *mut T;
                for i in 0..$len {
                    T::exhume(ptr.offset(i as isize), heap)?;
                }
                Ok(())
            }
        })+
    };
}

array_impl!(
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
    21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
);

macro_rules! tuple_impl {
    ($(($($ty:ident $pos:tt),*),)+) => {
        $(impl<'input, $($ty),*> Exhume<'input> for ($($ty,)*)
        where
            $($ty: Exhume<'input>,)*
        {
            #[allow(non_snake_case)]
            unsafe fn exhume(
                this: *mut Self,
                heap: &mut Heap<'input>,
            ) -> Result<(), Error> {
                $(<$ty>::exhume(&mut (*this).$pos as *mut $ty, heap)?;)*
                Ok(())
            }
        })+
    }
}

tuple_impl! {
    (A 0),
    (A 0, B 1),
    (A 0, B 1, C 2),
    (A 0, B 1, C 2, D 3),
    (A 0, B 1, C 2, D 3, E 4),
    (A 0, B 1, C 2, D 3, E 4, F 5),
    (A 0, B 1, C 2, D 3, E 4, F 5, G 6),
    (A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7),
    (A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8),
    (A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9),
    (A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10),
    (A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, I 8, J 9, K 10, L 11),
}
