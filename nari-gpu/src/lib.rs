mod descriptor;
mod device;
mod instance;
mod swapchain;

pub use self::descriptor::{CpuDescriptor, Descriptors};
pub use self::device::{Gpu, Pool};
pub use self::instance::Instance;
pub use self::swapchain::{Frame, Swapchain};

pub use ash::vk::{
    AccelerationStructureKHR as AccelerationStructure, AccessFlags2KHR as Access, BlendFactor,
    BlendOp, BufferImageCopy, BufferUsageFlags, CommandBuffer,
    DispatchIndirectCommand as ComputeDispatch, DrawIndirectCommand as GraphicsDraw,
    DrawMeshTasksIndirectCommandNV as GraphicsDrawMesh, Extent2D, Extent3D, Format, ImageLayout,
    ImageType, ImageUsageFlags, ImageView, Offset2D, Offset3D, PipelineStageFlags2KHR as Stage,
    PrimitiveTopology, Rect2D, Sampler, Semaphore, ShaderModule as Shader,
};

pub type DeviceAddress = u64;
pub type ImageAddress = u32;
pub type Timestamp = u64;

// Re-export dependencies
pub use ash::vk;
pub use gpu_allocator::{vulkan::*, MemoryLocation};

use std::ops::Range;

/// View a slice as raw byte slice.
///
/// Reinterprets the passed data as raw memory.
/// Be aware of possible packing and aligning rules by Rust compared to OpenGL.
pub fn as_u8_slice<T>(data: &[T]) -> &[u8] {
    let len = std::mem::size_of::<T>() * data.len();
    unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, len) }
}

#[derive(Debug, Copy, Clone)]
pub struct SemaphoreSubmit {
    pub semaphore: Semaphore,
    pub stage: Stage,
}

#[derive(Debug, Copy, Clone)]
pub struct Submit<'a> {
    pub waits: &'a [SemaphoreSubmit],
    pub signals: &'a [SemaphoreSubmit],
}

#[derive(Debug, Copy, Clone)]
pub struct MemoryAccess {
    pub access: Access,
    pub stage: Stage,
}

#[derive(Debug, Copy, Clone)]
pub struct MemoryBarrier {
    pub src: MemoryAccess,
    pub dst: MemoryAccess,
}

