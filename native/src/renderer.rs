use crate::model::Model;
use eframe::glow::{self, HasContext};
use glam::{DVec3, Mat4, Vec3};
use std::collections::HashSet;
pub type Key = (usize, u32);
struct GpuMesh {
    key: Key,
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    ebo: glow::Buffer,
    count: i32,
    color: [f32; 3],
    center: DVec3,
}
struct GpuCloud {
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    count: i32,
    center: DVec3,
}
pub struct Renderer {
    clouds: Vec<GpuCloud>,
    program: glow::Program,
    meshes: Vec<GpuMesh>,
}
#[derive(Clone)]
pub struct Draw {
    pub hidden_clouds: Vec<usize>,
    pub matrix: Mat4,
    pub eye: Vec3,
    pub reference: Vec3,
    pub selected: HashSet<Key>,
    pub hidden: HashSet<Key>,
    pub clip_min: Vec3,
    pub clip_max: Vec3,
    pub mode: u8,
    pub edges: bool,
    pub background: [f32; 3],
}
impl Renderer {
    pub fn new(gl: &glow::Context) -> Result<Self, String> {
        unsafe {
            let program = gl.create_program()?;
            let sources = [
                (
                    glow::VERTEX_SHADER,
                    r#"#version 330
layout(location=0) in vec3 position;
layout(location=1) in vec3 normal;
uniform mat4 matrix;
out vec3 world;
out vec3 n;
void main(){world=position;n=normal;gl_PointSize=3.0;gl_Position=matrix*vec4(position,1.0);}"#,
                ),
                (
                    glow::FRAGMENT_SHADER,
                    r#"#version 330
in vec3 world;
in vec3 n;
uniform vec4 color;
uniform vec3 clip_min;
uniform vec3 clip_max;
uniform int unlit;
out vec4 output_color;
void main(){if(any(lessThan(world,clip_min))||any(greaterThan(world,clip_max))) discard;
float light=unlit==1?1.0:0.45+0.55*abs(dot(normalize(n),normalize(vec3(0.4,-0.5,0.8))));
output_color=unlit==2?vec4(n,1.0):vec4(color.rgb*light,color.a);}"#,
                ),
            ];
            for (kind, src) in sources {
                let shader = gl.create_shader(kind)?;
                gl.shader_source(shader, src);
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    return Err(gl.get_shader_info_log(shader));
                }
                gl.attach_shader(program, shader);
                gl.delete_shader(shader);
            }
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                return Err(gl.get_program_info_log(program));
            }
            Ok(Self {
                program,
                meshes: vec![],
                clouds: vec![],
            })
        }
    }
    pub fn upload(&mut self, gl: &glow::Context, index: usize, model: &Model) {
        unsafe {
            for mesh in &model.meshes {
                let vao = gl.create_vertex_array().unwrap();
                let vbo = gl.create_buffer().unwrap();
                let ebo = gl.create_buffer().unwrap();
                gl.bind_vertex_array(Some(vao));
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
                let center = (mesh.min + mesh.max) * 0.5;
                let offset = (mesh.origin - center).as_vec3();
                let vertices: Vec<_> = mesh
                    .vertices
                    .iter()
                    .map(|v| {
                        [
                            v[0] + offset.x,
                            v[1] + offset.y,
                            v[2] + offset.z,
                            v[3],
                            v[4],
                            v[5],
                        ]
                    })
                    .collect();
                gl.buffer_data_u8_slice(
                    glow::ARRAY_BUFFER,
                    bytemuck::cast_slice(&vertices),
                    glow::STATIC_DRAW,
                );
                gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo));
                gl.buffer_data_u8_slice(
                    glow::ELEMENT_ARRAY_BUFFER,
                    bytemuck::cast_slice(&mesh.indices),
                    glow::STATIC_DRAW,
                );
                gl.enable_vertex_attrib_array(0);
                gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 24, 0);
                gl.enable_vertex_attrib_array(1);
                gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, 24, 12);
                self.meshes.push(GpuMesh {
                    key: (index, mesh.id),
                    vao,
                    vbo,
                    ebo,
                    count: mesh.indices.len() as i32,
                    color: mesh.color,
                    center: (mesh.min + mesh.max) * 0.5,
                });
            }
            gl.bind_vertex_array(None);
        }
    }
    pub fn upload_clouds(
        &mut self,
        gl: &glow::Context,
        clouds: &[crate::pointcloud::Cloud],
        threshold: f64,
    ) {
        unsafe {
            for cloud in self.clouds.drain(..) {
                gl.delete_vertex_array(cloud.vao);
                gl.delete_buffer(cloud.vbo);
            }
            for cloud in clouds {
                let (min, max) = cloud.bounds();
                let center = (min + max) * 0.5;
                let vertices: Vec<[f32; 6]> = cloud
                    .world_points()
                    .enumerate()
                    .map(|(i, p)| {
                        let p = (p - center).as_vec3();
                        let color = cloud
                            .distances
                            .get(i)
                            .map(|d| {
                                let t = (d / threshold.max(0.001)).clamp(0., 1.) as f32;
                                [t, 1. - t, 0.15]
                            })
                            .unwrap_or(cloud.colors[i]);
                        [p.x, p.y, p.z, color[0], color[1], color[2]]
                    })
                    .collect();
                let vao = gl.create_vertex_array().unwrap();
                let vbo = gl.create_buffer().unwrap();
                gl.bind_vertex_array(Some(vao));
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
                gl.buffer_data_u8_slice(
                    glow::ARRAY_BUFFER,
                    bytemuck::cast_slice(&vertices),
                    glow::STATIC_DRAW,
                );
                gl.enable_vertex_attrib_array(0);
                gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 24, 0);
                gl.enable_vertex_attrib_array(1);
                gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, 24, 12);
                self.clouds.push(GpuCloud {
                    vao,
                    vbo,
                    count: vertices.len() as i32,
                    center,
                });
            }
            gl.bind_vertex_array(None);
        }
    }
    pub fn paint(&self, gl: &glow::Context, d: &Draw) {
        unsafe {
            gl.enable(glow::SCISSOR_TEST);
            gl.clear_color(d.background[0], d.background[1], d.background[2], 1.);
            gl.depth_mask(true);
            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LEQUAL);
            gl.disable(glow::CULL_FACE);
            gl.use_program(Some(self.program));
            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            let mut meshes: Vec<_> = self
                .meshes
                .iter()
                .filter(|m| !d.hidden.contains(&m.key))
                .collect();
            if d.mode == 2 {
                meshes.sort_by(|a, b| {
                    b.center
                        .distance_squared(d.eye.as_dvec3())
                        .total_cmp(&a.center.distance_squared(d.eye.as_dvec3()))
                });
                gl.depth_mask(false);
            }
            if d.mode == 1 {
                gl.polygon_mode(glow::FRONT_AND_BACK, glow::LINE);
            }
            for mesh in meshes {
                let matrix = d.matrix
                    * Mat4::from_translation((mesh.center - d.reference.as_dvec3()).as_vec3());
                gl.uniform_matrix_4_f32_slice(
                    gl.get_uniform_location(self.program, "matrix").as_ref(),
                    false,
                    &matrix.to_cols_array(),
                );
                gl.uniform_3_f32_slice(
                    gl.get_uniform_location(self.program, "clip_min").as_ref(),
                    &(d.clip_min.as_dvec3() - mesh.center).as_vec3().to_array(),
                );
                gl.uniform_3_f32_slice(
                    gl.get_uniform_location(self.program, "clip_max").as_ref(),
                    &(d.clip_max.as_dvec3() - mesh.center).as_vec3().to_array(),
                );
                let selected = d.selected.contains(&mesh.key);
                let c = if selected {
                    [1., 0.58, 0.08]
                } else if d.mode == 3 {
                    [0.78, 0.79, 0.80]
                } else {
                    mesh.color
                };
                let alpha = if d.mode == 2 && !selected { 0.22 } else { 1. };
                gl.uniform_4_f32(
                    gl.get_uniform_location(self.program, "color").as_ref(),
                    c[0],
                    c[1],
                    c[2],
                    alpha,
                );
                gl.uniform_1_i32(
                    gl.get_uniform_location(self.program, "unlit").as_ref(),
                    i32::from(d.mode == 1),
                );
                gl.bind_vertex_array(Some(mesh.vao));
                gl.draw_elements(glow::TRIANGLES, mesh.count, glow::UNSIGNED_INT, 0);
                if d.edges && d.mode != 1 {
                    gl.polygon_mode(glow::FRONT_AND_BACK, glow::LINE);
                    gl.enable(glow::POLYGON_OFFSET_LINE);
                    gl.polygon_offset(-1., -1.);
                    gl.uniform_4_f32(
                        gl.get_uniform_location(self.program, "color").as_ref(),
                        0.12,
                        0.13,
                        0.16,
                        0.45,
                    );
                    gl.draw_elements(glow::TRIANGLES, mesh.count, glow::UNSIGNED_INT, 0);
                    gl.disable(glow::POLYGON_OFFSET_LINE);
                    gl.polygon_mode(glow::FRONT_AND_BACK, glow::FILL);
                }
            }
            gl.polygon_mode(glow::FRONT_AND_BACK, glow::FILL);
            gl.depth_mask(true);
            gl.enable(glow::PROGRAM_POINT_SIZE);
            gl.uniform_1_i32(gl.get_uniform_location(self.program, "unlit").as_ref(), 2);
            for (i, cloud) in self.clouds.iter().enumerate() {
                if d.hidden_clouds.contains(&i) {
                    continue;
                }
                let matrix = d.matrix
                    * Mat4::from_translation((cloud.center - d.reference.as_dvec3()).as_vec3());
                gl.uniform_matrix_4_f32_slice(
                    gl.get_uniform_location(self.program, "matrix").as_ref(),
                    false,
                    &matrix.to_cols_array(),
                );
                gl.uniform_3_f32_slice(
                    gl.get_uniform_location(self.program, "clip_min").as_ref(),
                    &(d.clip_min.as_dvec3() - cloud.center).as_vec3().to_array(),
                );
                gl.uniform_3_f32_slice(
                    gl.get_uniform_location(self.program, "clip_max").as_ref(),
                    &(d.clip_max.as_dvec3() - cloud.center).as_vec3().to_array(),
                );
                gl.bind_vertex_array(Some(cloud.vao));
                gl.draw_arrays(glow::POINTS, 0, cloud.count);
            }
            gl.disable(glow::PROGRAM_POINT_SIZE);
            gl.disable(glow::DEPTH_TEST);
            gl.bind_vertex_array(None);
            gl.use_program(None);
        }
    }
    pub fn clear(&mut self, gl: &glow::Context) {
        unsafe {
            for cloud in self.clouds.drain(..) {
                gl.delete_vertex_array(cloud.vao);
                gl.delete_buffer(cloud.vbo);
            }
            for m in self.meshes.drain(..) {
                gl.delete_vertex_array(m.vao);
                gl.delete_buffer(m.vbo);
                gl.delete_buffer(m.ebo);
            }
        }
    }
    pub fn destroy(&mut self, gl: &glow::Context) {
        self.clear(gl);
        unsafe {
            gl.delete_program(self.program);
        }
    }
}
