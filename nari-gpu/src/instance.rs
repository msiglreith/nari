use ash::{ext, khr, vk};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use std::ffi::CStr;

pub struct Instance {
    #[allow(dead_code)]
    pub entry: ash::Entry,
    pub instance: ash::Instance,
    pub surface: Option<ash::vk::SurfaceKHR>,
    pub surface_instance: khr::surface::Instance,
    pub physical_device: ash::vk::PhysicalDevice,
    pub device_id: usize,
    pub family_index: u32,
    instance_extensions: Vec<vk::ExtensionProperties>,
    device_extensions: Vec<vk::ExtensionProperties>,
    pub device_properties: vk::PhysicalDeviceProperties,
}

impl Instance {
    pub unsafe fn with_surface(
        window: &(impl HasWindowHandle + HasDisplayHandle),
    ) -> anyhow::Result<Self> {
        Self::new(Some((
            window.window_handle()?.as_raw(),
            window.display_handle()?.as_raw(),
        )))
    }

    pub unsafe fn headless() -> anyhow::Result<Self> {
        Self::new(None)
    }

    unsafe fn new(
        surface_handle: Option<(RawWindowHandle, RawDisplayHandle)>,
    ) -> anyhow::Result<Self> {
        let entry = ash::Entry::load()?;
        let instance_extensions = entry.enumerate_instance_extension_properties(None)?;
        let supports_extension = |extension| -> bool {
            instance_extensions
                .iter()
                .any(|ext| CStr::from_ptr(ext.extension_name.as_ptr()) == extension)
        };

        let mut extensions = Vec::default();
        if let Some((_, display)) = surface_handle {
            let surface_extensions = ash_window::enumerate_required_extensions(display)?;
            extensions.extend(surface_extensions);
        }

        let supports_debug_utils = supports_extension(ext::debug_utils::NAME);
        if supports_debug_utils {
            extensions.push(ext::debug_utils::NAME.as_ptr());
        }

        let app_desc = vk::ApplicationInfo::default().api_version(vk::make_api_version(0, 1, 3, 0));
        let instance_desc = vk::InstanceCreateInfo::default()
            .application_info(&app_desc)
            .enabled_extension_names(&extensions);
        let instance = entry.create_instance(&instance_desc, None)?;

        let surface = if let Some((window, display)) = surface_handle {
            Some(ash_window::create_surface(
                &entry, &instance, display, window, None,
            )?)
        } else {
            None
        };
        let surface_instance = khr::surface::Instance::new(&entry, &instance);

        let (physical_device, device_id, family_index, _family_properties) = instance
            .enumerate_physical_devices()?
            .into_iter()
            .enumerate()
            .find_map(|(device_id, device)| {
                instance
                    .get_physical_device_queue_family_properties(device)
                    .into_iter()
                    .enumerate()
                    .find(|(i, family)| {
                        let universal = family
                            .queue_flags
                            .contains(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE);
                        if !universal {
                            return false;
                        }

                        if let Some(surface) = surface {
                            let surface_support = surface_instance
                                .get_physical_device_surface_support(device, *i as _, surface)
                                .unwrap();
                            if !surface_support {
                                return false;
                            }
                        }

                        true
                    })
                    .map(|(index, family)| (device, device_id, index as u32, family))
            })
            .unwrap();

        let device_extensions = instance.enumerate_device_extension_properties(physical_device)?;
        let device_properties = instance.get_physical_device_properties(physical_device);

        Ok(Instance {
            entry,
            instance,
            surface,
            surface_instance,
            physical_device,
            device_id,
            family_index,
            instance_extensions,
            device_extensions,
            device_properties,
        })
    }

    pub unsafe fn supports_instance_extension(&self, extension: &CStr) -> bool {
        self.instance_extensions
            .iter()
            .any(|ext| CStr::from_ptr(ext.extension_name.as_ptr()) == extension)
    }

    pub unsafe fn supports_device_extension(&self, extension: &CStr) -> bool {
        self.device_extensions
            .iter()
            .any(|ext| CStr::from_ptr(ext.extension_name.as_ptr()) == extension)
    }
}
