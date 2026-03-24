use std::{
    collections::{HashMap, HashSet, VecDeque},
    mem,
    sync::Arc,
};

use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use glam::Mat4;
use image::{GenericImage, ImageReader, RgbaImage};
use wgpu::util::DeviceExt;
use winit::{dpi::PhysicalSize, window::Window};

use crate::{
    config::GraphicsBackend,
    game::model::StaticModelMesh as CpuStaticModelMesh,
    world::voxel::{
        chunk::ChunkCoord,
        meshing::{ChunkMesh, MeshVertex},
    },
};

pub(crate) mod backend_trait;
pub(crate) mod opengl;
pub(crate) mod vulkan;

use backend_trait::{Backend, ClearColor};

#[derive(Debug, Clone, Copy)]
pub(crate) struct CameraMatrices {
    pub(crate) view: Mat4,
    pub(crate) projection: Mat4,
}

impl Default for CameraMatrices {
    fn default() -> Self {
        Self {
            view: Mat4::IDENTITY,
            projection: Mat4::IDENTITY,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct VoxelFrameStats {
    pub(crate) uploaded_chunks: usize,
    pub(crate) uploaded_bytes: usize,
    pub(crate) drawn_chunks: usize,
    pub(crate) pending_uploads: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct StaticModelMesh {
    pub(crate) label: String,
    pub(crate) vertices: Vec<StaticModelVertex>,
    pub(crate) indices: Vec<u32>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct StaticModelVertex {
    pub(crate) position: [f32; 3],
    pub(crate) normal: [f32; 3],
    pub(crate) uv: [f32; 2],
    pub(crate) color: [f32; 4],
}

impl From<CpuStaticModelMesh> for StaticModelMesh {
    fn from(value: CpuStaticModelMesh) -> Self {
        Self {
            label: value.label,
            vertices: value
                .vertices
                .into_iter()
                .map(|vertex| StaticModelVertex {
                    position: vertex.position,
                    normal: vertex.normal,
                    uv: vertex.uv,
                    color: vertex.color,
                })
                .collect(),
            indices: value.indices,
        }
    }
}

pub(crate) struct Renderer {
    inner: WgpuBackend,
}

impl Renderer {
    pub(crate) fn new(window: Arc<Window>, backend: GraphicsBackend, vsync: bool) -> Result<Self> {
        let inner = pollster::block_on(WgpuBackend::new(window, backend, vsync))?;
        Ok(Self { inner })
    }

    pub(crate) fn enqueue_chunk_mesh_upload(&mut self, coord: ChunkCoord, mesh: ChunkMesh) {
        self.inner.enqueue_chunk_mesh_upload(coord, mesh);
    }

    pub(crate) fn enqueue_chunk_mesh_remove(&mut self, coord: ChunkCoord) {
        self.inner.enqueue_chunk_mesh_remove(coord);
    }

    pub(crate) fn set_visible_chunks<I>(&mut self, coords: I)
    where
        I: IntoIterator<Item = ChunkCoord>,
    {
        self.inner.set_visible_chunks(coords);
    }

    pub(crate) fn set_chunk_upload_budget_bytes_per_frame(&mut self, budget_bytes: usize) {
        self.inner
            .set_chunk_upload_budget_bytes_per_frame(budget_bytes);
    }

    pub(crate) fn take_voxel_frame_stats(&mut self) -> VoxelFrameStats {
        self.inner.take_voxel_frame_stats()
    }

    pub(crate) fn replace_static_model_meshes(&mut self, meshes: Vec<StaticModelMesh>) {
        self.inner.replace_static_model_meshes(meshes);
    }

    pub(crate) fn replace_viewmodel_meshes(&mut self, meshes: Vec<StaticModelMesh>) {
        self.inner.replace_viewmodel_meshes(meshes);
    }
}

impl Backend for Renderer {
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        self.inner.resize(size)
    }

    fn update_camera_matrices(&mut self, camera: CameraMatrices) {
        self.inner.update_camera_matrices(camera);
    }

    fn render(&mut self, clear: ClearColor) -> Result<()> {
        self.inner.render(clear)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct QuadVertex {
    position: [f32; 3],
    color: [f32; 3],
    uv: [f32; 2],
}

impl QuadVertex {
    fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        const ATTRS: [wgpu::VertexAttribute; 3] =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];

        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<QuadVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRS,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuChunkVertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
    material_id: u32,
}

impl GpuChunkVertex {
    fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        const ATTRS: [wgpu::VertexAttribute; 4] =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Uint32];

        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<GpuChunkVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRS,
        }
    }
}

impl From<MeshVertex> for GpuChunkVertex {
    fn from(value: MeshVertex) -> Self {
        Self {
            position: value.position,
            normal: value.normal,
            uv: value.uv,
            material_id: value.material_id,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CameraUniform {
    view: [[f32; 4]; 4],
    projection: [[f32; 4]; 4],
}

impl CameraUniform {
    fn from_matrices(matrices: CameraMatrices) -> Self {
        Self {
            view: matrices.view.to_cols_array_2d(),
            projection: matrices.projection.to_cols_array_2d(),
        }
    }
}

struct GpuChunkMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

struct GpuStaticModelMesh {
    _label: String,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuStaticModelVertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
    color: [f32; 4],
}

impl GpuStaticModelVertex {
    fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        const ATTRS: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x3,
            2 => Float32x2,
            3 => Float32x4
        ];

        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<GpuStaticModelVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRS,
        }
    }
}

impl From<StaticModelVertex> for GpuStaticModelVertex {
    fn from(value: StaticModelVertex) -> Self {
        Self {
            position: value.position,
            normal: value.normal,
            uv: value.uv,
            color: value.color,
        }
    }
}

enum ChunkUploadOp {
    Upsert {
        coord: ChunkCoord,
        vertices: Vec<GpuChunkVertex>,
        indices: Vec<u32>,
        estimated_bytes: usize,
    },
    Remove {
        coord: ChunkCoord,
    },
}

struct WgpuBackend {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    backend_name: &'static str,
    quad_pipeline: wgpu::RenderPipeline,
    voxel_pipeline: wgpu::RenderPipeline,
    static_model_pipeline: wgpu::RenderPipeline,
    viewmodel_pipeline: wgpu::RenderPipeline,
    quad_vertex_buffer: wgpu::Buffer,
    quad_index_buffer: wgpu::Buffer,
    quad_index_count: u32,
    texture_bind_group: wgpu::BindGroup,
    block_texture_bind_group: wgpu::BindGroup,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    depth_format: wgpu::TextureFormat,
    _depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    chunk_meshes: HashMap<ChunkCoord, GpuChunkMesh>,
    static_model_meshes: Vec<GpuStaticModelMesh>,
    viewmodel_meshes: Vec<GpuStaticModelMesh>,
    pending_chunk_uploads: VecDeque<ChunkUploadOp>,
    visible_chunks: HashSet<ChunkCoord>,
    upload_budget_bytes_per_frame: usize,
    frame_stats: VoxelFrameStats,
}

impl WgpuBackend {
    async fn new(window: Arc<Window>, backend: GraphicsBackend, vsync: bool) -> Result<Self> {
        let (backend_bits, backend_name) = match backend {
            GraphicsBackend::Vulkan => (vulkan::backends(), vulkan::NAME),
            GraphicsBackend::Opengl => (opengl::backends(), opengl::NAME),
        };

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: backend_bits,
            ..Default::default()
        });

        let surface = instance
            .create_surface(window.clone())
            .context("failed to create wgpu surface")?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .context("no suitable GPU adapter found")?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("tokenburner-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
            })
            .await
            .context("failed to request wgpu device")?;

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let present_mode = if vsync {
            wgpu::PresentMode::AutoVsync
        } else {
            wgpu::PresentMode::AutoNoVsync
        };

