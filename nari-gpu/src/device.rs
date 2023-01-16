use crate as gpu;
use ash::{
    extensions::{ext, khr, nv},
    vk,
    vk::Handle,
};
use gpu_allocator::vulkan::{Allocation, Allocator, AllocatorCreateDesc};
use std::collections::{hash_map::Entry, HashMap};
use std::{
    ffi::CString,
    path::{Path, PathBuf},
};

pub struct Extensions {
    pub debug_utils: Option<ext::DebugUtils>,
    pub mesh_shader: Option<nv::MeshShader>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum PoolState {
    Free,
    Recording,
    Executing(gpu::Timestamp),
}

#[derive(Default)]
struct Shrine {
    allocations: Vec<Allocation>,
    buffers: Vec<vk::Buffer>,
    images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
}

struct PoolData {
    state: PoolState,
    cmd_pool: vk::CommandPool,
    cmd_buffer: gpu::CommandBuffer,
    shrine: Shrine,
}

#[derive(Debug, Copy, Clone)]
pub struct Pool {
    pub cmd_buffer: gpu::CommandBuffer,
    pub id: usize, //// todo: pools idx
}

impl Pool {
    pub fn null() -> Self {
        Self {
            cmd_buffer: gpu::CommandBuffer::null(),
            id: 0,
        }
    }
}

pub struct Gpu {
    pub device: ash::Device,
    pub ext: Extensions,
    pub allocator: Allocator,
    spv_dir: PathBuf,

    // Queue
    pub queue: vk::Queue,
    queue_family_index: u32,
    pools: Vec<PoolData>,
    pub timeline: vk::Semaphore,
    timeline_value: gpu::Timestamp,

    // Resources
    pub descriptor_pool: vk::DescriptorPool,
    pub descriptors_sampled_image: gpu::Descriptors,
    pub descriptors_storage_image: gpu::Descriptors,
    addresses_sampled_image: HashMap<(vk::ImageView, vk::Sampler), gpu::ImageAddress>,
    addresses_storage_image: HashMap<vk::ImageView, gpu::ImageAddress>,

