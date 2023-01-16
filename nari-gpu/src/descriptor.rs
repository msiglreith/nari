use ash::vk;

pub type CpuDescriptor = u32;
type CpuDescriptors = Vec<CpuDescriptor>;

#[derive(Debug, Copy, Clone)]
pub struct GpuDescriptors {
    pub layout: vk::DescriptorSetLayout,
    pub set: vk::DescriptorSet,
}

pub struct Descriptors {
    free_handle: usize,

    cpu: CpuDescriptors,
    pub(crate) gpu: GpuDescriptors,
}

impl Descriptors {
    pub fn new(len: usize, gpu: GpuDescriptors) -> Self {
        let mut cpu = CpuDescriptors::with_capacity(len);
        for i in 0..len {
            cpu.push(i as u32 + 1);
        }

        Self {
            free_handle: 0,
            cpu,
            gpu,
        }
    }

    fn invalid_index(&self) -> usize {
        self.cpu.len()
    }

    pub unsafe fn create(&mut self) -> CpuDescriptor {
        assert_ne!(self.free_handle, self.invalid_index()); // out of memory

        let idx = self.free_handle;
        let handle = self.cpu[self.free_handle];

        assert_ne!(handle as usize, self.free_handle);
        self.free_handle = handle as _;

        let handle = idx as _;
        self.cpu[idx] = handle;

        handle
    }
}