        let alpha_mode = caps.alpha_modes[0];

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            desired_maximum_frame_latency: 2,
            present_mode,
            alpha_mode,
            view_formats: vec![],
        };

        surface.configure(&device, &config);

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("tokenburner-camera-bgl"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(
                            std::num::NonZeroU64::new(mem::size_of::<CameraUniform>() as u64)
                                .expect("camera uniform size should be > 0"),
                        ),
                    },
                    count: None,
                }],
            });

        let camera_uniform = CameraUniform::from_matrices(CameraMatrices::default());
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tokenburner-camera-uniform"),
            contents: bytemuck::bytes_of(&camera_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tokenburner-camera-bg"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("tokenburner-texture-bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let textured_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tokenburner-textured-quad-shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../assets/shaders/clear.wgsl").into(),
            ),
        });

        let voxel_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tokenburner-voxel-shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../assets/shaders/voxel.wgsl").into(),
            ),
        });

        let static_model_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tokenburner-static-model-shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../assets/shaders/model.wgsl").into(),
            ),
        });

        let quad_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tokenburner-textured-pipeline-layout"),
            bind_group_layouts: &[&camera_bind_group_layout, &texture_bind_group_layout],
            immediate_size: 0,
        });

        let depth_format = wgpu::TextureFormat::Depth24Plus;

        let quad_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tokenburner-textured-quad-pipeline"),
            layout: Some(&quad_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &textured_shader,
                entry_point: Some("vs_main"),
                buffers: &[QuadVertex::layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &textured_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            multiview_mask: None,
            cache: None,
        });

        let voxel_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("tokenburner-voxel-pipeline-layout"),
                bind_group_layouts: &[&camera_bind_group_layout, &texture_bind_group_layout],
                immediate_size: 0,
            });

        let voxel_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tokenburner-voxel-pipeline"),
            layout: Some(&voxel_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &voxel_shader,
                entry_point: Some("vs_main"),
                buffers: &[GpuChunkVertex::layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &voxel_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            multiview_mask: None,
            cache: None,
        });

        let static_model_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("tokenburner-static-model-pipeline-layout"),
                bind_group_layouts: &[&camera_bind_group_layout],
                immediate_size: 0,
            });

        let static_model_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("tokenburner-static-model-pipeline"),
                layout: Some(&static_model_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &static_model_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[GpuStaticModelVertex::layout()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                primitive: wgpu::PrimitiveState {
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: depth_format,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &static_model_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                multiview_mask: None,
                cache: None,
            });

        let viewmodel_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tokenburner-viewmodel-pipeline"),
            layout: Some(&static_model_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &static_model_shader,
                entry_point: Some("vs_main"),
                buffers: &[GpuStaticModelVertex::layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &static_model_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            multiview_mask: None,
            cache: None,
        });

        const QUAD_VERTICES: [QuadVertex; 4] = [
            QuadVertex {
                position: [-0.6, -0.6, 0.0],
                color: [1.0, 1.0, 1.0],
                uv: [0.0, 1.0],
            },
            QuadVertex {
                position: [0.6, -0.6, 0.0],
                color: [1.0, 1.0, 1.0],
                uv: [1.0, 1.0],
            },
            QuadVertex {
                position: [0.6, 0.6, 0.0],
                color: [1.0, 1.0, 1.0],
                uv: [1.0, 0.0],
            },
            QuadVertex {
                position: [-0.6, 0.6, 0.0],
                color: [1.0, 1.0, 1.0],
                uv: [0.0, 0.0],
            },
        ];

        const QUAD_INDICES: [u16; 6] = [0, 1, 2, 0, 2, 3];

        let quad_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tokenburner-quad-vb"),
            contents: bytemuck::cast_slice(&QUAD_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let quad_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tokenburner-quad-ib"),
            contents: bytemuck::cast_slice(&QUAD_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        const TEX_WIDTH: u32 = 8;
        const TEX_HEIGHT: u32 = 8;

        let texture_data = checkerboard_rgba(TEX_WIDTH, TEX_HEIGHT, 2);
        let texture_extent = wgpu::Extent3d {
            width: TEX_WIDTH,
            height: TEX_HEIGHT,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tokenburner-checkerboard-texture"),
            size: texture_extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &texture_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * TEX_WIDTH),
                rows_per_image: Some(TEX_HEIGHT),
            },
            texture_extent,
        );

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("tokenburner-checkerboard-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tokenburner-texture-bg"),
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let block_texture_bind_group =
            Self::create_block_texture_bind_group(&device, &queue, &texture_bind_group_layout)?;

        let (depth_texture, depth_view) =
            Self::create_depth_resources(&device, &config, depth_format);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            backend_name,
            quad_pipeline,
            voxel_pipeline,
            static_model_pipeline,
            viewmodel_pipeline,
            quad_vertex_buffer,
            quad_index_buffer,
            quad_index_count: QUAD_INDICES.len() as u32,
            texture_bind_group,
            block_texture_bind_group,
            camera_buffer,
            camera_bind_group,
            depth_format,
            _depth_texture: depth_texture,
            depth_view,
            chunk_meshes: HashMap::new(),
            static_model_meshes: Vec::new(),
            viewmodel_meshes: Vec::new(),
            pending_chunk_uploads: VecDeque::new(),
            visible_chunks: HashSet::new(),
            upload_budget_bytes_per_frame: 2 * 1024 * 1024,
            frame_stats: VoxelFrameStats::default(),
        })
    }

    fn create_depth_resources(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        format: wgpu::TextureFormat,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tokenburner-depth-texture"),
            size: wgpu::Extent3d {
                width: config.width.max(1),
                height: config.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
        (depth_texture, depth_view)
    }

    fn create_block_texture_bind_group(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
    ) -> Result<wgpu::BindGroup> {
        const TILE_SIZE: u32 = 16;
        const ATLAS_COLUMNS: u32 = 4;
        const ATLAS_ROWS: u32 = 2;
        const TILE_FILES: [&str; 5] = [
            "assets/textures/blocks/grass_top.png",
            "assets/textures/blocks/grass_side.png",
            "assets/textures/blocks/grass_bottom.png",
            "assets/textures/blocks/stone.png",
            "assets/textures/blocks/sand.png",
        ];

        let mut atlas = RgbaImage::new(TILE_SIZE * ATLAS_COLUMNS, TILE_SIZE * ATLAS_ROWS);

        for (tile_index, path) in TILE_FILES.iter().enumerate() {
            let image = ImageReader::open(path)
                .with_context(|| format!("failed to open block texture {}", path))?
                .decode()
                .with_context(|| format!("failed to decode block texture {}", path))?
                .to_rgba8();

            if image.width() != TILE_SIZE || image.height() != TILE_SIZE {
                anyhow::bail!(
                    "block texture {} has size {}x{}, expected {}x{}",
                    path,
                    image.width(),
                    image.height(),
                    TILE_SIZE,
                    TILE_SIZE
                );
            }

            let tile_index = tile_index as u32;
            let dest_x = (tile_index % ATLAS_COLUMNS) * TILE_SIZE;
            let dest_y = (tile_index / ATLAS_COLUMNS) * TILE_SIZE;
            atlas.copy_from(&image, dest_x, dest_y).with_context(|| {
                format!("failed to copy block texture {} into atlas", path)
            })?;
        }

        let texture_extent = wgpu::Extent3d {
            width: atlas.width(),
            height: atlas.height(),
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tokenburner-block-atlas"),
            size: texture_extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            atlas.as_raw(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * atlas.width()),
                rows_per_image: Some(atlas.height()),
            },
            texture_extent,
        );

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("tokenburner-block-atlas-sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Ok(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tokenburner-block-atlas-bg"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        }))
    }

    fn reconfigure_surface(&mut self) {
        self.surface.configure(&self.device, &self.config);
        let (depth_texture, depth_view) =
            Self::create_depth_resources(&self.device, &self.config, self.depth_format);
        self._depth_texture = depth_texture;
        self.depth_view = depth_view;
    }

    fn enqueue_chunk_mesh_upload(&mut self, coord: ChunkCoord, mesh: ChunkMesh) {
        if mesh.is_empty() {
            self.pending_chunk_uploads
                .push_back(ChunkUploadOp::Remove { coord });
            return;
        }

        let vertices: Vec<GpuChunkVertex> = mesh.vertices.into_iter().map(Into::into).collect();
        let indices = mesh.indices;

        let estimated_bytes = vertices.len() * mem::size_of::<GpuChunkVertex>()
            + indices.len() * mem::size_of::<u32>();

        self.pending_chunk_uploads.push_back(ChunkUploadOp::Upsert {
            coord,
            vertices,
            indices,
            estimated_bytes,
        });
    }

    fn enqueue_chunk_mesh_remove(&mut self, coord: ChunkCoord) {
        self.pending_chunk_uploads
            .push_back(ChunkUploadOp::Remove { coord });
    }

    fn set_visible_chunks<I>(&mut self, coords: I)
    where
        I: IntoIterator<Item = ChunkCoord>,
    {
        self.visible_chunks.clear();
        self.visible_chunks.extend(coords);
    }

    fn set_chunk_upload_budget_bytes_per_frame(&mut self, budget_bytes: usize) {
        self.upload_budget_bytes_per_frame = budget_bytes.max(1);
    }

    fn take_voxel_frame_stats(&mut self) -> VoxelFrameStats {
        mem::take(&mut self.frame_stats)
    }

    fn replace_static_model_meshes(&mut self, meshes: Vec<StaticModelMesh>) {
        self.static_model_meshes = self.upload_static_model_meshes(meshes);
    }

    fn replace_viewmodel_meshes(&mut self, meshes: Vec<StaticModelMesh>) {
        self.viewmodel_meshes = self.upload_static_model_meshes(meshes);
    }

    fn upload_static_model_meshes(
        &self,
        meshes: Vec<StaticModelMesh>,
    ) -> Vec<GpuStaticModelMesh> {
        meshes
            .into_iter()
            .filter(|mesh| !mesh.vertices.is_empty() && !mesh.indices.is_empty())
            .map(|mesh| {
                let gpu_vertices: Vec<GpuStaticModelVertex> =
                    mesh.vertices.into_iter().map(Into::into).collect();

                let vertex_buffer =
                    self.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("tokenburner-static-model-vb"),
                            contents: bytemuck::cast_slice(&gpu_vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });

                let index_buffer =
                    self.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("tokenburner-static-model-ib"),
                            contents: bytemuck::cast_slice(&mesh.indices),
                            usage: wgpu::BufferUsages::INDEX,
                        });

                GpuStaticModelMesh {
                    _label: mesh.label,
                    vertex_buffer,
                    index_buffer,
                    index_count: mesh.indices.len() as u32,
                }
            })
            .collect()
    }

    fn process_chunk_uploads_with_budget(&mut self) -> (usize, usize) {
        let mut uploaded_chunks = 0usize;
        let mut uploaded_bytes = 0usize;
        let mut uploaded_any = false;

        let budget_bytes = self.upload_budget_bytes_per_frame.max(1);

        loop {
            let Some(op) = self.pending_chunk_uploads.pop_front() else {
                break;
            };

            match op {
                ChunkUploadOp::Remove { coord } => {
                    self.chunk_meshes.remove(&coord);
                }
                ChunkUploadOp::Upsert {
                    coord,
                    vertices,
                    indices,
                    estimated_bytes,
                } => {
                    if uploaded_any && uploaded_bytes.saturating_add(estimated_bytes) > budget_bytes
                    {
                        self.pending_chunk_uploads
                            .push_front(ChunkUploadOp::Upsert {
                                coord,
                                vertices,
                                indices,
                                estimated_bytes,
                            });
                        break;
                    }

                    uploaded_any = true;
                    uploaded_bytes = uploaded_bytes.saturating_add(estimated_bytes);

                    if vertices.is_empty() || indices.is_empty() {
                        self.chunk_meshes.remove(&coord);
                        continue;
                    }

                    let vertex_buffer =
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("tokenburner-voxel-vb"),
                                contents: bytemuck::cast_slice(&vertices),
                                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                            });

                    let index_buffer =
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("tokenburner-voxel-ib"),
                                contents: bytemuck::cast_slice(&indices),
                                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                            });

                    self.chunk_meshes.insert(
                        coord,
                        GpuChunkMesh {
                            vertex_buffer,
                            index_buffer,
                            index_count: indices.len() as u32,
                        },
                    );

                    uploaded_chunks += 1;
                }
            }
        }

        (uploaded_chunks, uploaded_bytes)
    }
}

