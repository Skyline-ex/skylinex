use object::{elf, LittleEndian};

#[repr(C)]
pub struct ModuleHeader {
    pub magic: u32,
    pub dynamic_offset: u32,
    pub bss_start_offset: u32,
    pub bss_end_offset: u32,
    pub unwind_start_offset: u32,
    pub unwind_end_offset: u32,
    pub module_object_offset: u32,
}

impl ModuleHeader {
    pub const MOD0_MAGIC: u32 = 0x30444F4D;
}

#[repr(C)]
pub struct ModuleObject {
    pub next: *mut Self,
    pub prev: *mut Self,
    pub rela_or_rel_plt: *mut (),
    pub rela_or_rel: *mut (),
    pub module_base: *mut u8,
    pub dynamic: *mut elf::Dyn64<LittleEndian>,
    pub is_rela: bool,
    pub rela_or_rel_plt_size: u64,
    pub dt_init: extern "C" fn(),
    pub dt_fini: extern "C" fn(),
    pub hash_bucket: *mut u32,
    pub hash_chain: *mut u32,
    pub dynstr: *mut u8,
    pub dynsym: *mut elf::Sym64<LittleEndian>,
    pub dynstr_size: u64,
    pub got: *mut *mut (),
    pub rela_dyn_size: u64,
    pub rel_dyn_size: u64,
    pub rel_count: u64,
    pub rela_count: u64,
    pub hash_nchain_value: u64,
    pub hash_nbucket_value: u64,
    pub got_stub_ptr: *mut (),
    pub soname_idx: u64,
    pub nro_size: usize,
    pub cannot_revert_symbols: usize,
}

impl ModuleObject {
    pub fn get_module_name(&self) -> Option<&'static str> {
        let info = match crate::nx::query_memory(self.module_base as u64) {
            Ok(info) => info,
            Err(_) => return None
        };

        let ro_info = match crate::nx::query_memory(info.addr + info.size) {
            Ok(info) => info,
            Err(_) => return None
        };

        unsafe {
            let rw_data_offset = *(ro_info.addr as *const u32);
            if rw_data_offset as u64 + info.addr == ro_info.addr + ro_info.size {
                return None;
            }

            if rw_data_offset != 0 {
                return None;
            }

            let path_length = *(ro_info.addr as *const i32).add(1);
            if path_length <= 0 {
                return None;
            }

            let name = std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                (ro_info.addr + 8) as *const u8,
                path_length as usize,
            ));

            let split = name.split('\\').last().unwrap();
            let name = split.split('/').last().unwrap();

            Some(name)
        }
    }

    pub fn contains_address(&self, address: u64) -> bool {
        let info = match crate::nx::query_memory(self.module_base as u64) {
            Ok(info) => info,
            Err(_) => return false,
        };

        info.addr <= address && address <= (info.addr + info.size)
    }

    pub fn find_symbol_for_address(&self, address: u64) -> Option<(&'static str, u64)> {
        let symbols = unsafe {
            std::slice::from_raw_parts(self.dynsym, self.hash_nchain_value as usize)
        };

        for symbol in symbols.iter() {
            let shndx = symbol.st_shndx.get(LittleEndian);
            if shndx == 0 || (shndx & 0xFF00) == 0xFF00 {
                continue;
            }

            if symbol.st_info & 0xF != 2 {
                continue;
            }

            let function_start = unsafe { 
                self.module_base.add(symbol.st_value.get(LittleEndian) as usize) as u64 
            };
            let function_end = function_start + symbol.st_size.get(LittleEndian);
            if function_start <= address && address <= function_end {
                let mut sym_start = unsafe { self.dynstr.add(symbol.st_name.get(LittleEndian) as usize) };
                let mut len = 0;
                while unsafe { *sym_start != 0 } {
                    unsafe { sym_start = sym_start.add(1); }
                    len += 1;
                }

                return Some((
                    unsafe { 
                        std::str::from_utf8_unchecked(
                            std::slice::from_raw_parts(self.dynstr.add(symbol.st_name.get(LittleEndian) as usize), 
                            len
                        ))
                    },
                    function_start
                ))
            }
        }
        
        None
    }
}

#[repr(C)]
pub struct ModuleObjectList {
    front: *mut ModuleObject,
    back: *mut ModuleObject,
}

impl ModuleObjectList {
    pub fn iter(&self) -> ModuleObjectListIterator {
        ModuleObjectListIterator { end: self as *const ModuleObjectList as *const ModuleObject, current: self.front }
    }
}

pub struct ModuleObjectListIterator {
    end: *const ModuleObject,
    current: *const ModuleObject,
}

impl Iterator for ModuleObjectListIterator {
    type Item = &'static ModuleObject;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current == self.end {
            None
        } else {
            let next = unsafe { (*self.current).next };
            let current = unsafe { &*self.current };
            self.current = next;
            Some(current)
        }
    }
}

extern "C" {
    #[link_name = "_ZN2nn2ro6detail15g_pAutoLoadListE"]
    pub(crate) static AUTO_LOAD_LIST: &'static mut ModuleObjectList;

    #[link_name = "_ZN2nn2ro6detail17g_pManualLoadListE"]
    pub(crate) static MANUAL_LOAD_LIST: &'static mut ModuleObjectList;
}

pub fn find_module_for_address(address: u64) -> Option<&'static crate::rtld::ModuleObject> {
    for object in unsafe { crate::rtld::AUTO_LOAD_LIST.iter() } {
        if object.contains_address(address) {
            return Some(object);
        }
    }

    unsafe { crate::rtld::MANUAL_LOAD_LIST.iter().find(|object| object.contains_address(address)) }
}

pub fn find_module_by_name(name: &str) -> Option<&'static crate::rtld::ModuleObject> {
    let mut objects = unsafe {
        AUTO_LOAD_LIST.iter().chain(MANUAL_LOAD_LIST.iter())
    };

    objects
        .find(|object| object.get_module_name().unwrap_or("__invalid_name") == name)
}

pub fn get_module_for_self() -> Option<&'static ModuleObject> {
    let info = crate::nx::query_memory(get_module_for_self as *const () as u64).ok()?;
    let module_header: *const ModuleHeader = unsafe {
        (info.addr + *(info.addr as *const u32).add(1) as u64) as *const ModuleHeader
    };
    if unsafe { (*module_header).magic != ModuleHeader::MOD0_MAGIC } {
        return None;
    }
    let module_object: *const ModuleObject = unsafe {
        (module_header as u64) + (*module_header).module_object_offset as u64
    } as *const ModuleObject;

    Some(unsafe { &*module_object })
}