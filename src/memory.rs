#[repr(C)]
struct ExlMemoryRange {
    start: usize,
    size: usize,
}

#[repr(C)]
pub struct ModuleMemory {
    total: ExlMemoryRange,
    text: ExlMemoryRange,
    rodata: ExlMemoryRange,
    data: ExlMemoryRange,
    bss: ExlMemoryRange,
    module_header: *const crate::rtld::ModuleHeader,
    module_object: *mut crate::rtld::ModuleObject,
}

impl ModuleMemory {
    /// Gets the text section of the module memory as a slice of bytes
    pub fn text(&self) -> &'static [u8] {
        unsafe {
            std::slice::from_raw_parts(
                self.text.start as *const u8,
                self.text.size
            )
        }
    }

    /// Gets the read-only data section of the module memory as a slice of bytes
    pub fn rodata(&self) -> &'static [u8] {
        unsafe {
            std::slice::from_raw_parts(
                self.rodata.start as *const u8,
                self.rodata.size
            )
        }
    }

    /// Gets the read-write data section of the module memory as a slice of bytes
    pub fn data(&self) -> &'static [u8] {
        unsafe {
            std::slice::from_raw_parts(
                self.data.start as *const u8,
                self.data.size
            )
        }
    }

    /// Gets the read-write data section of the module memory as a mutable slice of bytes
    pub fn data_mut(&self) -> &'static mut [u8] {
        unsafe {
            std::slice::from_raw_parts_mut(
                self.data.start as *mut u8,
                self.data.size
            )
        }
    }

    /// Gets the zero-initialized sub-section of the data section as a slice of bytes
    pub fn bss(&self) -> &'static [u8] {
        unsafe {
            std::slice::from_raw_parts(
                self.bss.start as *const u8,
                self.bss.size
            )
        }
    }

    /// Gets the zero-initialized sub-section of the data section as a mutable slice of bytes
    pub fn bss_mut(&self) -> &'static mut [u8] {
        unsafe {
            std::slice::from_raw_parts_mut(
                self.bss.start as *mut u8,
                self.bss.size
            )
        }
    }

    /// Gets a static reference to an object in the data section
    pub fn data_at_offset<T: Sized>(&self, offset: usize) -> &'static T {
        unsafe {
            &*((self.data.start + offset) as *const T)
        }
    }

    /// Gets a static mutable reference to an object in the data section
    pub fn data_at_offset_mut<T: Sized>(&self, offset: usize) -> &'static mut T {
        unsafe {
            &mut *((self.data.start + offset) as *mut T)
        }
    }

    pub fn module_header(&self) -> &crate::rtld::ModuleHeader {
        unsafe {
            &*self.module_header
        }
    }

    pub fn module_object(&self) -> &crate::rtld::ModuleObject {
        unsafe {
            &*self.module_object
        }
    }
}

#[repr(u8)]
pub enum StaticModule {
    Rtld,
    Main,
    SkylineEx,
    Sdk
}

pub fn get_module(module: StaticModule) -> &'static ModuleMemory {
    unsafe {
        ffi::skex_memory_get_known_static_module(module)
    }
}

#[doc(hidden)]
pub mod ffi {
    use super::{ModuleMemory, StaticModule};


    extern "C" {
        pub fn skex_memory_get_known_static_module(module: StaticModule) -> &'static ModuleMemory;
    
        pub fn skex_memory_get_static_module_by_name(name: *const u8) -> Option<&'static ModuleMemory>;
    }
}