use core::slice;
use std::convert::TryFrom;

use crate::bindings::{zend_object, zend_resource, zval, IS_INTERNED_STRING_EX, IS_STRING_EX};

use super::{
    enums::DataType,
    types::{ZendLong, ZendString},
};

/// Zend value. Represents most data types that are in the Zend engine.
pub type Zval = zval;

impl Zval {
    /// Returns the value of the zval if it is a long.
    pub fn long(&self) -> Option<ZendLong> {
        if self.is_long() {
            Some(unsafe { self.value.lval })
        } else {
            None
        }
    }

    /// Returns the value of the zval if it is a bool.
    pub fn bool(&self) -> Option<bool> {
        if self.is_true() {
            Some(true)
        } else if self.is_false() {
            Some(false)
        } else {
            None
        }
    }

    /// Returns the value of the zval if it is a double.
    pub fn double(&self) -> Option<f64> {
        if self.is_double() {
            Some(unsafe { self.value.dval })
        } else if let Some(long) = self.long() {
            Some(long as f64)
        } else {
            None
        }
    }

    /// Returns the value of the zval if it is a string.
    pub fn string(&self) -> Option<String> {
        if self.is_string() {
            // SAFETY: Zend strings have a length that we know we can read.
            // By reading this many bytes we will not run into any issues.
            //
            // We can safely cast our *const c_char into a *const u8 as both
            // only occupy one byte.
            unsafe {
                let len = (*self.value.str).len;
                let ptr = (*self.value.str).val.as_ptr() as *const u8;
                let _str = std::str::from_utf8(slice::from_raw_parts(ptr, len as usize)).unwrap();

                Some(_str.to_string())
            }
        } else if let Some(double) = self.double() {
            Some(double.to_string())
        } else {
            None
        }
    }

    /// Returns the value of the zval if it is a resource.
    pub fn resource(&self) -> Option<*mut zend_resource> {
        // TODO: Can we improve this function? I haven't done much research into
        // resources so I don't know if this is the optimal way to return this.
        if self.is_resource() {
            Some(unsafe { self.value.res })
        } else {
            None
        }
    }

    /// Returns the value of the zval if it is an object.
    pub fn object(&self) -> Option<*mut zend_object> {
        // TODO: Can we improve this function? I haven't done much research into
        // objects so I don't know if this is the optimal way to return this.
        if self.is_object() {
            Some(unsafe { self.value.obj })
        } else {
            None
        }
    }

    /// Returns the value of the zval if it is a reference.
    pub fn reference(&self) -> Option<Zval> {
        if self.is_reference() {
            Some(unsafe { (*self.value.ref_).val })
        } else {
            None
        }
    }

    /// Returns true if the zval is a long, false otherwise.
    pub fn is_long(&self) -> bool {
        unsafe { self.u1.v.type_ == DataType::Long as u8 }
    }

    /// Returns true if the zval is null, false otherwise.
    pub fn is_null(&self) -> bool {
        unsafe { self.u1.v.type_ == DataType::Null as u8 }
    }

    /// Returns true if the zval is true, false otherwise.
    pub fn is_true(&self) -> bool {
        unsafe { self.u1.v.type_ == DataType::True as u8 }
    }

    /// Returns true if the zval is false, false otherwise.
    pub fn is_false(&self) -> bool {
        unsafe { self.u1.v.type_ == DataType::False as u8 }
    }

    /// Returns true if the zval is a bool, false otherwise.
    pub fn is_bool(&self) -> bool {
        self.is_true() || self.is_false()
    }

    /// Returns true if the zval is a double, false otherwise.
    pub fn is_double(&self) -> bool {
        unsafe { self.u1.v.type_ == DataType::Double as u8 }
    }

    /// Returns true if the zval is a string, false otherwise.
    pub fn is_string(&self) -> bool {
        unsafe { self.u1.v.type_ == DataType::String as u8 }
    }

    /// Returns true if the zval is a resource, false otherwise.
    pub fn is_resource(&self) -> bool {
        unsafe { self.u1.v.type_ == DataType::Resource as u8 }
    }

    /// Returns true if the zval is an array, false otherwise.
    pub fn is_array(&self) -> bool {
        unsafe { self.u1.v.type_ == DataType::Array as u8 }
    }

    /// Returns true if the zval is an object, false otherwise.
    pub fn is_object(&self) -> bool {
        unsafe { self.u1.v.type_ == DataType::Object as u8 }
    }

    /// Returns true if the zval is a reference, false otherwise.
    pub fn is_reference(&self) -> bool {
        unsafe { self.u1.v.type_ == DataType::Reference as u8 }
    }
}

/// Used to set the value of the zval.
///
/// This needs to be a trait to be implemented on a pointer that
/// points to a zval.
pub trait SetZval {
    /// Sets the value of the zval as a string.
    ///
    /// # Parameters
    ///
    /// * `val` - The value to set the zval as.
    fn set_string<S>(self, val: S) -> Result<(), String>
    where
        S: AsRef<str>;

