//! Represents an object in PHP. Allows for overriding the internal object used by classes,
//! allowing users to store Rust data inside a PHP object.

use std::{
    mem,
    ops::{Deref, DerefMut},
};

use crate::{
    bindings::{
        php_rs_zend_object_alloc, std_object_handlers, zend_object, zend_object_handlers,
        zend_object_std_init,
    },
    php::{class::ClassEntry, execution_data::ExecutionData},
};

pub type ZendObject = zend_object;
pub type ZendObjectHandlers = zend_object_handlers;

pub trait ZendObjectOverride {
    extern "C" fn create_object(ce: *mut ClassEntry) -> *mut ZendObject;
}

/// A Zend class object which is allocated when a PHP
/// class object is instantiated. Overrides the default
/// handler when the user provides a type T of the struct
/// they want to override with.
#[repr(C)]
#[derive(Debug)]
pub struct ZendClassObject<T: Default> {
    obj: T,
    std: *mut zend_object,
}

impl<T: Default> ZendClassObject<T> {
    /// Allocates a new object when an instance of the class is created
    /// in the PHP world.
    ///
    /// Internal function. The end user functions are generated by the
    /// [`object_override_handler`] macro which generates a function that
    /// wraps this function to be exported to C.
    ///
    /// # Parameters
    ///
    /// * `ce` - The class entry that was created.
    /// * `handlers` - A pointer to the object handlers for the class.
    ///
    /// # Safety
    ///
    /// This function is an internal function which is called only from
    /// the function generated by the [`object_override_handler`] macro,
    /// which in turn is called from the PHP runtime. PHP guarantees that
    /// the given [`ClassEntry`] is valid. The `handlers` to this function
    /// are also initialized by the [`object_handlers_init`] macro.
    /// However, we cannot ensure the user will call both of these macros
    /// in the same place.
    pub unsafe fn new_ptr(
        ce: *mut ClassEntry,
        handlers: *const ZendObjectHandlers,
    ) -> *mut zend_object {
        let obj = {
            let obj = (php_rs_zend_object_alloc(std::mem::size_of::<Self>() as u64, ce)
                as *mut Self)
                .as_mut()
                .unwrap();

            zend_object_std_init(obj.std, ce);
            obj
        };

        obj.obj = T::default();
        (*obj.std).handlers = handlers;
        obj.std
    }

    /// Attempts to retrieve the zend class object container from the
    /// zend object contained in the execution data of a function.
    ///
    /// # Parameters
    ///
    /// * `ex` - The execution data of the function.
    pub fn get(ex: &ExecutionData) -> Option<&'static mut Self> {
        // cast to u8 to work in terms of bytes
        let ptr = ex.This.object()? as *mut u8;
        let offset = std::mem::size_of::<T>();
        unsafe {
            let ptr = ptr.offset(0 - offset as isize);
            (ptr as *mut Self).as_mut()
        }
    }
}

impl<T: Default> Deref for ZendClassObject<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.obj
    }
}

impl<T: Default> DerefMut for ZendClassObject<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.obj
    }
}

impl ZendObjectHandlers {
    pub fn init() -> *mut ZendObjectHandlers {
        unsafe {
            let s = mem::size_of::<Self>();
            let ptr = libc::malloc(s) as *mut Self;
            libc::memcpy(
                ptr as *mut _,
                (&std_object_handlers as *const Self) as *mut _,
                s,
            );
            ptr
        }
    }
}