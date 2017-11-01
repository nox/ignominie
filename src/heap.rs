use Exhume;
use core::marker::PhantomData;
use core::mem;
use core::ptr;
use core::slice;
use error::{self, Error};

pub fn decode<'input, T>(input: &'input mut [u8]) -> Result<&'input T, Error>
where
    T: Exhume<'input>,
{
    let mut heap = Heap::new(input);
    let ptr = heap.reserve::<T>(0, 1)?;
    unsafe {
        T::exhume(ptr, &mut heap)?;
        Ok(&*ptr)
    }
}

pub struct Heap<'input> {
    start: *mut u8,
    remaining: *mut u8,
    end: *mut u8,
    marker: PhantomData<&'input mut ()>,
}

impl<'input> Heap<'input> {
    fn new(input: &'input mut [u8]) -> Self {
        let start = input.as_mut_ptr();
        Heap {
            start,
            remaining: start,
            end: unsafe { start.offset(input.len() as isize) },
            marker: PhantomData,
        }
    }

    fn reserve<T>(
        &mut self,
        offset: usize,
        len: usize,
    ) -> Result<*mut T, Error> {
        let ptr =
            (self.start as usize).checked_add(offset).ok_or(error::basic())?;
        if ptr < self.remaining as usize {
            return Err(error::basic());
        }
        if ptr % mem::align_of::<T>() != 0 {
            return Err(error::basic());
        }
        let byte_len =
            len.checked_mul(mem::size_of::<T>()).ok_or(error::basic())?;
        let remaining = ptr.checked_add(byte_len).ok_or(error::basic())?;
        if remaining > self.end as usize {
            return Err(error::basic());
        }
        self.remaining = remaining as *mut u8;
        Ok(ptr as *mut T)
    }
}

impl<'input, T> Exhume<'input> for &'input T
where
    T: Exhume<'input>,
{
    unsafe fn exhume(
        this: *mut Self,
        heap: &mut Heap<'input>,
    ) -> Result<(), Error> {
        let _ = mem::transmute::<Self, usize>;
        if (*(this as *const *const T)).is_null() {
            return Err(error::basic());
        }
        let ptr = heap.reserve::<T>(*(this as *mut usize), 1)?;
        T::exhume(ptr, heap)?;
        *this = &*ptr;
        Ok(())
    }
}

impl<'input, T> Exhume<'input> for &'input [T]
where
    T: Exhume<'input>,
{
    unsafe fn exhume(
        this: *mut Self,
        heap: &mut Heap<'input>,
    ) -> Result<(), Error> {
        if *(this as *const *const [T]) as *const T == ptr::null::<T>() {
            return Err(error::basic());
        }
        let offset = (*this).as_ptr() as usize;
        let len = (*this).len();
        let ptr = heap.reserve::<T>(offset, len)?;
        for i in 0..len {
            T::exhume(ptr.offset(i as isize), heap)?;
        }
        *this = slice::from_raw_parts(ptr, len);
        Ok(())
    }
}