    /// Sets the value of the zval as a long.
    ///
    /// # Parameters
    ///
    /// * `val` - The value to set the zval as.
    fn set_long(&self, val: ZendLong) -> Result<(), String>;

    /// Sets the value of the zval as a double.
    ///
    /// # Parameters
    ///
    /// * `val` - The value to set the zval as.
    fn set_double(&self, val: f64) -> Result<(), String>;

    /// Sets the value of the zval as a boolean.
    ///
    /// # Parameters
    ///
    /// * `val` - The value to set the zval as.
    fn set_bool(&self, val: bool) -> Result<(), String>;

    /// Sets the value of the zval as null.
    /// This is the default of a zval.
    fn set_null(&self) -> Result<(), String>;

    /// Sets the value of the zval as a resource.
    ///
    /// # Parameters
    ///
    /// * `val` - The value to set the zval as.
    fn set_resource(&self, val: *mut zend_resource) -> Result<(), String>;

    /// Sets the value of the zval as an object.
    ///
    /// # Parameters
    ///
    /// * `val` - The value to set the zval as.
    /// * `copy` - Whether to copy the object or pass as a reference.
    fn set_object(&self, val: *mut zend_object, copy: bool) -> Result<(), String>;
}

impl SetZval for *mut Zval {
    fn set_string<S>(self, val: S) -> Result<(), String>
    where
        S: AsRef<str>,
    {
        let _self = match unsafe { self.as_mut() } {
            Some(val) => val,
            None => {
                return Err(String::from(
                    "Could not retrieve mutable reference of zend value.",
                ))
            }
        };

        let zend_str = ZendString::new(val);
        _self.value.str = zend_str;
        _self.u1.type_info = if unsafe { zend_str.as_ref().unwrap().is_interned() } {
            IS_INTERNED_STRING_EX
        } else {
            IS_STRING_EX
        };

        Ok(())
    }

    fn set_long(&self, val: ZendLong) -> Result<(), String> {
        let _self = match unsafe { self.as_mut() } {
            Some(val) => val,
            None => {
                return Err(String::from(
                    "Could not retrieve mutable reference of zend value.",
                ))
            }
        };

        _self.value.lval = val;
        _self.u1.type_info = DataType::Long as u32;

        Ok(())
    }

    fn set_double(&self, val: f64) -> Result<(), String> {
        let _self = match unsafe { self.as_mut() } {
            Some(val) => val,
            None => {
                return Err(String::from(
                    "Could not retrieve mutable reference of zend value.",
                ))
            }
        };

        _self.value.dval = val;
        _self.u1.type_info = DataType::Double as u32;

        Ok(())
    }

    fn set_bool(&self, val: bool) -> Result<(), String> {
        let _self = match unsafe { self.as_mut() } {
            Some(val) => val,
            None => {
                return Err(String::from(
                    "Could not retrieve mutable reference of zend value.",
                ))
            }
        };

        _self.u1.type_info = if val {
            DataType::True as u32
        } else {
            DataType::False as u32
        };

        Ok(())
    }

    fn set_null(&self) -> Result<(), String> {
        let _self = match unsafe { self.as_mut() } {
            Some(val) => val,
            None => {
                return Err(String::from(
                    "Could not retrieve mutable reference of zend value.",
                ))
            }
        };

        _self.u1.type_info = DataType::Null as u32;

        Ok(())
    }

    fn set_resource(&self, val: *mut zend_resource) -> Result<(), String> {
        let _self = match unsafe { self.as_mut() } {
            Some(val) => val,
            None => {
                return Err(String::from(
                    "Could not retrieve mutable reference of zend value.",
                ))
            }
        };

        _self.u1.type_info = DataType::Resource as u32;
        _self.value.res = val;

        Ok(())
    }

    fn set_object(&self, val: *mut zend_object, _copy: bool) -> Result<(), String> {
        let _self = match unsafe { self.as_mut() } {
            Some(val) => val,
            None => {
                return Err(String::from(
                    "Could not retrieve mutable reference of zend value.",
                ))
            }
        };

        _self.u1.type_info = DataType::Object as u32;
        _self.value.obj = val;

        Ok(())
    }
}

impl TryFrom<&Zval> for ZendLong {
    type Error = ();
    fn try_from(value: &Zval) -> Result<Self, Self::Error> {
        match value.long() {
            Some(val) => Ok(val),
            _ => Err(()),
        }
    }
}

impl TryFrom<&Zval> for bool {
    type Error = ();
    fn try_from(value: &Zval) -> Result<Self, Self::Error> {
        match value.bool() {
            Some(val) => Ok(val),
            _ => Err(()),
        }
    }
}

impl TryFrom<&Zval> for f64 {
    type Error = ();
    fn try_from(value: &Zval) -> Result<Self, Self::Error> {
        match value.double() {
            Some(val) => Ok(val),
            _ => Err(()),
        }
    }
}

impl TryFrom<&Zval> for String {
    type Error = ();
    fn try_from(value: &Zval) -> Result<Self, Self::Error> {
        match value.string() {
            Some(val) => Ok(val),
            _ => Err(()),
        }
    }
}
