use std::{alloc::Layout, ffi::CString};

use crate::{
    builders::FunctionBuilder,
    class::{ConstructorMeta, ConstructorResult, RegisteredClass},
    convert::IntoZval,
    error::{Error, Result},
    exception::PhpException,
    ffi::{
        zend_declare_class_constant, zend_declare_property, zend_do_implement_interface,
        zend_register_internal_class_ex,
    },
    flags::{ClassFlags, MethodFlags, PropertyFlags},
    types::{ZendClassObject, ZendObject, ZendStr, Zval},
    zend::{ClassEntry, ExecutionData, FunctionEntry},
};

/// Builds a class to be exported as a PHP class.
pub struct ClassBuilder {
    name: String,
    ptr: &'static mut ClassEntry,
    extends: Option<&'static ClassEntry>,
    interfaces: Vec<&'static ClassEntry>,
    methods: Vec<FunctionEntry>,
    object_override: Option<unsafe extern "C" fn(class_type: *mut ClassEntry) -> *mut ZendObject>,
    properties: Vec<(String, Zval, PropertyFlags)>,
    constants: Vec<(String, Zval)>,
}

impl ClassBuilder {
    /// Creates a new class builder, used to build classes
    /// to be exported to PHP.
    ///
    /// # Parameters
    ///
    /// * `name` - The name of the class.
    #[allow(clippy::unwrap_used)]
    pub fn new<T: Into<String>>(name: T) -> Self {
        // SAFETY: Allocating temporary class entry. Will return a null-ptr if
        // allocation fails, which will cause the program to panic (standard in
        // Rust). Unwrapping is OK - the ptr will either be valid or null.
        let ptr = unsafe {
            (std::alloc::alloc_zeroed(Layout::new::<ClassEntry>()) as *mut ClassEntry)
                .as_mut()
                .unwrap()
        };

        Self {
            name: name.into(),
            ptr,
            extends: None,
            interfaces: vec![],
            methods: vec![],
            object_override: None,
            properties: vec![],
            constants: vec![],
        }
    }

    /// Sets the class builder to extend another class.
    ///
    /// # Parameters
    ///
    /// * `parent` - The parent class to extend.
    pub fn extends(mut self, parent: &'static ClassEntry) -> Self {
        self.extends = Some(parent);
        self
    }

    /// Implements an interface on the class.
    ///
    /// # Parameters
    ///
    /// * `interface` - Interface to implement on the class.
    ///
    /// # Panics
    ///
    /// Panics when the given class entry `interface` is not an interface.
    pub fn implements(mut self, interface: &'static ClassEntry) -> Self {
        if !interface.is_interface() {
            panic!("Given class entry was not an interface.");
        }

        self.interfaces.push(interface);
        self
    }

    /// Adds a method to the class.
    ///
    /// # Parameters
    ///
    /// * `func` - The function entry to add to the class.
    /// * `flags` - Flags relating to the function. See [`MethodFlags`].
    pub fn method(mut self, mut func: FunctionEntry, flags: MethodFlags) -> Self {
        func.flags = flags.bits();
        self.methods.push(func);
        self
    }

    /// Adds a property to the class. The initial type of the property is given
    /// by the type of the given default. Note that the user can change the
    /// type.
    ///
    /// # Parameters
    ///
    /// * `name` - The name of the property to add to the class.
    /// * `default` - The default value of the property.
    /// * `flags` - Flags relating to the property. See [`PropertyFlags`].
    ///
    /// # Panics
    ///
    /// Function will panic if the given `default` cannot be converted into a
    /// [`Zval`].
    pub fn property<T: Into<String>>(
        mut self,
        name: T,
        default: impl IntoZval,
        flags: PropertyFlags,
    ) -> Self {
        let default = match default.into_zval(true) {
            Ok(default) => default,
            Err(_) => panic!("Invalid default value for property `{}`.", name.into()),
        };

        self.properties.push((name.into(), default, flags));
        self
    }

    /// Adds a constant to the class. The type of the constant is defined by the
    /// type of the given default.
    ///
    /// Returns a result containing the class builder if the constant was
    /// successfully added.
    ///
    /// # Parameters
    ///
    /// * `name` - The name of the constant to add to the class.
    /// * `value` - The value of the constant.
    pub fn constant<T: Into<String>>(mut self, name: T, value: impl IntoZval) -> Result<Self> {
        let value = value.into_zval(true)?;

        self.constants.push((name.into(), value));
        Ok(self)
    }