impl MemoryBarrier {
    pub fn full() -> Self {
        MemoryBarrier {
            src: MemoryAccess {
                access: Access::MEMORY_READ | Access::MEMORY_WRITE,
                stage: Stage::ALL_COMMANDS,
            },
            dst: MemoryAccess {
                access: Access::MEMORY_READ | Access::MEMORY_WRITE,
                stage: Stage::ALL_COMMANDS,
            },
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum BufferInit<'a> {
    Host { pool: Pool, data: &'a [u8] },
    None,
}

#[derive(Debug)]
pub struct Buffer {
    pub buffer: vk::Buffer,
    pub allocation: Allocation,
}

#[derive(Debug, Copy, Clone)]
pub struct BufferView {
    pub buffer: vk::Buffer,
    pub offset: u64,
    pub range: u64,
}

impl BufferView {
    pub fn whole(buffer: &Buffer) -> Self {
        Self {
            buffer: buffer.buffer,
            offset: 0,
            range: vk::WHOLE_SIZE,
        }
    }

    pub unsafe fn handle(&self, gpu: &Gpu) -> vk::DeviceAddress {
        let desc = vk::BufferDeviceAddressInfo::default().buffer(self.buffer);
        gpu.get_buffer_device_address(&desc) + self.offset
    }
}

#[derive(Debug, Copy, Clone)]
pub enum ImageInit<'a> {
    Host {
        pool: Pool,
        aspect: vk::ImageAspectFlags,
        data: &'a [u8],
    },
    None,
}

#[derive(Debug)]
pub struct Image {
    pub image: vk::Image,
    pub allocation: Option<Allocation>,
    pub mip_levels: usize,
    pub array_layers: usize,
}

impl Image {
    pub fn aspect(&self, aspect_mask: vk::ImageAspectFlags) -> ImageRange {
        ImageRange {
            image: self.image,
            range: vk::ImageSubresourceRange {
                aspect_mask,
                base_mip_level: 0,
                level_count: self.mip_levels as _,
                base_array_layer: 0,
                layer_count: self.array_layers as _,
            },
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ImageDesc {
    pub ty: ImageType,
    pub format: Format,
    pub extent: Extent3D,
    pub usage: ImageUsageFlags,
    pub mip_levels: usize,
    pub array_layers: usize,
    pub samples: usize,
}

#[derive(Debug, Copy, Clone)]
pub struct ImageViewDesc {
    pub ty: vk::ImageViewType,
    pub format: vk::Format,
    pub range: ImageRange,
    pub usage: ImageUsageFlags,
}

#[derive(Debug, Copy, Clone)]
pub struct ImageRange {
    pub image: vk::Image,
    pub range: vk::ImageSubresourceRange,
}

impl ImageRange {
    pub fn levels(self, levels: Range<u32>) -> Self {
        Self {
            image: self.image,
            range: vk::ImageSubresourceRange {
                base_mip_level: levels.start,
                level_count: levels.end - levels.start,
                ..self.range
            },
        }
    }

    pub fn layers(self, layers: Range<u32>) -> Self {
        Self {
            image: self.image,
            range: vk::ImageSubresourceRange {
                base_array_layer: layers.start,
                layer_count: layers.end - layers.start,
                ..self.range
            },
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ImageAccess {
    pub access: Access,
    pub stage: Stage,
    pub layout: ImageLayout,
}

impl ImageAccess {
    pub const UNDEFINED: Self = Self {
        access: Access::NONE,
        stage: Stage::NONE,
        layout: ImageLayout::UNDEFINED,
    };

    pub const COLOR_ATTACHMENT_WRITE: Self = Self {
        access: Access::COLOR_ATTACHMENT_WRITE,
        stage: Stage::COLOR_ATTACHMENT_OUTPUT,
        layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
    };

    pub const PRESENT: Self = Self {
        access: Access::NONE,
        stage: Stage::NONE,
        layout: ImageLayout::PRESENT_SRC_KHR,
    };
}

#[derive(Debug, Copy, Clone)]
pub struct ImageBarrier {
    pub image: ImageRange,
    pub src: ImageAccess,
    pub dst: ImageAccess,
}

pub struct ShaderEntry<'a> {
    pub module: Shader,
    pub entry: &'a str,
}

#[derive(Copy, Clone)]
pub struct Pipeline<T> {
    pipeline: vk::Pipeline,
    bind_point: vk::PipelineBindPoint,
    layout: vk::PipelineLayout,
    layout_ty: std::marker::PhantomData<T>,
}

#[derive(Copy, Clone)]
pub struct GraphicsAttachment {
    pub image_view: ImageView,
    pub layout: ImageLayout,
    pub load: vk::AttachmentLoadOp,
    pub store: vk::AttachmentStoreOp,
    pub clear: vk::ClearValue,
}

impl GraphicsAttachment {
    fn as_vk(self) -> vk::RenderingAttachmentInfo<'static> {
        vk::RenderingAttachmentInfo::default()
            .image_view(self.image_view)
            .image_layout(self.layout)
            .load_op(self.load)
            .store_op(self.store)
            .clear_value(self.clear)
    }
}

pub enum GraphicsPrimitivesShader<'a> {
    Vertex { shader: ShaderEntry<'a> },
    Mesh { shader: ShaderEntry<'a> },
}

pub struct GraphicsPrimitives<'a> {
    pub shader: GraphicsPrimitivesShader<'a>,
    pub topology: PrimitiveTopology,
    pub restart: bool,
}

#[derive(Debug, Copy, Clone)]
pub struct GraphicsBlendEq {
    pub op: BlendOp,
    pub src: BlendFactor,
    pub dst: BlendFactor,
}

impl GraphicsBlendEq {
    pub const OVER: Self = Self {
        op: BlendOp::ADD,
        src: BlendFactor::ONE,
        dst: BlendFactor::ONE_MINUS_SRC_ALPHA,
    };
}

#[derive(Debug, Copy, Clone)]
pub enum GraphicsOutputBlend {
    Enable {
        color: GraphicsBlendEq,
        alpha: GraphicsBlendEq,
    },
    Disable,
}

pub struct GraphicsOutputColor {
    pub format: vk::Format,
    pub blend: GraphicsOutputBlend,
}

pub struct GraphicsOutputDepthStencil {
    pub depth_format: vk::Format,
    pub stencil_format: vk::Format,
}