    // Pipeline layouts only differ in the number of push constants.
    // As these are quite limited we can just cache them and handle it internally.
    //
    // key: num push constants
    // value: pipeline layout with fixed (global) descriptor set + (key) push constants.
    layouts: HashMap<usize, vk::PipelineLayout>,
}

impl Gpu {
    pub unsafe fn new(instance: &gpu::Instance, spv_dir: &Path) -> anyhow::Result<Self> {
        let supports_debug_utils = instance.supports_instance_extension(ext::DebugUtils::name());
        let supports_mesh_shader = instance.supports_device_extension(nv::MeshShader::name());

        let (device, queue) = {
            let mut device_extensions = vec![khr::Swapchain::name().as_ptr()];
            if supports_mesh_shader {
                device_extensions.push(nv::MeshShader::name().as_ptr());
            }

            let features = vk::PhysicalDeviceFeatures::default()
                .robust_buffer_access(true)
                .shader_storage_image_read_without_format(true)
                .shader_storage_image_write_without_format(true);
            let mut features11 = vk::PhysicalDeviceVulkan11Features::default()
                .variable_pointers(true)
                .variable_pointers_storage_buffer(true);
            let mut features12 = vk::PhysicalDeviceVulkan12Features::default()
                .timeline_semaphore(true)
                .buffer_device_address(true)
                .descriptor_indexing(true)
                .descriptor_binding_partially_bound(true)
                .runtime_descriptor_array(true)
                .shader_storage_buffer_array_non_uniform_indexing(true)
                .descriptor_binding_storage_buffer_update_after_bind(true)
                .imageless_framebuffer(true)
                .vulkan_memory_model(true);
            let mut features13 = vk::PhysicalDeviceVulkan13Features::default()
                .robust_image_access(true)
                .dynamic_rendering(true)
                .synchronization2(true);
            let mut features_mesh_shader = vk::PhysicalDeviceMeshShaderFeaturesNV::default()
                .task_shader(supports_mesh_shader)
                .mesh_shader(supports_mesh_shader);

            let queue_priorities = [1.0];
            let queue_descs = [vk::DeviceQueueCreateInfo::default()
                .queue_family_index(instance.family_index)
                .queue_priorities(&queue_priorities)];

            let device_desc = vk::DeviceCreateInfo::default()
                .queue_create_infos(&queue_descs)
                .enabled_extension_names(&device_extensions)
                .enabled_features(&features)
                .push_next(&mut features11)
                .push_next(&mut features12)
                .push_next(&mut features13)
                .push_next(&mut features_mesh_shader);

            let device =
                instance
                    .instance
                    .create_device(instance.physical_device, &device_desc, None)?;
            let queue = device.get_device_queue(instance.family_index, 0);

            (device, queue)
        };

        // extensions
        let ext_debug_utils =
            supports_debug_utils.then(|| ext::DebugUtils::new(&instance.entry, &instance.instance));
        let ext_mesh_shader =
            supports_mesh_shader.then(|| nv::MeshShader::new(&instance.instance, &device));

        const SAMPLED_IMAGE_COUNT: u32 = 64 * 1024;
        const STORAGE_IMAGE_COUNT: u32 = 64 * 1024;

        let descriptor_pool = {
            let pool_sizes = [
                vk::DescriptorPoolSize {
                    ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                    descriptor_count: SAMPLED_IMAGE_COUNT,
                },
                vk::DescriptorPoolSize {
                    ty: vk::DescriptorType::STORAGE_IMAGE,
                    descriptor_count: STORAGE_IMAGE_COUNT,
                },
            ];
            let desc = vk::DescriptorPoolCreateInfo::default()
                .max_sets(2)
                .pool_sizes(&pool_sizes)
                .flags(vk::DescriptorPoolCreateFlags::UPDATE_AFTER_BIND);
            device.create_descriptor_pool(&desc, None)?
        };

        let sampled_image_layout = {
            let binding_flags = [vk::DescriptorBindingFlags::PARTIALLY_BOUND; 1];
            let mut flag_desc = vk::DescriptorSetLayoutBindingFlagsCreateInfo::default()
                .binding_flags(&binding_flags);

            let bindings = [vk::DescriptorSetLayoutBinding::default()
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .binding(0)
                .descriptor_count(SAMPLED_IMAGE_COUNT)
                .stage_flags(vk::ShaderStageFlags::ALL)];

            let desc = vk::DescriptorSetLayoutCreateInfo::default()
                .bindings(&bindings)
                .flags(vk::DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL)
                .push_next(&mut flag_desc);
            device.create_descriptor_set_layout(&desc, None)?
        };

        let storage_image_layout = {
            let binding_flags = [vk::DescriptorBindingFlags::PARTIALLY_BOUND; 1];
            let mut flag_desc = vk::DescriptorSetLayoutBindingFlagsCreateInfo::default()
                .binding_flags(&binding_flags);

            let bindings = [vk::DescriptorSetLayoutBinding::default()
                .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
                .binding(0)
                .descriptor_count(STORAGE_IMAGE_COUNT)
                .stage_flags(vk::ShaderStageFlags::ALL)];

            let desc = vk::DescriptorSetLayoutCreateInfo::default()
                .bindings(&bindings)
                .flags(vk::DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL)
                .push_next(&mut flag_desc);
            device.create_descriptor_set_layout(&desc, None)?
        };

        let sets = {
            let layouts = [sampled_image_layout, storage_image_layout];

            let desc = vk::DescriptorSetAllocateInfo::default()
                .set_layouts(&layouts)
                .descriptor_pool(descriptor_pool);

            device.allocate_descriptor_sets(&desc)?
        };

        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance: instance.instance.clone(),
            device: device.clone(),
            physical_device: instance.physical_device,
            debug_settings: Default::default(),
            buffer_device_address: true,
        })?;

        let timeline = {
            let mut timeline_desc = vk::SemaphoreTypeCreateInfo::default()
                .semaphore_type(vk::SemaphoreType::TIMELINE)
                .initial_value(0);
            let desc = vk::SemaphoreCreateInfo::default().push_next(&mut timeline_desc);
            device.create_semaphore(&desc, None)?
        };

        let gpu_sampled_images = gpu::descriptor::GpuDescriptors {
            layout: sampled_image_layout,
            set: sets[0],
        };
        let gpu_storage_images = gpu::descriptor::GpuDescriptors {
            layout: storage_image_layout,
            set: sets[1],
        };

        let descriptors_sampled_image =
            gpu::Descriptors::new(SAMPLED_IMAGE_COUNT as _, gpu_sampled_images);
        let descriptors_storage_image =
            gpu::Descriptors::new(STORAGE_IMAGE_COUNT as _, gpu_storage_images);