    /// Sets the flags for the class.
    ///
    /// # Parameters
    ///
    /// * `flags` - Flags relating to the class. See [`ClassFlags`].
    pub fn flags(mut self, flags: ClassFlags) -> Self {
        self.ptr.ce_flags = flags.bits();
        self
    }

    /// Overrides the creation of the Zend object which will represent an
    /// instance of this class.
    ///
    /// # Parameters
    ///
    /// * `T` - The type which will override the Zend object. Must implement
    ///   [`RegisteredClass`]
    /// which can be derived using the [`php_class`](crate::php_class) attribute
    /// macro.
    ///
    /// # Panics
    ///
    /// Panics if the class name associated with `T` is not the same as the
    /// class name specified when creating the builder.
    pub fn object_override<T: RegisteredClass>(mut self) -> Self {
        extern "C" fn create_object<T: RegisteredClass>(_: *mut ClassEntry) -> *mut ZendObject {
            // SAFETY: After calling this function, PHP will always call the constructor
            // defined below, which assumes that the object is uninitialized.
            let obj = unsafe { ZendClassObject::<T>::new_uninit() };
            obj.into_raw().get_mut_zend_obj()
        }

        extern "C" fn constructor<T: RegisteredClass>(ex: &mut ExecutionData, _: &mut Zval) {
            let ConstructorMeta { constructor, .. } = match T::CONSTRUCTOR {
                Some(c) => c,
                None => {
                    PhpException::default("You cannot instantiate this class from PHP.".into())
                        .throw()
                        .expect("Failed to throw exception when constructing class");
                    return;
                }
            };

            let this = match constructor(ex) {
                ConstructorResult::Ok(this) => this,
                ConstructorResult::Exception(e) => {
                    e.throw()
                        .expect("Failed to throw exception while constructing class");
                    return;
                }
                ConstructorResult::ArgError => return,
            };
            let this_obj = match ex.get_object::<T>() {
                Some(obj) => obj,
                None => {
                    PhpException::default("Failed to retrieve reference to `this` object.".into())
                        .throw()
                        .expect("Failed to throw exception while constructing class");
                    return;
                }
            };
            this_obj.initialize(this);
        }

        debug_assert_eq!(
            self.name.as_str(),
            T::CLASS_NAME,
            "Class name in builder does not match class name in `impl RegisteredClass`."
        );
        self.object_override = Some(create_object::<T>);
        self.method(
            {
                let mut func = FunctionBuilder::new("__construct", constructor::<T>);
                if let Some(ConstructorMeta { build_fn, .. }) = T::CONSTRUCTOR {
                    func = build_fn(func);
                }
                func.build().expect("Failed to build constructor function")
            },
            MethodFlags::Public,
        )
    }

    /// Builds the class, returning a reference to the class entry.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] variant if the class could not be registered.
    pub fn build(mut self) -> Result<&'static mut ClassEntry> {
        self.ptr.name = ZendStr::new_interned(&self.name, true)?.into_raw();

        self.methods.push(FunctionEntry::end());
        let func = Box::into_raw(self.methods.into_boxed_slice()) as *const FunctionEntry;
        self.ptr.info.internal.builtin_functions = func;

        let class = unsafe {
            zend_register_internal_class_ex(
                self.ptr,
                match self.extends {
                    Some(ptr) => (ptr as *const _) as *mut _,
                    None => std::ptr::null_mut(),
                },
            )
            .as_mut()
            .ok_or(Error::InvalidPointer)?
        };

        // SAFETY: We allocated memory for this pointer in `new`, so it is our job to
        // free it when the builder has finished.
        unsafe {
            std::alloc::dealloc((self.ptr as *mut _) as *mut u8, Layout::new::<ClassEntry>())
        };

        for iface in self.interfaces {
            unsafe { zend_do_implement_interface(class, std::mem::transmute(iface)) };
        }

        for (name, mut default, flags) in self.properties {
            unsafe {
                zend_declare_property(
                    class,
                    CString::new(name.as_str())?.as_ptr(),
                    name.len() as _,
                    &mut default,
                    flags.bits() as _,
                );
            }
        }

        for (name, value) in self.constants {
            let value = Box::into_raw(Box::new(value));
            unsafe {
                zend_declare_class_constant(
                    class,
                    CString::new(name.as_str())?.as_ptr(),
                    name.len() as u64,
                    value,
                )
            };
        }

        if let Some(object_override) = self.object_override {
            class.__bindgen_anon_2.create_object = Some(object_override);
        }

        Ok(class)
    }
}
