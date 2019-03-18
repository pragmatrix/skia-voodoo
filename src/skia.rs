use voodoo::*;
use skia_safe::{skia, graphics, graphics::vulkan};
use std::os::raw;
use std::{ffi, ptr};
use vks;
use once_cell::sync;
pub use skia_safe::skia::{Canvas, Path, Paint};

pub struct Context {
    graphics: graphics::Context
}

fn instance_loader() -> &'static Loader {
    static INSTANCE: sync::OnceCell<Loader> = sync::OnceCell::INIT;
    INSTANCE.get_or_init(|| {
        Loader::new().unwrap()
    })
}

static mut GET_DEVICE_PROC_ADDR : vks::PFN_vkGetDeviceProcAddr = None;

unsafe extern "C" fn resolve(
    name: *const raw::c_char,
    instance: skia_bindings::VkInstance,
    device: skia_bindings::VkDevice)
    -> *const raw::c_void {

    let get_str = || ffi::CStr::from_ptr(name).to_str().unwrap();

    if !device.is_null() {
        let device = device as vks::VkDevice;

        let get_device_proc = GET_DEVICE_PROC_ADDR.unwrap();

        match get_device_proc(device, name) {
            Some(f) => {
                f as _
            },
            None => {
                println!("device proc resolve for {} failed", get_str());
                ptr::null()
            }
        }
    } else {

        // instance might be null here (vkCreateInstance, for example)

        let instance = instance as vks::VkInstance;
        let instance_proc_addr =
            instance_loader()
                .get_instance_proc_addr()
                .unwrap();

        match instance_proc_addr(instance, name) {
            Some(f) => {
                f as _
            },
            None => {
                println!("instance proc resolve for {} failed", get_str());
                ptr::null()
            }
        }
    }
}

impl Context {

    // note: it seems that we can access everything through queue.
    pub fn new(
        instance: &Instance,
        physical_device: &PhysicalDevice,
        device: &Device,
        queue: &Queue
    ) -> Context {

        let get_device_proc_addr =
            instance.proc_addr_loader()
                .vk.pfn_vkGetDeviceProcAddr.
                unwrap();

        unsafe {
            GET_DEVICE_PROC_ADDR = Some(get_device_proc_addr);
        }

        let backend = unsafe {
            vulkan::BackendContext::new(
                instance.handle().to_raw() as *mut _,
                physical_device.handle().to_raw() as *mut _,
                device.handle().to_raw() as *mut _,
                queue.handle().to_raw() as *mut _,
                queue.index(),
                Some(resolve)) };

        let graphics = graphics::Context::from_vulkan(&backend).unwrap();

        Context {
            graphics
        }
    }
}

pub struct Surface {
    surface: skia::Surface
}

impl Surface {

    pub fn from_texture(
        context: &mut Context,
        (image, image_memory, (width, height)):
        (&Image, &DeviceMemory, (i32, i32)))
        -> Surface {
        let allocation_size = image.memory_requirements().size();

        let alloc = unsafe {
            vulkan::Alloc::from_device_memory(
                image_memory.handle().to_raw() as _,
                0, allocation_size, vulkan::AllocFlag::empty())
        };

        let image_info = unsafe {
            vulkan::ImageInfo::from_image(
                image.handle().to_raw() as _,
                alloc,
                skia_bindings::VkImageTiling::VK_IMAGE_TILING_OPTIMAL,
                skia_bindings::VkImageLayout::VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL,
                skia_bindings::VkFormat::VK_FORMAT_R8G8B8A8_SRGB,
                1, None, None /* level count */
            )
        };

        let backend_texture = unsafe {
            graphics::BackendTexture::new_vulkan((width, height), &image_info)
        };

        let surface =
            skia::Surface::from_backend_texture(
                &mut context.graphics,
                &backend_texture,
                skia_bindings::GrSurfaceOrigin::kTopLeft_GrSurfaceOrigin,
                /* sample_count */ 1,
                skia_bindings::SkColorType::kRGBA_8888_SkColorType
            ).unwrap();

        Surface { surface }
    }

    pub fn canvas(&mut self) -> &mut skia::Canvas {
        self.surface.canvas()
    }

    pub fn flush(&mut self) {
        self.surface.flush();
    }

    // Use to retrieve the current layout the image is in.
    pub fn image_layout(&mut self) -> skia_bindings::VkImageLayout {
        let texture = self.surface.get_backend_texture(skia_bindings::SkSurface_BackendHandleAccess::kFlushRead_BackendHandleAccess).unwrap();
        let image_info = texture.get_image_info().unwrap();
        image_info.layout
    }
}