        Ok(Self {
            device,
            queue,
            allocator,
            pools: Default::default(),
            timeline,
            ext: Extensions {
                debug_utils: ext_debug_utils,
                mesh_shader: ext_mesh_shader,
            },
            timeline_value: 0,
            spv_dir: spv_dir.to_path_buf(),
            descriptor_pool,
            descriptors_sampled_image,
            descriptors_storage_image,
            addresses_sampled_image: Default::default(),
            addresses_storage_image: Default::default(),
            layouts: Default::default(),
            queue_family_index: instance.family_index,
        })
    }

    pub unsafe fn buffer_address(&self, buffer: &gpu::Buffer) -> gpu::DeviceAddress {
        let desc = vk::BufferDeviceAddressInfo::default().buffer(buffer.buffer);
        self.get_buffer_device_address(&desc)
    }

    pub unsafe fn sampled_image_address(
        &mut self,
        image: gpu::ImageView,
        sampler: gpu::Sampler,
    ) -> gpu::ImageAddress {
        // todo: currently creates new descriptor per call!
        //       driver may reuse addresses! make image/sampler properly hashable
        let descriptor = self.descriptors_sampled_image.create();
        let image_infos = [vk::DescriptorImageInfo {
            sampler: sampler,
            image_view: image,
            image_layout: gpu::ImageLayout::GENERAL, // todo
        }];
        let updates = [vk::WriteDescriptorSet::default()
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .dst_set(self.descriptors_sampled_image.gpu.set)
            .dst_binding(0)
            .dst_array_element(descriptor)
            .image_info(&image_infos)];
        self.device.update_descriptor_sets(&updates, &[]);

        self.addresses_sampled_image
            .insert((image, sampler), descriptor);

        descriptor
    }

    pub unsafe fn storage_image_address(&mut self, image: gpu::ImageView) -> gpu::ImageAddress {
        match self.addresses_storage_image.entry(image) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                let descriptor = self.descriptors_storage_image.create();
                let image_infos = [vk::DescriptorImageInfo {
                    sampler: vk::Sampler::null(),
                    image_view: image,
                    image_layout: gpu::ImageLayout::GENERAL,
                }];
                let updates = [vk::WriteDescriptorSet::default()
                    .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
                    .dst_set(self.descriptors_storage_image.gpu.set)
                    .dst_binding(0)
                    .dst_array_element(descriptor)
                    .image_info(&image_infos)];
                self.device.update_descriptor_sets(&updates, &[]);

                *entry.insert(descriptor)
            }
        }
    }

    pub unsafe fn retire_buffer(&mut self, buffer: gpu::Buffer) -> anyhow::Result<()> {
        for pool in &mut self.pools {
            if pool.state == PoolState::Executing(self.timeline_value) {
                pool.shrine.allocations.push(buffer.allocation);
                pool.shrine.buffers.push(buffer.buffer);
                return Ok(());
            }
        }

        // no pool found, either before submitting anything or gpu already caught up
        self.device.destroy_buffer(buffer.buffer, None);
        self.allocator.free(buffer.allocation)?;

        return Ok(());
    }

    pub unsafe fn retire_image(&mut self, mut image: gpu::Image) -> anyhow::Result<()> {
        for pool in &mut self.pools {
            if pool.state == PoolState::Executing(self.timeline_value) {
                if let Some(allocation) = image.allocation.take() {
                    pool.shrine.allocations.push(allocation);
                }
                pool.shrine.images.push(image.image);
                return Ok(());
            }
        }

        // no pool found, either before submitting anything or gpu already caught up
        self.device.destroy_image(image.image, None);
        if let Some(allocation) = image.allocation.take() {
            self.allocator.free(allocation)?;
        }

        return Ok(());
    }

    pub unsafe fn retire_image_view(&mut self, image_view: vk::ImageView) -> anyhow::Result<()> {
        for pool in &mut self.pools {
            if pool.state == PoolState::Executing(self.timeline_value) {
                pool.shrine.image_views.push(image_view);
                return Ok(());
            }
        }

        // no pool found, either before submitting anything or gpu already caught up
        self.device.destroy_image_view(image_view, None);

        return Ok(());
    }

    pub fn cmd_retire_buffer(&mut self, pool: Pool, buffer: gpu::Buffer) {
        self.pools[pool.id]
            .shrine
            .allocations
            .push(buffer.allocation);
        self.pools[pool.id].shrine.buffers.push(buffer.buffer);
    }

    pub fn cmd_retire_image(&mut self, pool: gpu::Pool, mut image: gpu::Image) {
        if let Some(allocation) = image.allocation.take() {
            self.pools[pool.id].shrine.allocations.push(allocation);
        }
        self.pools[pool.id].shrine.images.push(image.image);
    }

    unsafe fn name_object(
        &self,
        ty: vk::ObjectType,
        handle: u64,
        name: &str,
    ) -> anyhow::Result<()> {
        if let Some(ext) = &self.ext.debug_utils {
            let name = CString::new(name)?;
            let info = vk::DebugUtilsObjectNameInfoEXT::default()
                .object_type(ty)
                .object_handle(handle)
                .object_name(&name);
            ext.set_debug_utils_object_name(self.device.handle(), &info)?;
        }
        Ok(())
    }

    pub unsafe fn create_buffer_upload(
        &mut self,
        name: &str,
        size: usize,
        mut usage: gpu::BufferUsageFlags,
    ) -> anyhow::Result<gpu::Buffer> {
        usage |= gpu::BufferUsageFlags::SHADER_DEVICE_ADDRESS;

        let buffer = {
            let desc = vk::BufferCreateInfo::default().size(size as _).usage(usage);
            let buffer = self.create_buffer(&desc, None)?;
            let alloc_desc = gpu_allocator::vulkan::AllocationCreateDesc {
                name,
                requirements: self.get_buffer_memory_requirements(buffer),
                location: gpu_allocator::MemoryLocation::CpuToGpu,
                linear: true,
                allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
            };
            let allocation = self.allocator.allocate(&alloc_desc)?;
            self.bind_buffer_memory(buffer, allocation.memory(), allocation.offset())?;
            gpu::Buffer { buffer, allocation }
        };

        self.name_object(vk::ObjectType::BUFFER, buffer.buffer.as_raw(), name)?;

        Ok(buffer)
    }

    pub unsafe fn create_buffer_gpu(
        &mut self,
        name: &str,
        size: usize,
        mut usage: gpu::BufferUsageFlags,
        initialization: gpu::BufferInit,
    ) -> anyhow::Result<gpu::Buffer> {
        usage |= gpu::BufferUsageFlags::SHADER_DEVICE_ADDRESS;
        if let gpu::BufferInit::Host { .. } = &initialization {
            usage |= gpu::BufferUsageFlags::TRANSFER_DST;
        }

        let buffer = {
            let desc = vk::BufferCreateInfo::default().size(size as _).usage(usage);
            let buffer = self.create_buffer(&desc, None)?;
            let alloc_desc = gpu_allocator::vulkan::AllocationCreateDesc {
                name,
                requirements: self.get_buffer_memory_requirements(buffer),
                location: gpu_allocator::MemoryLocation::GpuOnly,
                linear: true,
                allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
            };
            let allocation = self.allocator.allocate(&alloc_desc)?;
            self.bind_buffer_memory(buffer, allocation.memory(), allocation.offset())?;
            gpu::Buffer { buffer, allocation }
        };

        self.name_object(vk::ObjectType::BUFFER, buffer.buffer.as_raw(), name)?;

        match initialization {
            gpu::BufferInit::Host { pool, data } => {
                let buffer_init = {
                    let desc = vk::BufferCreateInfo::default()
                        .size(data.len() as _)
                        .usage(vk::BufferUsageFlags::TRANSFER_SRC);
                    let buffer = self.create_buffer(&desc, None)?;
                    let alloc_desc = gpu_allocator::vulkan::AllocationCreateDesc {
                        name: &format!("{} (init)", name),
                        requirements: self.get_buffer_memory_requirements(buffer),
                        location: gpu_allocator::MemoryLocation::CpuToGpu,
                        linear: true,
                        allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
                    };
                    let mut allocation = self.allocator.allocate(&alloc_desc)?;
                    {
                        let mapping = allocation.mapped_slice_mut().unwrap();
                        mapping[..data.len()].copy_from_slice(data);
                    }
                    self.bind_buffer_memory(buffer, allocation.memory(), allocation.offset())?;
                    gpu::Buffer { buffer, allocation }
                };

                self.cmd_copy_buffer(
                    pool.cmd_buffer,
                    buffer_init.buffer,
                    buffer.buffer,
                    &[vk::BufferCopy {
                        src_offset: 0,
                        dst_offset: 0,
                        size: data.len() as _,
                    }],
                );

                self.cmd_retire_buffer(pool, buffer_init);
            }
            gpu::BufferInit::None => (),
        }

        Ok(buffer)
    }

    pub unsafe fn create_image_gpu(
        &mut self,
        name: &str,
        desc: &gpu::ImageDesc,
        initialization: gpu::ImageInit,
    ) -> anyhow::Result<gpu::Image> {
        let image = {
            let vk_desc = vk::ImageCreateInfo::default()
                .image_type(desc.ty)
                .format(desc.format)
                .extent(desc.extent)
                .flags(vk::ImageCreateFlags::MUTABLE_FORMAT) // todo: hmm - image view create info list
                .mip_levels(desc.mip_levels as _)
                .array_layers(desc.array_layers as _)
                .samples(vk::SampleCountFlags::from_raw(desc.samples as _))
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(desc.usage);

            let image = self.create_image(&vk_desc, None)?;
            let alloc_desc = gpu_allocator::vulkan::AllocationCreateDesc {
                name,
                requirements: self.get_image_memory_requirements(image),
                location: gpu_allocator::MemoryLocation::GpuOnly,
                linear: false,
                allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
            };
            let allocation = self.allocator.allocate(&alloc_desc)?;
            self.bind_image_memory(image, allocation.memory(), allocation.offset())?;

            self.name_object(vk::ObjectType::IMAGE, image.as_raw(), name)?;

            gpu::Image {
                image,
                allocation: Some(allocation),
                mip_levels: desc.mip_levels,
                array_layers: desc.array_layers,
            }
        };

        match initialization {
            gpu::ImageInit::Host { pool, aspect, data } => {
                let buffer_init = {
                    let desc = vk::BufferCreateInfo::default()
                        .size(data.len() as _)
                        .usage(vk::BufferUsageFlags::TRANSFER_SRC);
                    let buffer = self.create_buffer(&desc, None)?;
                    let alloc_desc = gpu_allocator::vulkan::AllocationCreateDesc {
                        name: &format!("{} (init)", name),
                        requirements: self.get_buffer_memory_requirements(buffer),
                        location: gpu_allocator::MemoryLocation::CpuToGpu,
                        linear: true,
                        allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
                    };
                    let mut allocation = self.allocator.allocate(&alloc_desc)?;
                    {
                        let mapping = allocation.mapped_slice_mut().unwrap();
                        mapping[..data.len()].copy_from_slice(data);
                    }
                    self.bind_buffer_memory(buffer, allocation.memory(), allocation.offset())?;
                    gpu::Buffer { buffer, allocation }
                };

                let image = image.aspect(aspect);

                self.cmd_barriers(
                    pool,
                    &[],
                    &[gpu::ImageBarrier {
                        image,
                        src: gpu::ImageAccess {
                            access: gpu::Access::NONE,
                            stage: gpu::Stage::NONE,
                            layout: gpu::ImageLayout::UNDEFINED,
                        },
                        dst: gpu::ImageAccess {
                            access: gpu::Access::MEMORY_WRITE,
                            stage: gpu::Stage::COPY,
                            layout: gpu::ImageLayout::TRANSFER_DST_OPTIMAL,
                        },
                    }],
                );

                // todo: support multiple mipmaps
                let copy = vk::BufferImageCopy {
                    buffer_offset: 0,
                    buffer_row_length: 0,
                    buffer_image_height: 0,
                    image_subresource: vk::ImageSubresourceLayers {
                        aspect_mask: image.range.aspect_mask,
                        mip_level: image.range.base_mip_level,
                        base_array_layer: image.range.base_array_layer,
                        layer_count: image.range.layer_count,
                    },
                    image_extent: desc.extent,
                    image_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
                };

                self.cmd_copy_buffer_to_image(
                    pool.cmd_buffer,
                    buffer_init.buffer,
                    image.image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[copy],
                );

                self.cmd_retire_buffer(pool, buffer_init);
            }
            gpu::ImageInit::None => (),
        }

        Ok(image)
    }

    pub unsafe fn create_shader(&mut self, name: &str) -> anyhow::Result<gpu::Shader> {
        let mut file = std::io::Cursor::new(std::fs::read(self.spv_dir.join(name))?);
        let code = ash::util::read_spv(&mut file)?;
        let desc = vk::ShaderModuleCreateInfo::default().code(&code);
        let shader = self.device.create_shader_module(&desc, None)?;
        Ok(shader)
    }

    pub unsafe fn create_compute_pipeline<P>(
        &mut self,
        name: &str,
        shader: gpu::ShaderEntry,
    ) -> anyhow::Result<gpu::Pipeline<P>> {
        let layout_size = std::mem::size_of::<P>();
        let layout = match self.layouts.entry(layout_size) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                let mut push_constants = vec![];
                if layout_size > 0 {
                    push_constants.push(
                        vk::PushConstantRange::default()
                            .offset(0)
                            .size(layout_size as _)
                            .stage_flags(vk::ShaderStageFlags::ALL),
                    );
                }
                let set_layouts = [
                    self.descriptors_sampled_image.gpu.layout,
                    self.descriptors_storage_image.gpu.layout,
                ];

                let desc = vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(&set_layouts)
                    .push_constant_ranges(&push_constants);

                let pipeline_layout = self.device.create_pipeline_layout(&desc, None)?;
                *entry.insert(pipeline_layout)
            }
        };

        let entry = CString::new(shader.entry)?;

        let stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(shader.module)
            .name(&entry);
        let desc = vk::ComputePipelineCreateInfo::default()
            .stage(stage)
            .layout(layout);

        let pipelines = self
            .create_compute_pipelines(vk::PipelineCache::null(), &[desc], None)
            .unwrap();
        let pipeline = pipelines[0];

        self.name_object(vk::ObjectType::PIPELINE, pipeline.as_raw(), name)?;

        Ok(gpu::Pipeline {
            pipeline,
            bind_point: vk::PipelineBindPoint::COMPUTE,
            layout,
            layout_ty: std::marker::PhantomData,
        })
    }

    pub unsafe fn create_graphics_pipeline<P>(
        &mut self,
        name: &str,
        primitives: gpu::GraphicsPrimitives,
        rasterization: vk::PipelineRasterizationStateCreateInfo,
        fragments: gpu::ShaderEntry,
        output_color: &[gpu::GraphicsOutputColor],
    ) -> anyhow::Result<gpu::Pipeline<P>> {
        let layout_size = std::mem::size_of::<P>();
        let layout = match self.layouts.entry(layout_size) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                let mut push_constants = vec![];
                if layout_size > 0 {
                    push_constants.push(
                        vk::PushConstantRange::default()
                            .offset(0)
                            .size(layout_size as _)
                            .stage_flags(vk::ShaderStageFlags::ALL),
                    );
                }
                let set_layouts = [
                    self.descriptors_sampled_image.gpu.layout,
                    self.descriptors_storage_image.gpu.layout,
                ];

                let desc = vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(&set_layouts)
                    .push_constant_ranges(&push_constants);

                let pipeline_layout = self.device.create_pipeline_layout(&desc, None)?;
                *entry.insert(pipeline_layout)
            }
        };

        use std::ffi::CStr;
        let mut stages = Vec::new();
        let mut stage_entries = Vec::new();
        match primitives.shader {
            gpu::GraphicsPrimitivesShader::Vertex { shader } => {
                let entry = CString::new(shader.entry)?;
                let stage = vk::PipelineShaderStageCreateInfo::default()
                    .stage(vk::ShaderStageFlags::VERTEX)
                    .module(shader.module)
                    .name(CStr::from_ptr(entry.as_ptr()));
                stage_entries.push(entry);
                stages.push(stage);
            }
            gpu::GraphicsPrimitivesShader::Mesh { shader } => {
                let entry = CString::new(shader.entry)?;
                let stage = vk::PipelineShaderStageCreateInfo::default()
                    .stage(vk::ShaderStageFlags::MESH_NV)
                    .module(shader.module)
                    .name(CStr::from_ptr(entry.as_ptr()));
                stage_entries.push(entry);
                stages.push(stage);
            }
        };
        let fragment_entry = CString::new(fragments.entry)?;
        let fragment_stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(fragments.module)
            .name(&fragment_entry);
        stages.push(fragment_stage);

        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .primitive_restart_enable(primitives.restart)
            .topology(primitives.topology);

        let viewport = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);
        let multisample = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);

        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);
        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default();

        let color_blend_attachments = output_color
            .iter()
            .map(|c| match c.blend {
                gpu::GraphicsOutputBlend::Enable { color, alpha } => {
                    vk::PipelineColorBlendAttachmentState {
                        blend_enable: vk::TRUE,
                        src_color_blend_factor: color.src,
                        dst_color_blend_factor: color.dst,
                        color_blend_op: color.op,
                        src_alpha_blend_factor: alpha.src,
                        dst_alpha_blend_factor: alpha.dst,
                        alpha_blend_op: alpha.op,
                        color_write_mask: vk::ColorComponentFlags::RGBA,
                        ..Default::default()
                    }
                }
                gpu::GraphicsOutputBlend::Disable => vk::PipelineColorBlendAttachmentState {
                    blend_enable: vk::FALSE,
                    color_write_mask: vk::ColorComponentFlags::RGBA,
                    ..Default::default()
                },
            })
            .collect::<Box<[_]>>();
        let color_blend =
            vk::PipelineColorBlendStateCreateInfo::default().attachments(&color_blend_attachments);

        let color_formats = output_color.iter().map(|c| c.format).collect::<Box<[_]>>();
        let mut rendering =
            vk::PipelineRenderingCreateInfoKHR::default().color_attachment_formats(&color_formats);

        let desc = vk::GraphicsPipelineCreateInfo::default()
            .push_next(&mut rendering)
            .stages(&stages)
            .input_assembly_state(&input_assembly)
            .vertex_input_state(&vertex_input)
            .rasterization_state(&rasterization)
            .viewport_state(&viewport)
            .multisample_state(&multisample)
            .dynamic_state(&dynamic)
            .color_blend_state(&color_blend)
            .layout(layout);

        let pipelines = self
            .create_graphics_pipelines(vk::PipelineCache::null(), &[desc], None)
            .unwrap();
        let pipeline = pipelines[0];

        self.name_object(vk::ObjectType::PIPELINE, pipeline.as_raw(), name)?;

        Ok(gpu::Pipeline {
            pipeline,
            bind_point: vk::PipelineBindPoint::GRAPHICS,
            layout,
            layout_ty: std::marker::PhantomData,
        })
    }

    unsafe fn cmd_bind_descriptors(
        &mut self,
        cmd_buffer: vk::CommandBuffer,
        pipeline: vk::PipelineBindPoint,
        layout: vk::PipelineLayout,
    ) {
        let sets = [
            self.descriptors_sampled_image.gpu.set,
            self.descriptors_storage_image.gpu.set,
        ];
        self.cmd_bind_descriptor_sets(cmd_buffer, pipeline, layout, 0, &sets, &[]);
    }

    pub unsafe fn wait(&mut self, t: crate::Timestamp) -> anyhow::Result<()> {
        if t == 0 {
            // initial case
            return Ok(());
        }

        let semaphores = [self.timeline];
        let wait_values = [t];
        let wait_info = vk::SemaphoreWaitInfo::default()
            .semaphores(&semaphores)
            .values(&wait_values);
        self.device.wait_semaphores(&wait_info, !0)?;

        for pool in &mut self.pools {
            if let PoolState::Executing(t_pool) = pool.state {
                if t_pool > t {
                    continue;
                }

                self.device
                    .reset_command_pool(pool.cmd_pool, vk::CommandPoolResetFlags::empty())?;

                for view in pool.shrine.image_views.drain(..) {
                    self.device.destroy_image_view(view, None);
                }
                for buffer in pool.shrine.buffers.drain(..) {
                    self.device.destroy_buffer(buffer, None);
                }
                for image in pool.shrine.images.drain(..) {
                    self.device.destroy_image(image, None);
                }
                for allocation in pool.shrine.allocations.drain(..) {
                    self.allocator.free(allocation)?;
                }
            }
        }
        Ok(())
    }

    pub unsafe fn acquire_pool(&mut self) -> anyhow::Result<Pool> {
        let pool_id = self.pools.iter().position(|p| p.state == PoolState::Free);
        let pool_id = match pool_id {
            Some(id) => id,
            None => {
                let cmd_pool = {
                    let desc = vk::CommandPoolCreateInfo::default()
                        .queue_family_index(self.queue_family_index);
                    self.device.create_command_pool(&desc, None)?
                };
                let cmd_buffer = {
                    let desc = vk::CommandBufferAllocateInfo::default()
                        .command_pool(cmd_pool)
                        .level(vk::CommandBufferLevel::PRIMARY)
                        .command_buffer_count(1);
                    self.device.allocate_command_buffers(&desc)?[0]
                };

                self.pools.push(PoolData {
                    state: PoolState::Free,
                    cmd_pool,
                    cmd_buffer,
                    shrine: Shrine::default(),
                });
                self.pools.len() - 1
            }
        };

        let pool = &mut self.pools[pool_id];
        pool.state = PoolState::Recording;

        let cmd_buffer = pool.cmd_buffer;
        let begin_desc = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        self.device.begin_command_buffer(cmd_buffer, &begin_desc)?;

        let pool_handle = Pool {
            cmd_buffer,
            id: pool_id,
        };

        Ok(pool_handle)
    }

    pub unsafe fn submit_pool(
        &mut self,
        pool: Pool,
        submit: gpu::Submit,
    ) -> anyhow::Result<gpu::Timestamp> {
        self.device.end_command_buffer(pool.cmd_buffer)?;
        self.timeline_value += 1;
        self.pools[pool.id].state = PoolState::Executing(self.timeline_value);

        let waits = submit
            .waits
            .iter()
            .map(|desc| {
                vk::SemaphoreSubmitInfo::default()
                    .semaphore(desc.semaphore)
                    .stage_mask(desc.stage)
            })
            .collect::<Box<[_]>>();

        let mut signals = submit
            .signals
            .iter()
            .map(|desc| {
                vk::SemaphoreSubmitInfo::default()
                    .semaphore(desc.semaphore)
                    .stage_mask(desc.stage)
            })
            .collect::<Vec<_>>();
        signals.push(
            vk::SemaphoreSubmitInfo::default()
                .semaphore(self.timeline)
                .value(self.timeline_value)
                .stage_mask(gpu::Stage::NONE),
        );

        let cmd_buffers = [vk::CommandBufferSubmitInfo::default().command_buffer(pool.cmd_buffer)];

        let desc = [vk::SubmitInfo2::default()
            .wait_semaphore_infos(&waits)
            .signal_semaphore_infos(&signals)
            .command_buffer_infos(&cmd_buffers)];
        self.queue_submit2(self.queue, &desc, vk::Fence::null())?;

        Ok(self.timeline_value)
    }

    pub unsafe fn cmd_graphics_begin(
        &mut self,
        pool: gpu::Pool,
        render_area: vk::Rect2D,
        attachments_color: &[gpu::GraphicsAttachment],
    ) {
        let color_attachments = attachments_color
            .iter()
            .map(|attachment| attachment.as_vk())
            .collect::<Box<[_]>>();
        let desc = vk::RenderingInfo::default()
            .render_area(render_area)
            .layer_count(1)
            .color_attachments(&color_attachments);
        self.cmd_begin_rendering(pool.cmd_buffer, &desc);
    }

    pub unsafe fn cmd_graphics_draw<P>(
        &mut self,
        pool: gpu::Pool,
        kernel: gpu::Pipeline<P>,
        params: P,
        draws: &[gpu::GraphicsDraw],
    ) {
        assert_eq!(kernel.bind_point, vk::PipelineBindPoint::GRAPHICS);

        self.cmd_push_constants(
            pool.cmd_buffer,
            kernel.layout,
            vk::ShaderStageFlags::ALL,
            0,
            gpu::as_u8_slice(&[params]),
        );
        self.cmd_bind_descriptors(
            pool.cmd_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            kernel.layout,
        );
        self.cmd_bind_pipeline(
            pool.cmd_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            kernel.pipeline,
        );
        for draw in draws {
            self.cmd_draw(
                pool.cmd_buffer,
                draw.vertex_count,
                draw.instance_count,
                draw.first_vertex,
                draw.first_instance,
            );
        }
    }

    pub unsafe fn cmd_graphics_draw_mesh<P>(
        &mut self,
        pool: gpu::Pool,
        kernel: gpu::Pipeline<P>,
        params: P,
        draws: &[gpu::GraphicsDrawMesh],
    ) {
        assert_eq!(kernel.bind_point, vk::PipelineBindPoint::GRAPHICS);

        self.cmd_push_constants(
            pool.cmd_buffer,
            kernel.layout,
            vk::ShaderStageFlags::ALL,
            0,
            gpu::as_u8_slice(&[params]),
        );
        self.cmd_bind_descriptors(
            pool.cmd_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            kernel.layout,
        );
        self.cmd_bind_pipeline(
            pool.cmd_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            kernel.pipeline,
        );
        for draw in draws {
            self.ext.mesh_shader.as_ref().unwrap().cmd_draw_mesh_tasks(
                pool.cmd_buffer,
                draw.task_count,
                draw.first_task,
            );
        }
    }

    pub unsafe fn cmd_graphics_end(&mut self, pool: gpu::Pool) {
        self.cmd_end_rendering(pool.cmd_buffer);
    }

    pub unsafe fn cmd_compute_dispatch<P>(
        &mut self,
        pool: gpu::Pool,
        kernel: gpu::Pipeline<P>,
        params: P,
        launch: gpu::ComputeDispatch,
    ) {
        assert_eq!(kernel.bind_point, vk::PipelineBindPoint::COMPUTE);

        self.cmd_push_constants(
            pool.cmd_buffer,
            kernel.layout,
            vk::ShaderStageFlags::ALL,
            0,
            gpu::as_u8_slice(&[params]),
        );
        self.cmd_bind_descriptors(
            pool.cmd_buffer,
            vk::PipelineBindPoint::COMPUTE,
            kernel.layout,
        );
        self.cmd_bind_pipeline(
            pool.cmd_buffer,
            vk::PipelineBindPoint::COMPUTE,
            kernel.pipeline,
        );
        self.cmd_dispatch(pool.cmd_buffer, launch.x, launch.y, launch.z);
    }

    pub unsafe fn cmd_barriers(
        &mut self,
        pool: gpu::Pool,
        memory: &[gpu::MemoryBarrier],
        image: &[gpu::ImageBarrier],
    ) {
        let memory_barriers = memory
            .iter()
            .map(|barrier| {
                vk::MemoryBarrier2::default()
                    .src_access_mask(barrier.src.access)
                    .dst_access_mask(barrier.dst.access)
                    .src_stage_mask(barrier.src.stage)
                    .dst_stage_mask(barrier.dst.stage)
            })
            .collect::<Box<[_]>>();

        let image_barriers = image
            .iter()
            .map(|barrier| {
                vk::ImageMemoryBarrier2::default()
                    .image(barrier.image.image)
                    .subresource_range(barrier.image.range)
                    .old_layout(barrier.src.layout)
                    .new_layout(barrier.dst.layout)
                    .src_access_mask(barrier.src.access)
                    .dst_access_mask(barrier.dst.access)
                    .src_stage_mask(barrier.src.stage)
                    .dst_stage_mask(barrier.dst.stage)
            })
            .collect::<Box<[_]>>();

        let dependency = vk::DependencyInfo::default()
            .memory_barriers(&memory_barriers)
            .image_memory_barriers(&image_barriers);
        self.cmd_pipeline_barrier2(pool.cmd_buffer, &dependency);
    }
}

impl std::ops::Deref for Gpu {
    type Target = ash::Device;
    fn deref(&self) -> &Self::Target {
        &self.device
    }
}

impl std::ops::Drop for Gpu {
    fn drop(&mut self) {
        unsafe {
            self.device_wait_idle().unwrap();
        }
    }
}