impl Backend for WgpuBackend {
    fn name(&self) -> &'static str {
        self.backend_name
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.reconfigure_surface();
    }

    fn update_camera_matrices(&mut self, camera: CameraMatrices) {
        let camera_uniform = CameraUniform::from_matrices(camera);
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&camera_uniform));
    }

    fn render(&mut self, clear: ClearColor) -> Result<()> {
        let (uploaded_chunks, uploaded_bytes) = self.process_chunk_uploads_with_budget();

        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                self.reconfigure_surface();
                return Ok(());
            }
            Err(wgpu::SurfaceError::Timeout) => {
                return Ok(());
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                anyhow::bail!("wgpu surface out of memory");
            }
            Err(wgpu::SurfaceError::Other) => {
                return Ok(());
            }
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("tokenburner-main-pass"),
            });

        let mut drawn_chunks = 0usize;

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear-and-voxel-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: clear.r,
                            g: clear.g,
                            b: clear.b,
                            a: clear.a,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            rpass.set_pipeline(&self.quad_pipeline);
            rpass.set_bind_group(0, &self.camera_bind_group, &[]);
            rpass.set_bind_group(1, &self.texture_bind_group, &[]);
            rpass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
            rpass.set_index_buffer(self.quad_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            rpass.draw_indexed(0..self.quad_index_count, 0, 0..1);

            rpass.set_pipeline(&self.voxel_pipeline);
            rpass.set_bind_group(0, &self.camera_bind_group, &[]);
            rpass.set_bind_group(1, &self.block_texture_bind_group, &[]);

            for coord in &self.visible_chunks {
                let Some(chunk_mesh) = self.chunk_meshes.get(coord) else {
                    continue;
                };

                rpass.set_vertex_buffer(0, chunk_mesh.vertex_buffer.slice(..));
                rpass
                    .set_index_buffer(chunk_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                rpass.draw_indexed(0..chunk_mesh.index_count, 0, 0..1);
                drawn_chunks += 1;
            }

            rpass.set_pipeline(&self.static_model_pipeline);
            rpass.set_bind_group(0, &self.camera_bind_group, &[]);

            for mesh in &self.static_model_meshes {
                rpass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                rpass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                rpass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }

            rpass.set_pipeline(&self.viewmodel_pipeline);
            rpass.set_bind_group(0, &self.camera_bind_group, &[]);

            for mesh in &self.viewmodel_meshes {
                rpass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                rpass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                rpass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }

        self.queue.submit([encoder.finish()]);
        frame.present();

        self.frame_stats = VoxelFrameStats {
            uploaded_chunks,
            uploaded_bytes,
            drawn_chunks,
            pending_uploads: self.pending_chunk_uploads.len(),
        };

        Ok(())
    }
}

fn checkerboard_rgba(width: u32, height: u32, tile_size: u32) -> Vec<u8> {
    let safe_tile = tile_size.max(1);
    let mut data = vec![0u8; (width * height * 4) as usize];

    for y in 0..height {
        for x in 0..width {
            let is_light = ((x / safe_tile) + (y / safe_tile)).is_multiple_of(2);
            let rgba = if is_light {
                [245, 245, 245, 255]
            } else {
                [30, 30, 30, 255]
            };

            let idx = ((y * width + x) * 4) as usize;
            data[idx..idx + 4].copy_from_slice(&rgba);
        }
    }

    data
}
