use std::fmt;

macro_rules! cpu_disp_impl {
    ($self:ident, $impl:ident, $f:expr) => {
        if $f.alternate() {
            write!($f, "{{ x: ")?;
            fmt::$impl::fmt(&$self.x(), $f)?;
            write!($f, ", w: ")?;
            fmt::$impl::fmt(&$self.w(), $f)?;
            write!($f, " }}")
        } else {
            fmt::$impl::fmt(&$self.x(), $f)
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct CpuRegister(u64);

impl CpuRegister {
    /// Returns the Aarch64 64-bit representation of this register
    pub fn x(self) -> u64 {
        self.0
    }

    /// Returns the Aarch64 32-bit representation of this register
    /// 
    /// This is equivalent to [`CpuRegister::r`]
    pub fn w(self) -> u32 {
        (self.0 & 0xFFFF_FFFF) as u32
    }

    /// Returns the Aarch32 32-bit representation of this register
    /// 
    /// This is equivalent to [`CpuRegister::w`]
    pub fn r(self) -> u32 {
        self.w()
    }

    /// Sets the Aarch64 64-bit representation of this register
    pub fn set_x(&mut self, x: u64) {
        self.0 = x;
    }

    /// Sets the Aarch64 32-bit representation of this register
    /// 
    /// This is equivalent to [`CpuRegister::set_r`]
    pub fn set_w(&mut self, w: u32) {
        self.0 = w as u64;
    }

    /// Sets the Aarch32 32-bit representation of this register
    /// 
    /// This is equivalent to [`CpuRegister::set_w`]
    pub fn set_r(&mut self, r: u32) {
        self.0 = r as u64;
    }
}

impl fmt::Debug for CpuRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CpuRegister")
            .field("x", &self.x())
            .field("w", &self.w())
            .field("r", &self.r())
            .finish()
    }
}

impl fmt::Binary for CpuRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        cpu_disp_impl!(self, Binary, f)
    }
}

impl fmt::LowerExp for CpuRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        cpu_disp_impl!(self, LowerExp, f)
    }
}

impl fmt::LowerHex for CpuRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        cpu_disp_impl!(self, LowerHex, f)
    }
}

impl fmt::Octal for CpuRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        cpu_disp_impl!(self, Octal, f)
    }
}

impl fmt::UpperExp for CpuRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        cpu_disp_impl!(self, UpperExp, f)
    }
}

impl fmt::UpperHex for CpuRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        cpu_disp_impl!(self, UpperHex, f)
    }
}

impl fmt::Display for CpuRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        cpu_disp_impl!(self, Display, f)
    }
}

// VECTOR REGISTERS

macro_rules! vector_disp_array {
    ($array:ident, $impl:ident, $f:expr) => {
        for (idx, el) in $array.iter().enumerate() {
            fmt::$impl::fmt(el, $f)?;
            if idx + 1 != $array.len() {
                write!($f, ", ")?;
            }
        }
    }
}

macro_rules! vector_int_disp_impl {
    ($self:ident, $impl:ident, $f:expr) => {
        if $f.alternate() {
            write!($f, "{{ v: ")?;
            fmt::$impl::fmt(&$self.v(), $f)?;
            write!($f, ", h: [")?;
            let h = $self.h();
            vector_disp_array!(h, $impl, $f);
            write!($f, "], b: [")?;
            let b = $self.b();
            vector_disp_array!(b, $impl, $f);
            write!($f, "] }}")
        } else {
            fmt::$impl::fmt(&$self.v(), $f)
        }
    }
}

macro_rules! vector_fp_disp_impl {
    ($self:ident, $impl:ident, $f:expr) => {
        if $f.alternate() {
            write!($f, "{{ d: [")?;
            let d = $self.d();
            vector_disp_array!(d, $impl, $f);
            write!($f, "], s: [")?;
            let s = $self.s();
            vector_disp_array!(s, $impl, $f);
            write!($f, " }}")
        } else {
            let s = $self.s();
            write!($f, "[")?;
            vector_disp_array!(s, $impl, $f);
            write!($f, "]")
        }
    }
}

/// A structure to represent one of the Aarch64 NEON/SIMD registers.
/// 
/// There are 32 128-bit SIMD registers on Aarch64 systems, and they can be split into "lanes".
/// Each lane must be  8 * 2^n bits, where 0 <= n <= 4
/// 
/// It is common to pack structures such as 3-float Vectors, 4-float Vectors, 2-double Vectors, etc.
/// on to a singular SIMD register for easy addition/multiplication of the components.
/// 
/// The lanes are overlapping and can be set without disrupting the other lanes. For example, one
/// could divy up the 128-bits into three lanes: 32-bit, 32-bit, 64-bit. These lanes would be referenced
/// as `S[0]`, `S[1]`, and `D[1]` respectively.
/// 
/// The [`VectorRegister`] shares the same locations as the [`FpuRegister`], which only ever references the first
/// lane of the vector representation
#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct VectorRegister(u128);

