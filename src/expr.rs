use std::cell::UnsafeCell;
use std::ffi::{CStr, CString};
use std::fmt;

use symengine_sys::*;

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::map::{ExpressionMap, ExpressionMapKey};
pub struct Expression {
    pub(crate) basic: UnsafeCell<basic_struct>,
}

impl Expression {
    pub fn new<T>(expr: T) -> Self
    where
        T: Into<Vec<u8>> + fmt::Display,
    {
        let expr = CString::new(expr).unwrap();

        let new = Self::default();
        unsafe {
            basic_parse(new.basic.get(), expr.as_ptr());
        }
        new
    }

    fn from_value<T>(
        f: unsafe extern "C" fn(*mut basic_struct, T) -> CWRAPPER_OUTPUT_TYPE,
        value: T,
    ) -> Self {
        let expr = Self::default();
        unsafe {
            f(expr.basic.get(), value);
        }
        expr
    }
}

impl Default for Expression {
    fn default() -> Self {
        unsafe {
            let mut basic = std::mem::MaybeUninit::uninit();
            // Initialize the value properly
            basic_new_stack(basic.as_mut_ptr());

            let basic = basic.assume_init();

            Self {
                basic: UnsafeCell::new(basic),
            }
        }
    }
}

impl From<i64> for Expression {
    fn from(value: i64) -> Self {
        Self::from_value(integer_set_si, value)
    }
}

impl From<u64> for Expression {
    fn from(value: u64) -> Self {
        Self::from_value(integer_set_ui, value)
    }
}

impl From<f64> for Expression {
    fn from(value: f64) -> Self {
        Self::from_value(real_double_set_d, value)
    }
}

impl Clone for Expression {
    fn clone(&self) -> Self {
        let new = Expression::default();
        unsafe { basic_assign(new.basic.get(), self.basic.get()) };
        new
    }
}

impl Drop for Expression {
    fn drop(&mut self) {
        unsafe {
            basic_free_stack(self.basic.get());
        }
    }
}

impl fmt::Debug for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl Expression {
    pub fn as_str(&self) -> &str {
        let expr = unsafe { CStr::from_ptr(basic_str(self.basic.get())) };
        expr.to_str().unwrap()
    }

    fn binary_op(
        self,
        rhs: Self,
        op: unsafe extern "C" fn(
            *mut basic_struct,
            *mut basic_struct,
            *mut basic_struct,
        ) -> CWRAPPER_OUTPUT_TYPE,
    ) -> Self {
        let out = Self::default();
        unsafe {
            op(out.basic.get(), self.basic.get(), rhs.basic.get());
        }
        out
    }

    fn cmp_eq_op(
        &self,
        rhs: &Self,
        op: unsafe extern "C" fn(*mut basic_struct, *mut basic_struct) -> i32,
    ) -> bool {
        unsafe { op(self.basic.get(), rhs.basic.get()) == 1 }
    }
}

impl<T> std::ops::Add<T> for Expression
where
    T: Atomic,
{
    type Output = Self;

    fn add(self, rhs: T) -> Self {
        self.binary_op(rhs.into(), basic_add)
    }
}

impl<T> std::ops::Sub<T> for Expression
where
    T: Atomic,
{
    type Output = Self;

    fn sub(self, rhs: T) -> Self {
        self.binary_op(rhs.into(), basic_sub)
    }
}

impl<T> std::ops::Mul<T> for Expression
where
    T: Atomic,
{
    type Output = Self;

    fn mul(self, rhs: T) -> Self {
        self.binary_op(rhs.into(), basic_mul)
    }
}

impl<T> std::ops::Div<T> for Expression
where
    T: Atomic,
{
    type Output = Self;

    fn div(self, rhs: T) -> Self {
        self.binary_op(rhs.into(), basic_div)
    }
}

impl<T> PartialEq<T> for Expression
where
    T: Clone + Symbol,
{
    fn eq(&self, other: &T) -> bool {
        self.eq(&other.clone().into())
    }
}

impl PartialEq for Expression {
    fn eq(&self, other: &Self) -> bool {
        let lhs = Expression::default();
        let rhs = Expression::default();

        unsafe {
            basic_expand(lhs.basic.get(), self.basic.get());
            basic_expand(rhs.basic.get(), other.basic.get());
        }
        lhs.cmp_eq_op(&rhs, basic_eq)
    }
}
// implimenting exp function

pub trait UnaryOp {
    /// Apply a unary operation to the expression.
    fn unary_op<F>(self, op: F) -> Self
    where
        F: FnOnce(*mut symengine_sys::basic_struct, *mut symengine_sys::basic_struct);
}

impl UnaryOp for Expression {
    fn unary_op<F>(self, op: F) -> Self
    where
        F: FnOnce(*mut symengine_sys::basic_struct, *mut symengine_sys::basic_struct),
    {
        let out = Self::default();
        op(out.basic.get(), self.basic.get());
        out
    }
}

impl Expression {
    pub fn exp(self) -> Self {
        self.unary_op(|out, input| unsafe {
            symengine_sys::basic_exp(out, input);
        })
    }
}
// end of exp code block
//new methods
impl Expression {
    /// Computes the derivative of the expression with respect to `symbol`.
    pub fn diff(&self, symbol: &Expression) -> Self {
        let out = Self::default();
        unsafe {
            symengine_sys::basic_diff(out.basic.get(), self.basic.get(), symbol.basic.get());
        }
        out
    }
}
impl Expression {
    /// Substitute variables in the expression based on a map of substitutions.
    pub fn subs<K>(&self, subs: &ExpressionMap<K>) -> Self
    where
        K: ExpressionMapKey,
    {
        let out = Self::default();
        unsafe {
            symengine_sys::basic_subs(out.basic.get(), self.basic.get(), subs.get_basic_ptr());
        }
        out
    }
}
impl Expression {
    /// Numerically evaluate the symbolic expression.
    pub fn evalf(&self) -> Self {
        let out = Self::default();
        unsafe {
            symengine_sys::basic_evalf(out.basic.get(), self.basic.get(), 53, 0);
        }
        out
    }
}
impl Expression {
    /// Convert the expression to a f64 value.
    pub fn to_f64(&self) -> f64 {
        unsafe { symengine_sys::real_double_get_d(self.basic.get()) }
    }
}
//end of new methods

#[cfg(feature = "serde")]
impl Serialize for Expression {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Expression {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let expr = String::deserialize(deserializer)?;
        Ok(Self::new(expr))
    }
}

pub trait Atomic: Into<Expression> {}

impl<T: Symbol> Atomic for T {}
impl Atomic for Expression {}

pub trait Symbol: Into<Expression> {}

impl Symbol for i64 {}
impl Symbol for u64 {}
impl Symbol for f64 {}
