use crate::prelude::r3;
use glam::Vec3;

use rend3::managers::TextureManager;
use rend3_routine::base::{BaseRenderGraph, BaseRenderGraphIntermediateState};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    *,
};

use super::{
    shader_manager::ShaderManager,
    viewport_3d_routine::{DrawType, Viewport3dRoutine, ViewportBuffers},
};

pub struct PointCloudBuffer {
    buffer: Buffer,
    len: usize,
}

const NUM_BUFFERS: usize = 1;
const NUM_TEXTURES: usize = 0;

impl ViewportBuffers<NUM_BUFFERS, NUM_TEXTURES> for PointCloudBuffer {
    type Settings = ();
    fn get_wgpu_buffers(&self, _settings: &()) -> [&Buffer; NUM_BUFFERS] {
        [&self.buffer]
    }

    fn get_wgpu_textures<'a>(
        &'a self,
        _texture_manager: &'a TextureManager,
        _settings: &(),
    ) -> [&'a TextureView; NUM_TEXTURES] {
        []
    }

    fn get_draw_type(&self, _settings: &Self::Settings) -> DrawType<'_> {
        DrawType::UseInstances {
            num_vertices: 6,
            num_instances: self.len,
        }
    }
}

pub struct PointCloudRoutine {
    inner: Viewport3dRoutine<PointCloudBuffer, NUM_BUFFERS, NUM_TEXTURES>,
}

impl PointCloudRoutine {
    pub fn new(device: &Device, base: &BaseRenderGraph, shader_manager: &ShaderManager) -> Self {
        Self {
            inner: Viewport3dRoutine::new(
                "point cloud",
                device,
                base,
                shader_manager.get("point_cloud_draw"),
                PrimitiveTopology::TriangleList,
                FrontFace::Ccw,
                false,
            ),
        }
    }

    pub fn add_point_cloud(&mut self, device: &Device, points: &[Vec3]) {
        let buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(points),
            usage: BufferUsages::STORAGE,
        });
        self.inner.buffers.push(PointCloudBuffer {
            buffer,
            len: points.len(),
        });
    }

    pub fn clear(&mut self) {
        self.inner.clear()
    }

    pub fn add_to_graph<'node>(
        &'node self,
        graph: &mut r3::RenderGraph<'node>,
        state: &BaseRenderGraphIntermediateState,
    ) {
        self.inner.add_to_graph(graph, state, &());
    }
}