impl VectorRegister {
    /// Returns the 128-bit representation of this register
    pub fn v(self) -> u128 {
        self.0
    }

    /// Returns the two 64-bit components of this register as [`f64`] values
    pub fn d(self) -> [f64; 2] {
        unsafe {
            [
                *(&self as *const Self as *const f64).add(0),
                *(&self as *const Self as *const f64).add(1),
            ]
        }
    }

    /// Returns the four 32-bit components of this register as [`f32`] values
    pub fn s(self) -> [f32; 4] {
        unsafe {
            [
                *(&self as *const Self as *const f32).add(0),
                *(&self as *const Self as *const f32).add(1),
                *(&self as *const Self as *const f32).add(2),
                *(&self as *const Self as *const f32).add(3),
            ]
        }
    }

    /// Returns the eight 16-bit components of this register
    pub fn h(self) -> [u16; 8] {
        unsafe {
            [
                *(&self as *const Self as *const u16).add(0),
                *(&self as *const Self as *const u16).add(1),
                *(&self as *const Self as *const u16).add(2),
                *(&self as *const Self as *const u16).add(3),
                *(&self as *const Self as *const u16).add(4),
                *(&self as *const Self as *const u16).add(5),
                *(&self as *const Self as *const u16).add(6),
                *(&self as *const Self as *const u16).add(7),
            ]
        }
    }

    /// Returns the sixteen 8-bit components of this register
    pub fn b(self) -> [u8; 16] {
        unsafe {
            [
                *(&self as *const Self as *const u8).add(0),
                *(&self as *const Self as *const u8).add(1),
                *(&self as *const Self as *const u8).add(2),
                *(&self as *const Self as *const u8).add(3),
                *(&self as *const Self as *const u8).add(4),
                *(&self as *const Self as *const u8).add(5),
                *(&self as *const Self as *const u8).add(6),
                *(&self as *const Self as *const u8).add(7),
                *(&self as *const Self as *const u8).add(8),
                *(&self as *const Self as *const u8).add(9),
                *(&self as *const Self as *const u8).add(10),
                *(&self as *const Self as *const u8).add(11),
                *(&self as *const Self as *const u8).add(12),
                *(&self as *const Self as *const u8).add(13),
                *(&self as *const Self as *const u8).add(14),
                *(&self as *const Self as *const u8).add(15),
            ]
        }
    }

    /// Sets all 128 bits of the vector register
    pub fn set_v(&mut self, v: u128) {
        self.0 = v;
    }

    /// Sets the specified 64-bit lane of this register (other 64-bits are unmodified)
    pub fn set_d(&mut self, index: usize, d: f64) {
        unsafe {
            std::slice::from_raw_parts_mut(self as *mut Self as *mut f64, 2)[index] = d;
        }
    }

    /// Sets the specified 32-bit lane of this register (other lanes are unmodified)
    pub fn set_s(&mut self, index: usize, s: f32) {
        unsafe {
            std::slice::from_raw_parts_mut(self as *mut Self as *mut f32, 4)[index] = s;
        }
    }

    /// Sets the specified 16-bit lane of this register (other lanes are unmodified)
    pub fn set_h(&mut self, index: usize, h: u16) {
        unsafe {
            std::slice::from_raw_parts_mut(self as *mut Self as *mut u16, 8)[index] = h;
        }
    }

    /// Sets the specified 8-bit lane of this register (other lanes are unmodified)
    pub fn set_b(&mut self, index: usize, b: u8) {
        unsafe {
            std::slice::from_raw_parts_mut(self as *mut Self as *mut u8, 16)[index] = b;
        }
    }
}

impl fmt::Debug for VectorRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VectorRegister")
            .field("v", &self.v())
            .field("d", &self.d())
            .field("s", &self.s())
            .field("h", &self.h())
            .field("b", &self.b())
            .finish()
    }
}

impl fmt::Binary for VectorRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        vector_int_disp_impl!(self, Binary, f)
    }
}

impl fmt::LowerExp for VectorRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        vector_fp_disp_impl!(self, LowerExp, f)
    }
}

impl fmt::LowerHex for VectorRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        vector_int_disp_impl!(self, LowerHex, f)
    }
}

impl fmt::Octal for VectorRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        vector_int_disp_impl!(self, Octal, f)
    }
}

