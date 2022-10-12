macro_rules! ffi {
    ($fn:ident()) => {
        unsafe { pyo3_ffi::$fn() }
    };

    ($fn:ident($obj1:expr)) => {
        unsafe { pyo3_ffi::$fn($obj1) }
    };

    ($fn:ident($obj1:expr, $obj2:expr)) => {
        unsafe { pyo3_ffi::$fn($obj1, $obj2) }
    };

    ($fn:ident($obj1:expr, $obj2:expr, $obj3:expr)) => {
        unsafe { pyo3_ffi::$fn($obj1, $obj2, $obj3) }
    };

    ($fn:ident($obj1:expr, $obj2:expr, $obj3:expr, $obj4:expr)) => {
        unsafe { pyo3_ffi::$fn($obj1, $obj2, $obj3, $obj4) }
    };
}

// PyObject_CallNoArgs was added to python in 3.9 but to limited API in 3.10
#[cfg(all(not(PyPy), any(Py_3_10, all(not(Py_LIMITED_API), Py_3_9))))]
macro_rules! call_method {
    ($obj1:expr, $obj2:expr) => {
        from_ptr_or_err(unsafe { pyo3_ffi::PyObject_CallMethodNoArgs($obj1, $obj2) })
    };
    ($obj1:expr, $obj2:expr, $obj3:expr) => {
        from_ptr_or_err(unsafe { pyo3_ffi::PyObject_CallMethodOneArg($obj1, $obj2, $obj3) })
    };
}

#[cfg(not(Py_3_9))]
macro_rules! call_method {
    ($obj1:expr, $obj2:expr) => {
        from_ptr_or_err(unsafe {
            pyo3_ffi::PyObject_CallMethodObjArgs(
                $obj1,
                $obj2,
                std::ptr::null_mut() as *mut pyo3_ffi::PyObject,
            )
        })
    };
    ($obj1:expr, $obj2:expr, $obj3:expr) => {
        from_ptr_or_err(unsafe {
            pyo3_ffi::PyObject_CallMethodObjArgs(
                $obj1,
                $obj2,
                $obj3,
                std::ptr::null_mut() as *mut pyo3_ffi::PyObject,
            )
        })
    };
}

// PyObject_CallNoArgs was added to python in 3.9 but to limited API in 3.10
#[cfg(all(not(PyPy), any(Py_3_10, all(not(Py_LIMITED_API), Py_3_9))))]
macro_rules! call_object {
    ($obj:expr) => {
        from_ptr_or_err(unsafe { pyo3_ffi::PyObject_CallNoArgs($obj) })
    };
}

#[cfg(not(Py_3_9))]
macro_rules! call_object {
    ($obj1:expr) => {
        from_ptr_or_err(unsafe {
            pyo3_ffi::PyObject_Call(
                $obj1,
                $crate::serializer::types::PY_TUPLE_0,
                std::ptr::null_mut() as *mut pyo3_ffi::PyObject,
            )
        })
    };
}

pub(crate) use call_method;
pub(crate) use call_object;
pub(crate) use ffi;
