use voodoo::*;
use std::{ptr, os::raw, ffi::CStr};
use vks;
use once_cell::sync;
use skia_safe::{gpu, gpu::vk};
pub use skia_safe::{Canvas, Path, Paint, ColorType, SurfaceBackendHandleAccess};
use skia_safe::gpu::SurfaceOrigin;

pub struct Context {
    graphics: gpu::Context
}

fn instance_loader() -> &'static Loader {
    static INSTANCE: sync::OnceCell<Loader> = sync::OnceCell::INIT;
    INSTANCE.get_or_init(|| {
        Loader::new().unwrap()
    })
}

unsafe fn resolve(
    get_device_proc_addr: vks::PFN_vkGetDeviceProcAddr,
    name: &CStr,
    instance: vk::Instance,
    device: vk::Device)
    -> *const raw::c_void {

    let get_str = || name.to_str().unwrap();

    if !device.is_null() {
        let device = device as vks::VkDevice;

        let get_device_proc = get_device_proc_addr.unwrap();

        match get_device_proc(device, name.as_ptr() as _) {
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

        match instance_proc_addr(instance, name.as_ptr() as _) {
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

        let resolve = |gpo| unsafe {
            match gpo {
                vk::GetProcOf::Device(device, name) => {
                    let name = CStr::from_ptr(name);
                    resolve(Some(get_device_proc_addr), name, ptr::null_mut(), device)
                }
                vk::GetProcOf::Instance(instance, name) => {
                    let name = CStr::from_ptr(name);
                    resolve(Some(get_device_proc_addr), name, instance, ptr::null_mut())
                }
            }
        };

        let backend = unsafe {
            vk::BackendContext::new(
                instance.handle().to_raw() as _,
                physical_device.handle().to_raw() as _,
                device.handle().to_raw() as _,
                (queue.handle().to_raw() as _, queue.index() as _),
                &resolve)
        };

        let graphics = gpu::Context::new_vulkan(&backend).unwrap();

        Context {
            graphics
        }
    }
}

pub struct Surface {
    surface: skia_safe::Surface
}

impl Surface {

    pub fn from_texture(
        context: &mut Context,
        (image, image_memory, (width, height)):
        (&Image, &DeviceMemory, (i32, i32)))
        -> Surface {
        let allocation_size = image.memory_requirements().size();

        let alloc = unsafe {
            vk::Alloc::from_device_memory(
                image_memory.handle().to_raw() as _,
                0, allocation_size, vk::AllocFlag::empty())
        };

        let image_info = unsafe {
            vk::ImageInfo::from_image(
                image.handle().to_raw() as _,
                alloc,
                skia_bindings::VkImageTiling::VK_IMAGE_TILING_OPTIMAL,
                skia_bindings::VkImageLayout::VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL,
                skia_bindings::VkFormat::VK_FORMAT_R8G8B8A8_SRGB,
                1, None, None /* level count */
            )
        };

        let backend_texture = unsafe {
            gpu::BackendTexture::new_vulkan((width, height), &image_info)
        };

        let surface =
            skia_safe::Surface::from_backend_texture(
                &mut context.graphics,
                &backend_texture,
                SurfaceOrigin::TopLeft,
                None,
                ColorType::RGBA8888,
                None, None
            ).unwrap();

        Surface { surface }
    }

    pub fn canvas(&mut self) -> &mut skia_safe::Canvas {
        self.surface.canvas()
    }

    pub fn flush(&mut self) {
        self.surface.flush();
    }

    // Use to retrieve the current layout the image is in.
    pub fn image_layout(&mut self) -> skia_bindings::VkImageLayout {
        let texture = self.surface.backend_texture(SurfaceBackendHandleAccess::FlushRead).unwrap();
        let image_info = texture.vulkan_image_info().unwrap();
        image_info.layout
    }
}
