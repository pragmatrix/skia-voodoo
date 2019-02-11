use voodoo::*;
use skia_safe::{skia, graphics, graphics::vulkan};
use std::os::raw;
use std::{ffi, ptr};
use skia_safe::bindings;
use vks;
use once_cell::sync;

#[derive(Debug)]
pub struct Context {
    backend: vulkan::BackendContext,
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
    instance: bindings::VkInstance,
    device: bindings::VkDevice)
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

        let graphics = graphics::Context::new_vulkan(&backend).unwrap();

        let ctx = Context {
            backend,
            graphics
        };

        ctx
    }
}

#[derive(Debug)]
struct Surface<'a> {
    context: &'a Context,
    surface: skia::Surface
}

impl<'a> Surface<'a> {
    pub unsafe fn from_texture(
        context: &'a mut Context,
        (image, image_memory, (width, height)):
        (&Image, &DeviceMemory, (u32, u32)))
        -> Surface<'a> {
        let allocation_size = image.memory_requirements().size();

        let alloc =
            vulkan::Alloc::new(
                image_memory.handle().to_raw() as *mut _,
                0, allocation_size, 0);

        let image_info =
            vulkan::ImageInfo::new(
                image.handle().to_raw() as *mut _,
                &alloc,
                bindings::VkImageTiling::VK_IMAGE_TILING_OPTIMAL,
                bindings::VkImageLayout::VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL,
                bindings::VkFormat::VK_FORMAT_R8G8B8A8_SRGB,
                1 /* level count */
            );

        let backend_texture =
            graphics::BackendTexture::new_vulkan((width, height), &image_info);

        let surface =
            skia::Surface::new_from_backend_texture(
                &mut context.graphics,
                &backend_texture,
                bindings::GrSurfaceOrigin::kTopLeft_GrSurfaceOrigin,
                1,
                bindings::SkColorType::kRGBA_8888_SkColorType
            ).unwrap();

        Surface { context, surface }
    }
}