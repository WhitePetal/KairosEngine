use std::{error::Error, sync::Arc};

use wgpu::{
    Adapter, BackendOptions, Backends, Device, ExperimentalFeatures, Features, InstanceFlags,
    Limits, MemoryBudgetThresholds, MemoryHints, PowerPreference, PresentMode, Queue,
    RequestAdapterOptions, Surface, SurfaceConfiguration, TextureUsages, Trace,
    wgt::DeviceDescriptor,
};
use winit::{dpi::PhysicalSize, platform::pump_events, window::Window};

pub struct RenderPipeline {
    window: Arc<Window>,
    pub device: Device,
    pub surface: Surface<'static>,
    pub surface_config: SurfaceConfiguration,
    pub adapter: Adapter,
    pub queue: Queue,
    window_size: PhysicalSize<u32>,
    window_size_changed: bool,
}

impl RenderPipeline {
    pub async fn new(window: Arc<Window>) -> Result<Self, Box<dyn Error>> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: Backends::all(),
            flags: InstanceFlags::default(),
            memory_budget_thresholds: MemoryBudgetThresholds::default(),
            backend_options: BackendOptions::from_env_or_default(),
        });
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await?;

        let features = adapter.features();
        let (device, queue) = adapter
            .request_device(&DeviceDescriptor {
                label: None,
                required_features: Features::empty(),
                required_limits: Limits::default(),
                experimental_features: ExperimentalFeatures::default(),
                memory_hints: MemoryHints::default(),
                trace: Trace::default(),
            })
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);
        let window_size = window.inner_size();
        let width = window_size.width;
        let height = window_size.height;
        println!("surface_caps.formats: {:?}", surface_caps.formats);
        let surface_config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface_caps.formats[0],
            width,
            height,
            present_mode: PresentMode::Fifo,
            desired_maximum_frame_latency: 3,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &surface_config);

        Ok(Self {
            window,
            device,
            surface,
            surface_config,
            adapter,
            queue,
            window_size,
            window_size_changed: false,
        })
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }

        self.surface_config.width = size.width;
        self.surface_config.height = size.height;
        self.surface.configure(&self.device, &self.surface_config);
    }

    pub fn max_texture_side(&self) -> usize {
        self.device.limits().max_texture_dimension_2d as usize
    }
}
