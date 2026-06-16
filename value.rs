use std::collections::BTreeMap;
use std::sync::Arc;

use crate::bridge::BuiltinFunction;

pub type ClosureFunc = extern "C" fn(*const *const Value, usize) -> *mut Value;

#[repr(C)]
#[derive(Clone, Debug)]
pub enum Value {
    I32(i32),
    I64(i64),
    F64(f64),
    Bool(bool),
    Unit,
    Str(Arc<str>),
    Array(Arc<[Value]>),
    Record(Arc<BTreeMap<String, Value>>),
    Closure(Arc<ClosureData>),
    Tag { tag: u32, payload: Arc<[Value]> },
    Effect(Arc<dyn BuiltinFunction>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClosureData {
    pub func_ptr: usize,
    pub captures: Arc<[Value]>,
}

impl ClosureData {
    pub fn new(func_ptr: usize, captures: Arc<[Value]>) -> Self {
        Self { func_ptr, captures }
    }

    pub fn call(&self, args: &[Value]) -> Result<Value, String> {
        if self.func_ptr == 0 {
            return Err("closure has a null function pointer".to_owned());
        }

        let mut call_values = Vec::with_capacity(self.captures.len() + args.len());
        call_values.extend(self.captures.iter().cloned());
        call_values.extend_from_slice(args);
        let call_args = call_values
            .iter()
            .map(|value| value as *const Value)
            .collect::<Vec<_>>();

        let function: ClosureFunc = unsafe { std::mem::transmute(self.func_ptr) };
        let result = function(call_args.as_ptr(), call_args.len());
        if result.is_null() {
            Err("closure returned a null value pointer".to_owned())
        } else {
            Ok(unsafe { *Box::from_raw(result) })
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::I32(lhs), Self::I32(rhs)) => lhs == rhs,
            (Self::I64(lhs), Self::I64(rhs)) => lhs == rhs,
            (Self::F64(lhs), Self::F64(rhs)) => lhs == rhs,
            (Self::Bool(lhs), Self::Bool(rhs)) => lhs == rhs,
            (Self::Unit, Self::Unit) => true,
            (Self::Str(lhs), Self::Str(rhs)) => lhs == rhs,
            (Self::Array(lhs), Self::Array(rhs)) => lhs.as_ref() == rhs.as_ref(),
            (Self::Record(lhs), Self::Record(rhs)) => lhs == rhs,
            (Self::Closure(lhs), Self::Closure(rhs)) => Arc::ptr_eq(lhs, rhs),
            (
                Self::Tag {
                    tag: lhs_tag,
                    payload: lhs_payload,
                },
                Self::Tag {
                    tag: rhs_tag,
                    payload: rhs_payload,
                },
            ) => lhs_tag == rhs_tag && lhs_payload.as_ref() == rhs_payload.as_ref(),
            (Self::Effect(lhs), Self::Effect(rhs)) => Arc::ptr_eq(lhs, rhs),
            _ => false,
        }
    }
}

impl Value {
    pub fn i32(value: i32) -> Self {
        Self::I32(value)
    }

    pub fn i64(value: i64) -> Self {
        Self::I64(value)
    }

    pub fn f64(value: f64) -> Self {
        Self::F64(value)
    }

    pub fn bool(value: bool) -> Self {
        Self::Bool(value)
    }

    pub fn unit() -> Self {
        Self::Unit
    }

    pub fn str(value: impl AsRef<str>) -> Self {
        Self::Str(Arc::<str>::from(value.as_ref()))
    }

    pub fn array(values: Vec<Value>) -> Self {
        Self::Array(Arc::<[Value]>::from(values))
    }

    pub fn tag(tag: u32, payload: Vec<Value>) -> Self {
        Self::Tag {
            tag,
            payload: Arc::<[Value]>::from(payload),
        }
    }

    pub fn as_i32(&self) -> i32 {
        match self {
            Self::I32(value) => *value,
            actual => panic!("expected i32 value, got {actual:?}"),
        }
    }

