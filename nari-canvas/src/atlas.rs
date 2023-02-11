use nari_gpu as gpu;
use nari_ochre::TILE_SIZE;

const PAGE_WIDTH: usize = 1024 / TILE_SIZE;
const PAGE_HEIGHT: usize = 1024 / TILE_SIZE;
const PAGE_TILES: usize = PAGE_WIDTH * PAGE_HEIGHT;

#[derive(Debug, Copy, Clone)]
pub struct AtlasTile {
    pub page: u16,
    pub tx: u8,
    pub ty: u8,
}

pub struct AtlasPage {
    pub image: gpu::Image,
    pub view: gpu::ImageView,
    pub address: gpu::ImageAddress,
    next_tile: usize,
}

#[derive(Default)]
pub struct Atlas {
    pub pages: Vec<AtlasPage>,
    pub sampler: gpu::Sampler,
}

impl Atlas {
    fn has_free_tile(&self) -> bool {
        match self.pages.last() {
            Some(page) => page.next_tile < PAGE_TILES,
            None => false,
        }
    }

    pub fn allocate(&mut self, gpu: &mut gpu::Gpu, pool: gpu::Pool) -> AtlasTile {
        if !self.has_free_tile() {
            let format = gpu::Format::R8_UINT;
            let image = unsafe {
                gpu.create_image_gpu(
                    "canvas::atlas-page",
                    &gpu::ImageDesc {
                        ty: gpu::ImageType::TYPE_2D,
                        format,
                        extent: gpu::Extent3D {
                            width: (PAGE_WIDTH * TILE_SIZE) as _,
                            height: (PAGE_HEIGHT * TILE_SIZE) as _,
                            depth: 1,
                        },
                        usage: gpu::ImageUsageFlags::TRANSFER_DST | gpu::ImageUsageFlags::SAMPLED,
                        mip_levels: 1,
                        array_layers: 1,
                        samples: 1,
                    },
                    gpu::ImageInit::None,
                )
                .unwrap()
            };
            let view = {
                let mut view_usage = gpu::vk::ImageViewUsageCreateInfo::default()
                    .usage(gpu::vk::ImageUsageFlags::SAMPLED);
                let desc = gpu::vk::ImageViewCreateInfo::default()
                    .image(image.image)
                    .view_type(gpu::vk::ImageViewType::TYPE_2D)
                    .format(format)
                    .subresource_range(gpu::vk::ImageSubresourceRange {
                        aspect_mask: gpu::vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    })
                    .push_next(&mut view_usage);

                unsafe { gpu.create_image_view(&desc, None).unwrap() }
            };
            let address = unsafe { gpu.sampled_image_address(view, self.sampler) };

            let layout = gpu::ImageLayout::GENERAL;

            // todo: wrong! -> split copy and shader access
            unsafe {
                gpu.cmd_barriers(
                    pool,
                    &[],
                    &[gpu::ImageBarrier {
                        image: image.aspect(gpu::vk::ImageAspectFlags::COLOR),
                        src: gpu::ImageAccess {
                            access: gpu::Access::NONE,
                            stage: gpu::Stage::NONE,
                            layout: gpu::ImageLayout::UNDEFINED,
                        },
                        dst: gpu::ImageAccess {
                            access: gpu::Access::TRANSFER_WRITE,
                            stage: gpu::Stage::COPY,
                            layout,
                        },
                    }],
                );
            }

            self.pages.push(AtlasPage {
                image,
                view,
                address,
                next_tile: 0,
            });
        }

        let page = self.pages.last_mut().unwrap();
        let tile_id = page.next_tile;
        page.next_tile += 1;

        AtlasTile {
            page: (self.pages.len() - 1) as _,
            tx: (tile_id % PAGE_WIDTH) as _,
            ty: (tile_id / PAGE_WIDTH) as _,
        }
    }
}