impl fmt::UpperExp for VectorRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        vector_fp_disp_impl!(self, UpperExp, f)
    }
}

impl fmt::UpperHex for VectorRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        vector_int_disp_impl!(self, UpperHex, f)
    }
}

impl fmt::Display for VectorRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        vector_fp_disp_impl!(self, Display, f)
    }
}

macro_rules! fpu_fp_disp_impl {
    ($self:ident, $impl:ident, $f:expr) => {
        if $f.alternate() {
            write!($f, "{{ d: ")?;
            fmt::$impl::fmt(&$self.d(), $f)?;
            write!($f, ", s: ")?;
            fmt::$impl::fmt(&$self.s(), $f)?;
            write!($f, " }}")
        } else {
            fmt::$impl::fmt(&$self.s(), $f)
        }
    }
}

macro_rules! fpu_int_disp_impl {
    ($self:ident, $impl:ident, $f:expr) => {
        if $f.alternate() {
            write!($f, "{{ q: ")?;
            fmt::$impl::fmt(&$self.q(), $f)?;
            write!($f, ", h: ")?;
            fmt::$impl::fmt(&$self.h(), $f)?;
            write!($f, ", b: ")?;
            fmt::$impl::fmt(&$self.b(), $f)?;
            write!($f, " }}")
        } else {
            fmt::$impl::fmt(&$self.q(), $f)
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct FpuRegister(u128);

impl FpuRegister {
    /// Copies this register into a [`VectorRegister`]
    pub fn as_vec(self) -> VectorRegister {
        VectorRegister(self.0)
    }

    /// Transforms the view of this register into a [`VectorRegister`] view.
    pub fn as_vec_mut<'a>(&'a mut self) -> &'a mut VectorRegister {
        unsafe {
            std::mem::transmute(self)
        }
    }

    /// Gets the 128-bit representation of this register
    pub fn q(self) -> u128 {
        self.0
    }

    /// Gets the 64-bit representation of this register as an [`f64`]
    pub fn d(self) -> f64 {
        unsafe {
            *(&self.0 as *const u128 as *const f64)
        }
    }

    /// Gets the 32-bit representation of this register as an [`f32`]
    pub fn s(self) -> f32 {
        unsafe {
            *(&self.0 as *const u128 as *const f32)
        }
    }

    /// Gets the 16-bit representation of this register
    pub fn h(self) -> u16 {
        unsafe {
            *(&self.0 as *const u128 as *const u16)
        }
    }

    /// Gets the 8-bit representation of this register
    pub fn b(self) -> u8 {
        unsafe {
            *(&self.0 as *const u128 as *const u8)
        }
    }

    /// Sets all 128-bits of the register
    pub fn set_q(&mut self, q: u128) {
        self.0 = q;
    }

    /// Sets the first 64-bits of the register, zeroing out the remaining bits
    pub fn set_d(&mut self, d: f64) {
        let vec = self.as_vec_mut();
        vec.set_v(0);
        vec.set_d(0, d);
    }

    /// Sets the first 32-bits of the register, zeroing out the remaining bits
    pub fn set_s(&mut self, s: f32) {
        let vec = self.as_vec_mut();
        vec.set_v(0);
        vec.set_s(0, s);
    }

    /// Sets the first 16-bits of the register, zeroing out the remaining bits
    pub fn set_h(&mut self, h: u16) {
        let vec = self.as_vec_mut();
        vec.set_v(0);
        vec.set_h(0, h);
    }

    /// Sets the first 8-bits of the register, zeroing out the remaining bits
    pub fn set_b(&mut self, b: u8) {
        let vec = self.as_vec_mut();
        vec.set_v(0);
        vec.set_b(0, b);
    }
}

impl fmt::Debug for FpuRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FpuRegister")
            .field("q", &self.q())
            .field("d", &self.d())
            .field("s", &self.s())
            .field("h", &self.h())
            .field("b", &self.b())
            .finish()
    }
}

impl fmt::Binary for FpuRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fpu_int_disp_impl!(self, Binary, f)
    }
}

impl fmt::LowerExp for FpuRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fpu_fp_disp_impl!(self, LowerExp, f)
    }
}

impl fmt::LowerHex for FpuRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fpu_int_disp_impl!(self, LowerHex, f)
    }
}

impl fmt::Octal for FpuRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fpu_int_disp_impl!(self, Octal, f)
    }
}

impl fmt::UpperExp for FpuRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fpu_fp_disp_impl!(self, UpperExp, f)
    }
}

impl fmt::UpperHex for FpuRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fpu_int_disp_impl!(self, UpperHex, f)
    }
}

impl fmt::Display for FpuRegister {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fpu_fp_disp_impl!(self, Display, f)
    }
}