    pub fn as_i64(&self) -> i64 {
        match self {
            Self::I64(value) => *value,
            actual => panic!("expected i64 value, got {actual:?}"),
        }
    }

    pub fn as_f64(&self) -> f64 {
        match self {
            Self::F64(value) => *value,
            actual => panic!("expected f64 value, got {actual:?}"),
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            Self::Bool(value) => *value,
            actual => panic!("expected bool value, got {actual:?}"),
        }
    }

    pub fn is_unit(&self) -> bool {
        matches!(self, Self::Unit)
    }
}

#[no_mangle]
pub extern "C" fn pipe_value_i32(value: i32) -> *mut Value {
    Box::into_raw(Box::new(Value::I32(value)))
}

#[no_mangle]
pub extern "C" fn pipe_value_f64(value: f64) -> *mut Value {
    Box::into_raw(Box::new(Value::F64(value)))
}

#[no_mangle]
pub extern "C" fn pipe_value_bool(value: u8) -> *mut Value {
    Box::into_raw(Box::new(Value::Bool(value != 0)))
}

#[no_mangle]
pub extern "C" fn pipe_value_unit() -> *mut Value {
    Box::into_raw(Box::new(Value::Unit))
}

#[no_mangle]
pub unsafe extern "C" fn pipe_value_clone(value: *const Value) -> *mut Value {
    Box::into_raw(Box::new((*value).clone()))
}

#[no_mangle]
pub unsafe extern "C" fn pipe_value_drop(value: *mut Value) {
    if !value.is_null() {
        drop(Box::from_raw(value));
    }
}

#[no_mangle]
pub unsafe extern "C" fn pipe_value_as_i32(value: *const Value) -> i32 {
    (*value).as_i32()
}

#[no_mangle]
pub unsafe extern "C" fn pipe_value_as_f64(value: *const Value) -> f64 {
    (*value).as_f64()
}

#[no_mangle]
pub unsafe extern "C" fn pipe_value_as_bool(value: *const Value) -> u8 {
    u8::from((*value).as_bool())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_deep_equality_checks_nested_data() {
        let lhs = Value::array(vec![
            Value::I32(1),
            Value::str("pipe"),
            Value::tag(7, vec![Value::Bool(true), Value::Unit]),
        ]);
        let rhs = Value::array(vec![
            Value::I32(1),
            Value::str("pipe"),
            Value::tag(7, vec![Value::Bool(true), Value::Unit]),
        ]);

        assert_eq!(lhs, rhs);
    }

    #[test]
    fn record_equality_is_deep_and_order_independent() {
        let mut lhs = BTreeMap::new();
        lhs.insert("name".to_owned(), Value::str("Ada"));
        lhs.insert("age".to_owned(), Value::I32(36));

        let mut rhs = BTreeMap::new();
        rhs.insert("age".to_owned(), Value::I32(36));
        rhs.insert("name".to_owned(), Value::str("Ada"));

        assert_eq!(
            Value::Record(Arc::new(lhs)),
            Value::Record(Arc::new(rhs))
        );
    }

    #[test]
    fn arc_backed_values_drop_when_last_reference_drops() {
        let inner: Arc<[Value]> = Arc::from(vec![Value::I32(1)]);
        assert_eq!(Arc::strong_count(&inner), 1);

        {
            let value = Value::Array(Arc::clone(&inner));
            assert_eq!(Arc::strong_count(&inner), 2);
            let cloned = value.clone();
            assert_eq!(Arc::strong_count(&inner), 3);
            drop(cloned);
            assert_eq!(Arc::strong_count(&inner), 2);
        }

        assert_eq!(Arc::strong_count(&inner), 1);
    }

    #[test]
    fn deeply_nested_arrays_can_be_created_and_dropped() {
        let mut value = Value::Unit;
        for _ in 0..512 {
            value = Value::array(vec![value]);
        }

        drop(value);
    }
}