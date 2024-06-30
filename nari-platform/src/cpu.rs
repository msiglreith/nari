use std::ptr::null_mut;

use windows_sys::Win32::System::SystemInformation::{
    GetLogicalProcessorInformation, RelationProcessorCore, RelationProcessorPackage,
    SYSTEM_LOGICAL_PROCESSOR_INFORMATION,
};

///
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Vendor {
    Intel,
    AMD,
    Unknown,
}

///
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceProperties {
    /// Device Hardware Vendor
    pub vendor: Vendor,
    /// Name of the device.
    pub device: String,
    /// Number of logical cores.
    pub logical_cores: usize,
    /// Number of physical cores.
    pub physical_cores: usize,
}

impl DeviceProperties {
    fn system_cpuid_vendor() -> Vendor {
        let brand = {
            let cpuid = unsafe { std::arch::x86_64::__cpuid(0) };
            let mut data = [0u8; 12];
            data[0..4].copy_from_slice(unsafe { &std::mem::transmute::<_, [u8; 4]>(cpuid.ebx) });
            data[4..8].copy_from_slice(unsafe { &std::mem::transmute::<_, [u8; 4]>(cpuid.edx) });
            data[8..12].copy_from_slice(unsafe { &std::mem::transmute::<_, [u8; 4]>(cpuid.ecx) });
            data
        };

        match &brand {
            b"AuthenticAMD" => Vendor::AMD,
            b"GenuineIntel" => Vendor::Intel,
            _ => Vendor::Unknown,
        }
    }

    fn system_cpuid_vendor_device() -> (Vendor, String) {
        let vendor = Self::system_cpuid_vendor();
        let device = match vendor {
            Vendor::AMD => {
                let name = {
                    let extract = |v: u32| -> [char; 4] {
                        [
                            (v & 0xFF) as u8 as _,
                            ((v >> 8) & 0xFF) as u8 as _,
                            ((v >> 16) & 0xFF) as u8 as _,
                            ((v >> 24) & 0xFF) as u8 as _,
                        ]
                    };

                    let mut name = String::new();
                    'name: for i in 2..=4 {
                        let raw = unsafe { std::arch::x86_64::__cpuid(0x80000000 + i) };

                        let chars = [
                            extract(raw.eax),
                            extract(raw.ebx),
                            extract(raw.ecx),
                            extract(raw.edx),
                        ];

                        for quad in &chars {
                            for c in quad {
                                if *c == '\0' {
                                    break 'name;
                                }

                                name.push(*c);
                            }
                        }
                    }
                    name
                };

                name.trim_end().to_owned()
            }
            _ => String::new(),
        };

        (vendor, device)
    }

    pub fn query() -> Self {
        let (vendor, device) = Self::system_cpuid_vendor_device();

        let processor_desc = {
            let mut length = 0;

            unsafe {
                GetLogicalProcessorInformation(null_mut(), &mut length);
            }

            let info_size = std::mem::size_of::<SYSTEM_LOGICAL_PROCESSOR_INFORMATION>() as u32;
            assert_eq!(length % info_size, 0);
            let num_infos = length / info_size;

            let mut infos = Vec::with_capacity(num_infos as _);
            unsafe {
                GetLogicalProcessorInformation(infos.as_mut_ptr(), &mut length);
            }
            unsafe {
                infos.set_len(num_infos as _);
            }

            infos
        };

        let mut logical_cores = 0;
        let mut physical_cores = 0;

        for desc in processor_desc {
            #[allow(non_upper_case_globals)]
            match desc.Relationship {
                RelationProcessorCore => {
                    physical_cores += 1;
                }
                RelationProcessorPackage => {
                    logical_cores += desc.ProcessorMask.count_ones() as usize;
                }
                _ => (),
            }
        }

        Self {
            vendor,
            device,
            logical_cores,
            physical_cores,
        }
    }
}
