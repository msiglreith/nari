use crate::{Gpu, Image, Instance};
use ash::{khr, prelude::*, vk};

#[derive(Debug)]
pub struct Frame {
    pub id: usize,
    pub acquire: vk::Semaphore,
    pub present: vk::Semaphore,
}

pub struct Swapchain {
    device_id: usize,
    pub swapchain_desc: vk::SwapchainCreateInfoKHR<'static>,
    pub swapchain_device: khr::swapchain::Device,
    pub swapchain: vk::SwapchainKHR,
    pub acquire_semaphore: vk::Semaphore,
    pub frame_semaphores: Vec<vk::Semaphore>,
    pub present_semaphores: Vec<vk::Semaphore>,
    pub frame_images: Vec<Image>,
    pub frame_rtvs: Vec<vk::ImageView>,
}

impl Swapchain {
    pub unsafe fn new(
        instance: &Instance,
        device: &Gpu,
        width: u32,
        height: u32,
        present_mode: vk::PresentModeKHR,
    ) -> anyhow::Result<Self> {
        let surface = instance.surface.expect("headless instance has no surface");
        let swapchain_device = khr::swapchain::Device::new(&instance.instance, &device.device);
        let swapchain_desc = {
            let surface_capabilities = instance
                .surface_instance
                .get_physical_device_surface_capabilities(instance.physical_device, surface)?;

            // supported on all platforms we care (excludes android)
            let surface_format = vk::SurfaceFormatKHR {
                format: vk::Format::B8G8R8A8_SRGB,
                color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
            };

            vk::SwapchainCreateInfoKHR::default()
                .surface(surface)
                .min_image_count(3)
                .image_format(surface_format.format)
                .image_color_space(surface_format.color_space)
                .image_extent(vk::Extent2D { width, height })
                .image_array_layers(1)
                .image_usage(
                    vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST,
                )
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                .pre_transform(surface_capabilities.current_transform)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(present_mode)
                .clipped(true)
        };

        let swapchain = swapchain_device.create_swapchain(&swapchain_desc, None)?;

        let frame_images = swapchain_device
            .get_swapchain_images(swapchain)?
            .into_iter()
            .map(|image| Image {
                image,
                allocation: None,
                mip_levels: 1,
                array_layers: 1,
            })
            .collect::<Vec<_>>();
        let frame_semaphores = (0..frame_images.len())
            .map(|_| {
                let desc = vk::SemaphoreCreateInfo::default();
                device.create_semaphore(&desc, None)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let present_semaphores = (0..frame_images.len())
            .map(|_| {
                let desc = vk::SemaphoreCreateInfo::default();
                device.create_semaphore(&desc, None)
            })
            .collect::<Result<Vec<_>, _>>()?;

        let frame_rtvs = frame_images
            .iter()
            .map(|image| {
                let image = image.aspect(vk::ImageAspectFlags::COLOR);
                let view_desc = vk::ImageViewCreateInfo::default()
                    .image(image.image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(swapchain_desc.image_format)
                    .subresource_range(image.range);
                device.create_image_view(&view_desc, None)
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Semaphore to use for next swapchain acquire operation.
        // Will be cycled through with `frame_semaphores`.
        let acquire_semaphore = {
            let desc = vk::SemaphoreCreateInfo::default();
            device.create_semaphore(&desc, None)?
        };

        Ok(Swapchain {
            device_id: instance.device_id,
            swapchain,
            swapchain_device,
            swapchain_desc,
            acquire_semaphore,
            frame_semaphores,
            present_semaphores,
            frame_images,
            frame_rtvs,
        })
    }

    pub unsafe fn acquire(&mut self) -> VkResult<Frame> {
        let desc = vk::AcquireNextImageInfoKHR::default()
            .swapchain(self.swapchain)
            .timeout(!0)
            .fence(vk::Fence::null())
            .semaphore(self.acquire_semaphore)
            .device_mask(1u32 << self.device_id);
        let (index, _suboptimal) = self.swapchain_device.acquire_next_image2(&desc)?;
        let frame = Frame {
            id: index as usize,
            acquire: self.acquire_semaphore,
            present: self.present_semaphores[index as usize],
        };

        std::mem::swap(
            &mut self.frame_semaphores[index as usize],
            &mut self.acquire_semaphore,
        );

        VkResult::Ok(frame)
    }

    pub unsafe fn present(&mut self, device: &Gpu, frame: Frame) -> VkResult<()> {
        let present_wait = [frame.present];
        let present_swapchains = [self.swapchain];
        let present_images = [frame.id as u32];
        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&present_wait)
            .swapchains(&present_swapchains)
            .image_indices(&present_images);
        self.swapchain_device
            .queue_present(device.queue, &present_info)?;
        Ok(())
    }

    pub unsafe fn resize(&mut self, device: &Gpu, width: u32, height: u32) -> VkResult<()> {
        self.swapchain_desc.image_extent = vk::Extent2D { width, height };
        self.swapchain_desc.old_swapchain = self.swapchain;
        self.swapchain = self
            .swapchain_device
            .create_swapchain(&self.swapchain_desc, None)?;

        self.frame_images = self
            .swapchain_device
            .get_swapchain_images(self.swapchain)?
            .into_iter()
            .map(|image| Image {
                image,
                allocation: None,
                mip_levels: 1,
                array_layers: 1,
            })
            .collect::<Vec<_>>();

        self.frame_rtvs = self
            .frame_images
            .iter()
            .map(|image| {
                let image = image.aspect(vk::ImageAspectFlags::COLOR);
                let view_desc = vk::ImageViewCreateInfo::default()
                    .image(image.image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    // .format(vk::Format::B8G8R8A8_SRGB)
                    .format(self.swapchain_desc.image_format)
                    .subresource_range(image.range);
                device.create_image_view(&view_desc, None)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(())
    }
}
